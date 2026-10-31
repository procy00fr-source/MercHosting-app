// Pestaña Copias de seguridad: hacer copias comprobadas (en frío o con el guardado pausado),
// verificarlas, protegerlas, restaurarlas (con copia previa automática) y limpiar las viejas.

import {
  Download,
  ArchiveRestore,
  CheckCircle2,
  FolderOpen,
  Globe,
  HardDrive,
  Lock,
  LockOpen,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  TriangleAlert,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { BackupInfo, BackupList, BackupScope, InstanceId, WorldOp } from "../api/types";
import { formatSize } from "../format";
import { Dialog } from "./Dialog";
import { uiLocale } from "../i18n/lang";

const OP_TEXT: Partial<Record<WorldOp["kind"], string>> = {
  backup: "Haciendo la copia y comprobándola",
  verify: "Verificando la copia",
  restore: "Restaurando",
};

const REASON: Record<BackupInfo["reason"], string> = {
  manual: "Manual",
  before_restore: "Antes de restaurar",
  before_update: "Antes de actualizar",
  scheduled: "Programada",
};

function when(ms: number): string {
  return new Date(ms).toLocaleString(uiLocale(), { dateStyle: "medium", timeStyle: "short" });
}

interface Props {
  /** Servidor de un amigo: sin abrir carpetas de esta PC. */
  remote?: boolean;
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  /** Estado del servidor: `ONLINE`, `STOPPED`, etc. */
  state: string;
  hasConsole: boolean;
}

type Ask = { kind: "restore" | "delete"; backup: BackupInfo } | { kind: "create"; scope: BackupScope; note: string };

export function BackupsPanel({ api, sup, id, state, hasConsole, remote = false }: Props) {
  const [list, setList] = useState<BackupList | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [op, setOp] = useState<WorldOp | null>(null);
  const [ask, setAsk] = useState<Ask | null>(null);
  const [keep, setKeep] = useState("");

  const stopped = state === "STOPPED" || state === "CRASHED";
  const online = state === "ONLINE" && hasConsole;
  const canCopy = stopped || online;

  const load = useCallback(async () => {
    try {
      const l = await sup.backups(id);
      setList(l);
      setKeep(String(l.keep));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  useEffect(() => {
    load();
    sup.adminState(id).then(
      (st) => setOp(st.world_op),
      () => undefined,
    );
  }, [load, sup, id]);

  // Mientras dura una copia, verificación o restauración, se consulta el progreso.
  const watching = busy || op !== null;
  useEffect(() => {
    if (!watching) return;
    let alive = true;
    const t = setInterval(async () => {
      try {
        const st = await sup.adminState(id);
        if (!alive) return;
        setOp(st.world_op);
        if (!st.world_op && !busy) load();
      } catch {
        /* se reintenta */
      }
    }, 500);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [watching, busy, sup, id, load]);

  async function run(fn: () => Promise<BackupList>, msg: (l: BackupList) => string) {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const l = await fn();
      setList(l);
      setKeep(String(l.keep));
      setNote(msg(l));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
      setOp(null);
    }
  }

  async function cancelOp() {
    try {
      await sup.cancelWorldOp(id);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const items = list?.items ?? [];
  const locked = busy || op !== null;
  const keepNum = Number(keep);
  const keepValid = Number.isInteger(keepNum) && keepNum >= 1 && keepNum <= 500;

  return (
    <div className="admin-panel backups-panel" role="tabpanel" aria-label="Copias de seguridad">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">Copias de seguridad</h2>
          <p className="field-help">
            Cada copia se comprueba entera al terminar (huella SHA-256 de cada archivo). Se guardan fuera de la carpeta del
            servidor.
          </p>
        </div>
        {!remote && (
          <button type="button" className="btn btn-sm" onClick={() => api.openBackupsFolder(id)}>
            <FolderOpen size={14} aria-hidden="true" /> Ver carpeta
          </button>
        )}
        <button type="button" className="btn btn-sm" onClick={load} disabled={locked}>
          <RefreshCw size={14} aria-hidden="true" /> Actualizar
        </button>
      </div>

      <div className="backup-actions" data-tour="hacer-copia">
        <button
          type="button"
          className="card backup-new"
          disabled={locked || !canCopy}
          onClick={() => setAsk({ kind: "create", scope: "world", note: "" })}
        >
          <Globe size={22} aria-hidden="true" />
          <span>
            <strong>Copia del mundo</strong>
            <span className="field-help">El mundo activo con el Nether y el End. Lo más rápido.</span>
          </span>
        </button>
        <button
          type="button"
          className="card backup-new"
          disabled={locked || !canCopy}
          onClick={() => setAsk({ kind: "create", scope: "full", note: "" })}
        >
          <HardDrive size={22} aria-hidden="true" />
          <span>
            <strong>Copia completa</strong>
            <span className="field-help">Toda la carpeta: mundos, mods, plugins y ajustes.</span>
          </span>
        </button>
      </div>
      {online && (
        <p className="field-help">
          El servidor está encendido: durante la copia se pausa el guardado automático del juego y se reanuda al terminar. Los
          jugadores pueden seguir jugando.
        </p>
      )}
      {!canCopy && (
        <p className="field-help warn-text">
          {state === "ONLINE"
            ? "MercHosting perdió la consola de este servidor: detén el servidor para hacer una copia en frío."
            : "Espera a que el servidor termine de encender o de apagarse."}
        </p>
      )}

      {op && OP_TEXT[op.kind] && (
        <div className="card world-op" role="status">
          <div className="dl">
            <div className="dl-head">
              <span>{OP_TEXT[op.kind]}…</span>
              <span>
                {formatSize(op.done_bytes)}
                {op.total_bytes ? ` de ${formatSize(op.total_bytes)}` : ""}
              </span>
            </div>
            <div
              className="dl-bar"
              role="progressbar"
              aria-label="Progreso"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={op.total_bytes ? Math.min(100, Math.round((op.done_bytes / op.total_bytes) * 100)) : undefined}
            >
              <span
                style={{ width: op.total_bytes ? `${Math.min(100, (op.done_bytes / op.total_bytes) * 100)}%` : "35%" }}
                data-indeterminate={!op.total_bytes}
              />
            </div>
          </div>
          <button type="button" className="btn btn-sm" onClick={cancelOp}>
            <X size={14} aria-hidden="true" /> Cancelar
          </button>
        </div>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {note && !error && (
        <p className="field-help ok-text" role="status">
          {note}
        </p>
      )}

      {list === null && !error ? (
        <p className="wiz-loading" role="status">
          Buscando copias…
        </p>
      ) : items.length === 0 ? (
        <div className="card empty-small">
          <h2>Todavía no hay copias</h2>
          <p>Haz una antes de probar mods nuevos, cambiar de versión o hacer un evento: si algo sale mal, vuelves atrás.</p>
        </div>
      ) : (
        <ul className="world-list" aria-label="Copias guardadas">
          {items.map((b) => (
            <li key={b.id} className="card world-card backup-card" data-damaged={b.verified === false}>
              <span className="world-icon">
                {b.scope === "world" ? <Globe size={20} aria-hidden="true" /> : <HardDrive size={20} aria-hidden="true" />}
              </span>
              <span className="world-main">
                <span className="world-name">
                  {when(b.created_unix_ms)}
                  <span className="badge badge-muted">{b.scope === "world" ? "Mundo" : "Completa"}</span>
                  {b.reason !== "manual" && <span className="badge badge-muted">{REASON[b.reason]}</span>}
                  {b.protected && (
                    <span className="badge">
                      <Lock size={10} aria-hidden="true" /> Protegida
                    </span>
                  )}
                </span>
                <span className="world-meta">
                  {b.scope === "world" && b.world ? `«${b.world}» · ` : ""}
                  {formatSize(b.size_bytes)} · {b.files} archivos
                  {b.mc_version ? ` · Minecraft ${b.mc_version}` : ""}
                  {b.note ? ` · ${b.note}` : ""}
                </span>
                <span className="world-meta">
                  {b.verified === true ? (
                    <span className="ok-text">
                      <CheckCircle2 size={12} aria-hidden="true" /> Verificada
                      {b.verified_unix_ms ? ` el ${when(b.verified_unix_ms)}` : ""}
                    </span>
                  ) : b.verified === false ? (
                    <span className="warn-text">
                      <ShieldAlert size={12} aria-hidden="true" /> Dañada: {b.problems.slice(0, 2).join("; ")}
                    </span>
                  ) : (
                    <span>Sin verificar</span>
                  )}
                  {b.hot &&
                    (b.consistent ? (
                      " · Hecha con el servidor encendido (guardado pausado)"
                    ) : (
                      <span className="warn-text">
                        {" "}
                        · <TriangleAlert size={12} aria-hidden="true" /> Encendido y sin confirmar el guardado: puede tener
                        cambios a medias
                      </span>
                    ))}
                </span>
              </span>
              <span className="world-actions">
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={locked}
                  aria-label={`Verificar copia del ${when(b.created_unix_ms)}`}
                  onClick={() =>
                    run(
                      () => sup.verifyBackup(id, b.id),
                      (l) =>
                        l.items.find((x) => x.id === b.id)?.verified
                          ? "La copia está bien: cada archivo coincide con su huella."
                          : "La copia está dañada: no la uses para restaurar.",
                    )
                  }
                >
                  <ShieldCheck size={14} aria-hidden="true" /> Verificar
                </button>
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={locked}
                  aria-label={`Descargar copia del ${when(b.created_unix_ms)}`}
                  onClick={async () => {
                    const dest = await api.pickSavePath(`copia-${b.id}.zip`);
                    if (!dest) return;
                    setBusy(true);
                    setError(null);
                    setNote(null);
                    try {
                      await sup.downloadBackup(id, b.id, dest);
                      setNote(`Copia guardada en ${dest}`);
                    } catch (e) {
                      setError(errorMessage(e));
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  <Download size={14} aria-hidden="true" /> Descargar
                </button>
                <button
                  type="button"
                  className="btn btn-sm btn-go"
                  disabled={locked || !stopped || b.verified === false}
                  title={stopped ? undefined : "Detén el servidor para restaurar"}
                  aria-label={`Restaurar copia del ${when(b.created_unix_ms)}`}
                  onClick={() => setAsk({ kind: "restore", backup: b })}
                >
                  <ArchiveRestore size={14} aria-hidden="true" /> Restaurar
                </button>
                <button
                  type="button"
                  className="icon-btn"
                  disabled={locked}
                  aria-label={`${b.protected ? "Quitar protección a" : "Proteger"} copia del ${when(b.created_unix_ms)}`}
                  title={b.protected ? "Quitar protección" : "Proteger (no se borra al limpiar copias viejas)"}
                  onClick={() =>
                    run(
                      () => sup.protectBackup(id, b.id, !b.protected),
                      () => (b.protected ? "Ya no está protegida." : "Protegida: no se borrará al limpiar copias viejas."),
                    )
                  }
                >
                  {b.protected ? <LockOpen size={16} aria-hidden="true" /> : <Lock size={16} aria-hidden="true" />}
                </button>
                <button
                  type="button"
                  className="icon-btn"
                  disabled={locked || b.protected}
                  aria-label={`Borrar copia del ${when(b.created_unix_ms)}`}
                  onClick={() => setAsk({ kind: "delete", backup: b })}
                >
                  <Trash2 size={16} aria-hidden="true" />
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}
      {!stopped && items.length > 0 && <p className="field-help">Para restaurar una copia, detén antes el servidor.</p>}

      {list && (
        <section className="card props-card" aria-label="Ajustes de copias">
          <h2>Limpieza automática</h2>
          <div className="field prop-row">
            <label htmlFor="bk-keep">Copias que se conservan (las protegidas no cuentan)</label>
            <div className="files-bar">
              <input
                id="bk-keep"
                type="text"
                inputMode="numeric"
                value={keep}
                onChange={(e) => setKeep(e.target.value)}
                aria-invalid={!keepValid}
              />
              <button
                type="button"
                className="btn btn-sm"
                disabled={locked || !keepValid || keepNum === list.keep}
                onClick={() => run(() => sup.setBackupSettings(id, keepNum), (l) => `Se conservarán las ${l.keep} copias más nuevas.`)}
              >
                Guardar
              </button>
            </div>
            {!keepValid ? (
              <p className="form-error">Un número entre 1 y 500.</p>
            ) : (
              <p className="field-help">Al hacer una copia nueva, se borran las más viejas que pasen de este número.</p>
            )}
          </div>
          <p className="field-help">
            {list.free_bytes !== null ? `Espacio libre en el disco: ${formatSize(list.free_bytes)}.` : ""} Carpeta:{" "}
            <span className="mono">{list.folder}</span>
          </p>
        </section>
      )}

      {ask?.kind === "create" && (
        <Dialog
          title={ask.scope === "world" ? "Copia del mundo" : "Copia completa"}
          lead={
            online
              ? "Se pausa el guardado del juego, se guarda todo y se copia. Al terminar se reanuda solo."
              : "El servidor está detenido: la copia es exacta."
          }
          onClose={() => setAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                onClick={() => {
                  const a = ask;
                  setAsk(null);
                  run(
                    () => sup.createBackup(id, a.scope, a.note.trim() || null),
                    (l) => {
                      const b = l.items[0];
                      return b && b.hot && !b.consistent
                        ? "Copia hecha, pero el juego no confirmó que guardó: puede tener cambios a medias."
                        : "Copia hecha y comprobada.";
                    },
                  );
                }}
              >
                Hacer copia
              </button>
            </>
          }
        >
          <div className="field">
            <label htmlFor="bk-note">Nota (opcional)</label>
            <input
              id="bk-note"
              type="text"
              maxLength={200}
              placeholder="Antes de instalar mods nuevos"
              value={ask.note}
              onChange={(e) => setAsk({ ...ask, note: e.target.value })}
            />
          </div>
        </Dialog>
      )}

      {ask?.kind === "restore" && (
        <Dialog
          title={`Restaurar la copia del ${when(ask.backup.created_unix_ms)}`}
          lead={
            ask.backup.scope === "world"
              ? `Se reemplaza el mundo «${ask.backup.world ?? "world"}» (y su Nether y End) por el de la copia.`
              : "Se reemplazan los archivos y carpetas del servidor que están en la copia. Lo que no está en la copia no se toca."
          }
          onClose={() => setAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-warn"
                onClick={() => {
                  const b = ask.backup;
                  setAsk(null);
                  run(
                    () => sup.restoreBackup(id, b.id),
                    () => "Copia restaurada. Lo que había antes quedó guardado como copia «Antes de restaurar».",
                  );
                }}
              >
                Restaurar
              </button>
            </>
          }
        >
          <p>
            Antes de cambiar nada, MercHosting hace una copia de lo que hay ahora («Antes de restaurar») y comprueba la copia
            elegida. Si algo falla a medias, el servidor queda como estaba.
          </p>
        </Dialog>
      )}

      {ask?.kind === "delete" && (
        <Dialog
          title="Borrar copia"
          lead={`La copia del ${when(ask.backup.created_unix_ms)} (${formatSize(ask.backup.size_bytes)}) se borra del disco. No se puede deshacer.`}
          onClose={() => setAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  const b = ask.backup;
                  setAsk(null);
                  run(() => sup.deleteBackup(id, b.id), () => "Copia borrada.");
                }}
              >
                Borrar copia
              </button>
            </>
          }
        />
      )}
    </div>
  );
}
