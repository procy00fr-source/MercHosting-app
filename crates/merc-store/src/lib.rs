//! Persistencia de MercHosting en SQLite.
//!
//! - Migraciones versionadas y aplicadas dentro de una transacción.
//! - La base de datos guarda metadatos de MercHosting, nunca archivos de los servidores.
//! - Dos instancias no pueden registrar la misma carpeta (protege el mundo de dos procesos).

use merc_core::detect::ServerKind;
use merc_core::launch::LaunchProfile;
use merc_core::{Instance, InstanceId};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("error de base de datos: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("datos dañados en la base de datos: {0}")]
    Corrupt(String),
    #[error("ya hay una instancia registrada en esa carpeta: {0}")]
    DuplicateRoot(String),
    #[error("la base de datos es de una versión más nueva de MercHosting (esquema {0}); actualiza la aplicación")]
    TooNew(u32),
}

/// Lista ordenada de migraciones. Nunca editar una migración publicada: añadir una nueva.
const MIGRATIONS: &[(u32, &str)] = &[(
    1,
    r#"
    CREATE TABLE instances (
        id              TEXT PRIMARY KEY NOT NULL,
        name            TEXT NOT NULL,
        root            TEXT NOT NULL,
        root_key        TEXT NOT NULL UNIQUE,
        kind            TEXT NOT NULL,
        mc_version      TEXT,
        loader_version  TEXT,
        launch_json     TEXT NOT NULL,
        imported        INTEGER NOT NULL,
        created_at      INTEGER NOT NULL
    );
    CREATE TABLE audit_events (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        at           INTEGER NOT NULL,
        actor        TEXT NOT NULL,
        instance_id  TEXT,
        action       TEXT NOT NULL,
        detail       TEXT NOT NULL
    );
    CREATE INDEX audit_by_instance ON audit_events(instance_id, at);
    "#,
)];

pub const SCHEMA_VERSION: u32 = 1;

/// Clave de comparación de carpetas: en Windows las rutas no distinguen mayúsculas.
pub fn root_key(root: &Path) -> String {
    let s = root.to_string_lossy().replace('\\', "/");
    let s = s.trim_end_matches('/').to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

pub struct Store {
    conn: Connection,
}

fn kind_to_str(k: ServerKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

fn kind_from_str(s: &str) -> ServerKind {
    serde_json::from_value(serde_json::Value::String(s.to_string())).unwrap_or(ServerKind::Unknown)
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, StoreError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // WAL: lecturas concurrentes y menor riesgo ante cierres abruptos.
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<u32, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COALESCE(MAX(version),0) FROM schema_migrations", [], |r| {
                r.get(0)
            })?)
    }

    pub fn insert_instance(&mut self, inst: &Instance) -> Result<(), StoreError> {
        let key = root_key(&inst.root);
        let tx = self.conn.transaction()?;
        let existing: Option<String> = tx
            .query_row("SELECT name FROM instances WHERE root_key = ?1", [&key], |r| r.get(0))
            .optional()?;
        if let Some(name) = existing {
            return Err(StoreError::DuplicateRoot(name));
        }
        let launch = serde_json::to_string(&inst.launch).map_err(|e| StoreError::Corrupt(e.to_string()))?;
        tx.execute(
            "INSERT INTO instances (id,name,root,root_key,kind,mc_version,loader_version,launch_json,imported,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                inst.id.0.to_string(),
                inst.name,
                inst.root.to_string_lossy(),
                key,
                kind_to_str(inst.kind),
                inst.mc_version,
                inst.loader_version,
                launch,
                inst.imported as i64,
                inst.created_at_unix
            ],
        )?;
        audit_tx(
            &tx,
            "local",
            Some(inst.id),
            if inst.imported {
                "instance.import"
            } else {
                "instance.create"
            },
            &inst.root.to_string_lossy(),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_launch(&mut self, id: InstanceId, launch: &LaunchProfile) -> Result<bool, StoreError> {
        let json = serde_json::to_string(launch).map_err(|e| StoreError::Corrupt(e.to_string()))?;
        let tx = self.conn.transaction()?;
        let n = tx.execute(
            "UPDATE instances SET launch_json=?1 WHERE id=?2",
            params![json, id.0.to_string()],
        )?;
        if n == 1 {
            audit_tx(&tx, "local", Some(id), "instance.launch_profile.update", &json)?;
        }
        tx.commit()?;
        Ok(n == 1)
    }

    pub fn get_instance(&self, id: InstanceId) -> Result<Option<Instance>, StoreError> {
        let mut stmt = self.conn.prepare(&format!("{SELECT_INSTANCE} WHERE id = ?1"))?;
        let row = stmt.query_row([id.0.to_string()], row_to_raw).optional()?;
        row.map(raw_to_instance).transpose()
    }

    pub fn list_instances(&self) -> Result<Vec<Instance>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(&format!("{SELECT_INSTANCE} ORDER BY created_at, name"))?;
        let rows = stmt.query_map([], row_to_raw)?.collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(raw_to_instance).collect()
    }

    /// Quita el registro de la instancia. No borra ningún archivo del servidor.
    pub fn unregister_instance(&mut self, id: InstanceId) -> Result<bool, StoreError> {
        let tx = self.conn.transaction()?;
        let n = tx.execute("DELETE FROM instances WHERE id=?1", [id.0.to_string()])?;
        if n == 1 {
            audit_tx(&tx, "local", Some(id), "instance.unregister", "archivos conservados")?;
        }
        tx.commit()?;
        Ok(n == 1)
    }

    pub fn audit(
        &mut self,
        actor: &str,
        instance: Option<InstanceId>,
        action: &str,
        detail: &str,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        audit_tx(&tx, actor, instance, action, detail)?;
        tx.commit()?;
        Ok(())
    }

    pub fn recent_audit(&self, instance: Option<InstanceId>, limit: u32) -> Result<Vec<AuditRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT at, actor, instance_id, action, detail FROM audit_events
             WHERE (?1 IS NULL OR instance_id = ?1) ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![instance.map(|i| i.0.to_string()), limit], |r| {
                Ok(AuditRow {
                    at_unix: r.get(0)?,
                    actor: r.get(1)?,
                    instance_id: r.get(2)?,
                    action: r.get(3)?,
                    detail: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    pub at_unix: i64,
    pub actor: String,
    pub instance_id: Option<String>,
    pub action: String,
    pub detail: String,
}

const SELECT_INSTANCE: &str =
    "SELECT id,name,root,kind,mc_version,loader_version,launch_json,imported,created_at FROM instances";

type RawRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    i64,
    i64,
);

fn row_to_raw(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
    ))
}

fn raw_to_instance(r: RawRow) -> Result<Instance, StoreError> {
    let (id, name, root, kind, mc, loader, launch, imported, created) = r;
    Ok(Instance {
        id: InstanceId(uuid::Uuid::parse_str(&id).map_err(|e| StoreError::Corrupt(format!("id {id}: {e}")))?),
        name,
        root: PathBuf::from(root),
        kind: kind_from_str(&kind),
        mc_version: mc,
        loader_version: loader,
        launch: serde_json::from_str(&launch).map_err(|e| StoreError::Corrupt(format!("launch_json de {id}: {e}")))?,
        imported: imported != 0,
        created_at_unix: created,
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn audit_tx(
    tx: &Transaction<'_>,
    actor: &str,
    instance: Option<InstanceId>,
    action: &str,
    detail: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO audit_events (at, actor, instance_id, action, detail) VALUES (?1,?2,?3,?4,?5)",
        params![now_unix(), actor, instance.map(|i| i.0.to_string()), action, detail],
    )?;
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);",
    )?;
    let current: u32 = conn.query_row("SELECT COALESCE(MAX(version),0) FROM schema_migrations", [], |r| {
        r.get(0)
    })?;
    if current > SCHEMA_VERSION {
        return Err(StoreError::TooNew(current));
    }
    for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            params![version, now_unix()],
        )?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merc_core::launch::LaunchTarget;

    fn sample(root: &str) -> Instance {
        Instance {
            id: InstanceId::new(),
            name: "MercPrime".into(),
            root: PathBuf::from(root),
            kind: ServerKind::NeoForge,
            mc_version: Some("1.21.1".into()),
            loader_version: Some("21.1.77".into()),
            launch: LaunchProfile {
                java: "C:/Java/jdk-21/bin/java.exe".into(),
                min_memory_mb: 1024,
                max_memory_mb: 4096,
                extra_jvm_args: vec![],
                target: LaunchTarget::ArgsFile {
                    args_file: "libraries/net/neoforged/neoforge/21.1.77/win_args.txt".into(),
                    user_jvm_args: Some("user_jvm_args.txt".into()),
                },
            },
            imported: true,
            created_at_unix: 1,
        }
    }

    #[test]
    fn roundtrip_and_audit() {
        let mut s = Store::open_in_memory().unwrap();
        assert_eq!(s.schema_version().unwrap(), SCHEMA_VERSION);
        let inst = sample("/srv/mercprime");
        s.insert_instance(&inst).unwrap();
        assert_eq!(s.get_instance(inst.id).unwrap().unwrap(), inst);
        assert_eq!(s.list_instances().unwrap().len(), 1);
        let audit = s.recent_audit(Some(inst.id), 10).unwrap();
        assert_eq!(audit[0].action, "instance.import");
    }

    #[test]
    fn duplicate_root_rejected() {
        let mut s = Store::open_in_memory().unwrap();
        s.insert_instance(&sample("/srv/a")).unwrap();
        let err = s.insert_instance(&sample("/srv/a/")).unwrap_err();
        assert!(matches!(err, StoreError::DuplicateRoot(_)));
    }

    #[test]
    fn unregister_keeps_nothing_else() {
        let mut s = Store::open_in_memory().unwrap();
        let i = sample("/srv/b");
        s.insert_instance(&i).unwrap();
        assert!(s.unregister_instance(i.id).unwrap());
        assert!(s.get_instance(i.id).unwrap().is_none());
        assert_eq!(s.recent_audit(Some(i.id), 5).unwrap()[0].action, "instance.unregister");
    }

    #[test]
    fn reopen_does_not_remigrate_and_rejects_newer() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("merc.db");
        {
            let mut s = Store::open(&p).unwrap();
            s.insert_instance(&sample("/srv/c")).unwrap();
        }
        let s = Store::open(&p).unwrap();
        assert_eq!(s.list_instances().unwrap().len(), 1);
        drop(s);
        let c = Connection::open(&p).unwrap();
        c.execute("INSERT INTO schema_migrations VALUES (999, 0)", []).unwrap();
        drop(c);
        assert!(matches!(Store::open(&p), Err(StoreError::TooNew(999))));
    }
}
