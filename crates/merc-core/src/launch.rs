//! Perfiles de arranque. Producen la lista de argumentos que se pasa directamente al
//! ejecutable de Java (sin shell ni CMD intermedio).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LaunchTarget {
    /// `java ... -jar <jar> nogui` (Vanilla, Paper, Purpur, Spigot, Fabric).
    Jar { jar: PathBuf },
    /// `java @user_jvm_args.txt ... @libraries/.../win_args.txt nogui` (NeoForge, Forge >= 1.17).
    ArgsFile {
        args_file: PathBuf,
        user_jvm_args: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchProfile {
    /// Ruta absoluta al ejecutable Java (java.exe). No se resuelve por PATH a escondidas.
    pub java: PathBuf,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    /// Argumentos JVM adicionales elegidos por el usuario (modo avanzado).
    pub extra_jvm_args: Vec<String>,
    pub target: LaunchTarget,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LaunchError {
    #[error("la memoria máxima debe ser al menos 512 MB")]
    MemoryTooLow,
    #[error("la memoria mínima no puede superar la máxima")]
    MemoryOrder,
    #[error("argumento JVM no permitido: {0}")]
    ForbiddenArg(String),
}

pub const MIN_MAX_MEMORY_MB: u32 = 512;

pub const ENCODING_ARGS: [&str; 4] = [
    "-Dstdout.encoding=UTF-8",
    "-Dstderr.encoding=UTF-8",
    "-Dsun.stdout.encoding=UTF-8",
    "-Dsun.stderr.encoding=UTF-8",
];

/// Argumentos que cambiarían qué se ejecuta o romperían el perfil: se gestionan por MercHosting.
fn forbidden(arg: &str) -> bool {
    let a = arg.trim();
    a.is_empty()
        || a.chars().any(|c| c.is_control())
        || a.starts_with('@')
        || a == "-jar"
        || a == "-cp"
        || a == "-classpath"
        || a == "--class-path"
        || a.starts_with("-Xmx")
        || a.starts_with("-Xms")
        || a.starts_with("--module-path")
}

impl LaunchProfile {
    pub fn validate(&self) -> Result<(), LaunchError> {
        if self.max_memory_mb < MIN_MAX_MEMORY_MB {
            return Err(LaunchError::MemoryTooLow);
        }
        if self.min_memory_mb > self.max_memory_mb {
            return Err(LaunchError::MemoryOrder);
        }
        if let Some(bad) = self.extra_jvm_args.iter().find(|a| forbidden(a)) {
            return Err(LaunchError::ForbiddenArg(bad.clone()));
        }
        Ok(())
    }

    /// Argumentos para Java, sin incluir el ejecutable. Las rutas son relativas al
    /// directorio de trabajo, que siempre es la raíz de la instancia.
    ///
    /// Orden para NeoForge/Forge: `@user_jvm_args.txt` primero y luego -Xms/-Xmx de
    /// MercHosting, para que la memoria elegida en la app prevalezca (en HotSpot gana el
    /// último -Xmx). Los @archivos se expanden en el lugar donde aparecen.
    pub fn build_args(&self) -> Result<Vec<String>, LaunchError> {
        self.validate()?;
        let mut args = Vec::new();
        if let LaunchTarget::ArgsFile {
            user_jvm_args: Some(u), ..
        } = &self.target
        {
            args.push(format!("@{}", u.to_string_lossy().replace('\\', "/")));
        }
        if self.min_memory_mb > 0 {
            args.push(format!("-Xms{}M", self.min_memory_mb));
        }
        args.push(format!("-Xmx{}M", self.max_memory_mb));
        args.extend(self.extra_jvm_args.iter().map(|s| s.trim().to_string()));
        // Sin consola, Java en Windows escribe en la página de códigos del sistema (p. ej. Cp1252)
        // y las tildes llegan rotas. Se fuerza UTF-8 en la salida: `stdout.encoding` (Java 19+)
        // y `sun.stdout.encoding` (Java 8–18). Java ignora las propiedades que no reconoce.
        args.extend(ENCODING_ARGS.iter().map(|s| s.to_string()));
        match &self.target {
            LaunchTarget::Jar { jar } => {
                args.push("-jar".into());
                args.push(jar.to_string_lossy().into_owned());
            }
            LaunchTarget::ArgsFile { args_file, .. } => {
                args.push(format!("@{}", args_file.to_string_lossy().replace('\\', "/")));
            }
        }
        args.push("nogui".into());
        Ok(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(target: LaunchTarget) -> LaunchProfile {
        LaunchProfile {
            java: "java".into(),
            min_memory_mb: 1024,
            max_memory_mb: 4096,
            extra_jvm_args: vec![],
            target,
        }
    }

    #[test]
    fn jar_args() {
        let p = base(LaunchTarget::Jar {
            jar: "server.jar".into(),
        });
        let mut want = vec!["-Xms1024M", "-Xmx4096M"];
        want.extend(ENCODING_ARGS);
        want.extend(["-jar", "server.jar", "nogui"]);
        assert_eq!(p.build_args().unwrap(), want);
    }

    #[test]
    fn neoforge_args_order() {
        let p = base(LaunchTarget::ArgsFile {
            args_file: "libraries/net/neoforged/neoforge/21.1.77/win_args.txt".into(),
            user_jvm_args: Some("user_jvm_args.txt".into()),
        });
        let mut want = vec!["@user_jvm_args.txt", "-Xms1024M", "-Xmx4096M"];
        want.extend(ENCODING_ARGS);
        want.extend(["@libraries/net/neoforged/neoforge/21.1.77/win_args.txt", "nogui"]);
        assert_eq!(p.build_args().unwrap(), want);
    }

    #[test]
    fn rejects_bad() {
        let mut p = base(LaunchTarget::Jar {
            jar: "server.jar".into(),
        });
        p.max_memory_mb = 256;
        assert_eq!(p.validate(), Err(LaunchError::MemoryTooLow));
        p.max_memory_mb = 2048;
        p.min_memory_mb = 4096;
        assert_eq!(p.validate(), Err(LaunchError::MemoryOrder));
        p.min_memory_mb = 0;
        for bad in ["-jar", "@otro.txt", "-Xmx99G", "-cp", "a\nb", ""] {
            p.extra_jvm_args = vec![bad.into()];
            assert!(matches!(p.validate(), Err(LaunchError::ForbiddenArg(_))), "{bad:?}");
        }
        p.extra_jvm_args = vec!["-XX:+UseG1GC".into()];
        assert!(p.validate().is_ok());
    }
}
