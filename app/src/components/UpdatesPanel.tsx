// Pestaña Actualizaciones: el software del servidor (compilación o loader, misma versión de
// Minecraft) y los mods, plugins y datapacks con versión nueva. Antes de cambiar nada se hace
// una copia completa; para volver atrás se restaura esa copia en «Copias de seguridad».

import { ArrowUpCircle, RefreshCw, RotateCcw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { ContentKind, InstalledItem, InstanceId, JobSnapshot, Perm, Platform, ServerUpdateInfo } from "../api/types";
import { formatSize } from "../format";
import { Dialog } from "./Dialog";
import { ModpackUpdateCard } from "./ModpackUpdateCard";

const KIND_LABEL: Record<ContentKind, string> = { mod: "Mod", plugin: "Plugin", datapack: "Datapack" };

interface Pending {
  kind: ContentKind;
  item: InstalledItem;
}

interface Props {
  sup: Supervisor;
  id: InstanceId;
  running: boolean;
  platforms: Platform[];
  /** Tipo de servidor, para nombrar bien lo que se actualiza. */
  kind?: string;
  can?: (p: Perm) => boolean;
  /** Servidor de un amigo: actualizar el software es cosa del dueño. */
  remote?: boolean;
  onOpenTab: (tab: string) => void;
  onChanged: () => void;
  pollMs?: number;
  mcVersion?: string | null;
}

export function UpdatesPanel({ sup, id, running, platforms, kind, can = () => true, remote = false, onOpenTab, onChanged, pollMs = 700, mcVersion = null }: Props) {
  const [server, setServer] = useState<ServerUpdateInfo | null>(null);
  const [serverError, setServerError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [job, setJob] = useState<JobSnapshot | null>(null);
  const [jobError, setJobError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);

  const [pending, setPending] = useState<Pending[] | null>(null);
  const [contentError, setContentError] = useState<string | null>(null);
  const [backupFirst, setBackupFirst] = useState(true);
  const [bulk, setBulk] = useState<{ current: string; ok: string[]; skipped: string[] } | null>(null);
  const [bulkBusy, setBulkBusy] = useState(false);
  const alive = useRef(true);
  useEffect(
    () => () => {
      alive.current = false;
    },
    [],
  );

  const checkServer = useCallback(async () => {
    setChecking(true);
    setServerError(null);
    try {
      setServer(await sup.serverUpdateCheck(id));
    } catch (e) {
      setServerError(errorMessage(e));
    } finally {
      setChecking(false);
    }
  }, [sup, id]);

  const kinds = platforms.map((p) => p.kind).join(",");
  const loadContent = useCallback(async () => {
    if (!can("content")) return setPending([]);
    setContentError(null);
    try {
      const all: Pending[] = [];
      for (const kind of kinds ? (kinds.split(",") as ContentKind[]) : []) {
        const l = await sup.contentInstalled(id, kind);
        if (!l.online && l.offline_reason) setContentError(l.offline_reason);
        for (const item of l.items) if (item.update && item.enabled && item.project_id) all.push({ kind, item });
      }
      setPending(all);
    } catch (e) {
      setContentError(errorMessage(e));
      setPending([]);
    }
  }, [sup, id, kinds]);

  useEffect(() => {
    checkServer();
    loadContent();
  }, [checkServer, loadContent]);

  async function applyServer() {
    if (!server?.latest) return;
    setConfirm(false);
    setJobError(null);
    setDone(null);
    try {
      let j = await sup.applyServerUpdate(id, server.latest);
      setJob(j);
      while (alive.current && j.state === "running") {
        await new Promise((r) => setTimeout(r, pollMs));
        j = await sup.job(j.job_id, 0);
        if (alive.current) setJob(j);
      }
      if (!alive.current) return;
      if (j.state === "succeeded") {
        setDone(j.message);
        onChanged();
        await checkServer();
      } else setJobError(j.error ?? j.message);
      setJob(null);
    } catch (e) {
      setJob(null);
      setJobError(errorMessage(e));
    }
  }

  async function updateAll() {
    if (!pending?.length) return;
    setBulkBusy(true);
    setContentError(null);
    const ok: string[] = [];
    const skipped: string[] = [];
    try {
      if (backupFirst) {
        setBulk({ current: "Haciendo una copia completa…", ok, skipped });
        await sup.createBackup(id, "full", "Antes de actualizar mods, plugins y datapacks");
      }
      for (const { kind, item } of pending) {
        const name = item.title ?? item.file_name;
        setBulk({ current: `Actualizando ${name}…`, ok: [...ok], skipped: [...skipped] });
        const source = item.source ?? "modrinth";
        try {
          const plan = await sup.contentPlan(id, kind, source, item.update!.version_id);
          if (plan.blockers.length) {
            skipped.push(`${name}: ${plan.blockers[0]}`);
            continue;
          }
          await sup.contentInstall(id, kind, source, item.update!.version_id);
          ok.push(`${name} ${item.update!.version_number}`);
        } catch (e) {
          skipped.push(`${name}: ${errorMessage(e)}`);
        }
      }
      setBulk({ current: "", ok, skipped });
    } catch (e) {
      setContentError(`No se hizo la copia previa, así que no se actualizó nada: ${errorMessage(e)}`);
      setBulk(null);
    } finally {
      setBulkBusy(false);
      await loadContent();
    }
  }

  const ownerOnly = remote;
  const serverBusy = job !== null;

  return (
    <div className="admin-panel updates-panel" role="tabpanel" aria-label="Actualizaciones">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">
            <ArrowUpCircle size={18} aria-hidden="true" /> Actualizaciones
          </h2>
          <p className="field-help">
            Antes de cambiar algo se hace una copia completa. Si algo sale mal, restáurala en Copias de seguridad y todo vuelve a como
            estaba.
          </p>
        </div>
        <button type="button" className="btn btn-sm btn-quiet" disabled={checking || serverBusy || bulkBusy} onClick={() => (checkServer(), loadContent())}>
          <RefreshCw size={14} aria-hidden="true" /> Buscar de nuevo
        </button>
      </div>

      <section className="props-card" aria-label="Software del servidor">
        <h3>Software del servidor</h3>
        {serverError && (
          <p className="form-error" role="alert">
            {serverError}
          </p>
        )}
        {!server && !serverError && <p className="field-help">Buscando…</p>}
        {server && !server.supported && <p className="field-help">{server.reason}</p>}
        {server?.supported && (
          <>
            <p>
              Minecraft {server.mc_version}: tienes {kind === "fabric" ? "el loader " : kind === "paper" || kind === "purpur" ? "la compilación " : ""}
              <strong>{server.current ?? "una versión desconocida"}</strong>
              {server.newer ? (
                <>
                  {" "}
                  · hay una más nueva: <strong>{server.latest}</strong>
                  {server.experimental && <span className="badge badge-warn"> experimental</span>}
                </>
              ) : (
                " · es la más nueva."
              )}
            </p>
            {server.notes.map((n) => (
              <p key={n} className="field-help">
                {n}
              </p>
            ))}
            {server.newer &&
              (ownerOnly ? (
                <p className="field-help">Solo el dueño del servidor puede actualizarlo.</p>
              ) : (
                <div className="row-actions">
                  <button type="button" className="btn btn-sm btn-go" disabled={running || serverBusy} onClick={() => setConfirm(true)}>
                    <ArrowUpCircle size={14} aria-hidden="true" /> Actualizar a {server.latest}
                  </button>
                  {running && <span className="field-help">Detén el servidor primero.</span>}
                </div>
              ))}
            <p className="field-help">Cambiar la versión de Minecraft todavía no se hace desde aquí: puede romper el mundo y los mods.</p>
          </>
        )}
        {job && (
          <p className="field-help" role="status">
            {job.message}
            {job.download ? ` · ${formatSize(job.download.done)}${job.download.total ? ` de ${formatSize(job.download.total)}` : ""}` : ""}
          </p>
        )}
        {done && (
          <div className="ok-text" role="status">
            <p>{done}.</p>
            {can("backups") && (
              <button type="button" className="btn btn-sm btn-quiet" onClick={() => onOpenTab("backups")}>
                <RotateCcw size={14} aria-hidden="true" /> Volver atrás (Copias de seguridad)
              </button>
            )}
          </div>
        )}
        {jobError && (
          <p className="form-error" role="alert">
            {jobError}
          </p>
        )}
      </section>

      {can("content") && platforms.some((p) => p.kind === "mod") && (
        <ModpackUpdateCard sup={sup} id={id} running={running} mcVersion={mcVersion} onChanged={() => (onChanged(), loadContent())} pollMs={pollMs} />
      )}

      {can("content") && platforms.length > 0 && (
        <section className="props-card" aria-label="Mods, plugins y datapacks">
          <h3>Mods, plugins y datapacks</h3>
          {contentError && (
            <p className="form-error" role="alert">
              {contentError}
            </p>
          )}
          {pending === null && <p className="field-help">Buscando…</p>}
          {pending?.length === 0 && !bulk && <p className="field-help">Todo está en su versión más nueva (de lo que vino de Modrinth o CurseForge).</p>}
          {pending && pending.length > 0 && (
            <>
              <ul className="update-list">
                {pending.map(({ kind, item }) => (
                  <li key={`${kind}:${item.file_name}`}>
                    <span className="badge">{KIND_LABEL[kind]}</span> <strong>{item.title ?? item.file_name}</strong> {item.version_number ?? ""} →{" "}
                    {item.update!.version_number}
                  </li>
                ))}
              </ul>
              <label className="check">
                <input type="checkbox" checked={backupFirst} disabled={bulkBusy} onChange={(e) => setBackupFirst(e.target.checked)} />
                Hacer una copia completa antes (recomendado)
              </label>
              <div className="row-actions">
                <button type="button" className="btn btn-sm btn-go" disabled={running || bulkBusy} onClick={updateAll}>
                  <ArrowUpCircle size={14} aria-hidden="true" /> {bulkBusy ? "Actualizando…" : `Actualizar todos (${pending.length})`}
                </button>
                {running && <span className="field-help">Detén el servidor primero.</span>}
              </div>
            </>
          )}
          {bulk && (
            <div role="status">
              {bulk.current && <p className="field-help">{bulk.current}</p>}
              {bulk.ok.length > 0 && <p className="ok-text">Actualizados: {bulk.ok.join(", ")}. Las versiones anteriores están en la papelera de MercHosting.</p>}
              {bulk.skipped.map((s) => (
                <p key={s} className="form-error">
                  No se actualizó {s}
                </p>
              ))}
            </div>
          )}
        </section>
      )}

      {confirm && server?.latest && (
        <Dialog
          title={`Actualizar a ${server.latest}`}
          onClose={() => setConfirm(false)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirm(false)}>
                Cancelar
              </button>
              <button type="button" className="btn btn-go" onClick={applyServer}>
                Hacer copia y actualizar
              </button>
            </>
          }
        >
          <p>
            Se hará una <strong>copia completa</strong> del servidor, se descargará {server.latest} de la fuente oficial (comprobando su
            huella) y el archivo anterior irá a la papelera de MercHosting.
          </p>
          <p>Si después algo no funciona, restaura esa copia en Copias de seguridad.</p>
        </Dialog>
      )}
    </div>
  );
}
