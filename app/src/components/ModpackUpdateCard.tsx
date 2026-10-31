// Actualizar el modpack del servidor. Se ve qué cambia antes de tocar nada, se hace una copia
// completa y se respeta lo que cambió la persona (configs editados, mods quitados o propios).

import { Link2, Package, Search, Unlink } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type {
  ContentHit,
  ContentSource,
  ContentVersion,
  InstanceId,
  JobSnapshot,
  ModpackStatusInfo,
  ModpackUpdatePlan,
  PackChangeKind,
} from "../api/types";
import { formatSize } from "../format";
import { Dialog } from "./Dialog";

const CHANGE: Record<PackChangeKind, { label: string; help: string }> = {
  add: { label: "Nuevos", help: "Archivos que trae la versión nueva." },
  replace: { label: "Se actualizan", help: "Archivos del modpack que no habías tocado." },
  remove: { label: "Se quitan", help: "La versión nueva ya no los trae. Van a la papelera de MercHosting." },
  keep_modified: { label: "Se conserva tu versión", help: "Los cambiaste tú (por ejemplo, un config editado): no se tocan." },
  keep_user_file: { label: "Archivos tuyos", help: "Hay un archivo tuyo donde el modpack quiere poner uno: se conserva el tuyo." },
  skip_removed: { label: "No se vuelven a poner", help: "Los quitaste o desactivaste tú." },
};
const ORDER: PackChangeKind[] = ["add", "replace", "remove", "keep_modified", "keep_user_file", "skip_removed"];
const SOURCE: Record<ContentSource, string> = { modrinth: "Modrinth", curseforge: "CurseForge", hangar: "Hangar" };

interface Props {
  sup: Supervisor;
  id: InstanceId;
  running: boolean;
  mcVersion: string | null;
  onChanged: () => void;
  pollMs?: number;
}

export function ModpackUpdateCard({ sup, id, running, mcVersion, onChanged, pollMs = 700 }: Props) {
  const [st, setSt] = useState<ModpackStatusInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pick, setPick] = useState<string>("");
  const [plan, setPlan] = useState<ModpackUpdatePlan | null>(null);
  const [planBusy, setPlanBusy] = useState(false);
  const [accept, setAccept] = useState(false);
  const [job, setJob] = useState<JobSnapshot | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const [linking, setLinking] = useState(false);
  const alive = useRef(true);
  useEffect(
    () => () => {
      alive.current = false;
    },
    [],
  );

  const load = useCallback(async () => {
    setError(null);
    try {
      const s = await sup.modpackStatus(id);
      setSt(s);
      const next = s.versions.find((v) => v.newer && v.compatible);
      setPick(next?.version_id ?? "");
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);
  useEffect(() => {
    void load();
  }, [load]);

  async function preview() {
    if (!pick) return;
    setPlanBusy(true);
    setError(null);
    setAccept(false);
    try {
      setPlan(await sup.modpackUpdatePreview(id, pick));
    } catch (e) {
      setError(errorMessage(e));
    }
    setPlanBusy(false);
  }

  async function apply() {
    if (!plan) return;
    const p = plan;
    setPlan(null);
    setDone(null);
    setError(null);
    try {
      let j = await sup.modpackUpdateApply(id, p.to_version_id, accept);
      setJob(j);
      while (alive.current && j.state === "running") {
        await new Promise((r) => setTimeout(r, pollMs));
        j = await sup.job(j.job_id, 0);
        if (alive.current) setJob(j);
      }
      if (!alive.current) return;
      setJob(null);
      if (j.state === "succeeded") {
        setDone(j.message);
        onChanged();
        await load();
      } else setError(j.error ?? j.message);
    } catch (e) {
      setJob(null);
      setError(errorMessage(e));
    }
  }

  const linked = st?.linked ?? null;
  const compatible = st?.versions.filter((v) => v.compatible && !v.current) ?? [];
  const newest = st?.versions.find((v) => v.newer && v.compatible);
  const counts = (k: PackChangeKind) => plan?.changes.filter((c) => c.kind === k) ?? [];

  return (
    <section className="props-card" aria-label="Modpack">
      <h3>
        <Package size={16} aria-hidden="true" /> Modpack
      </h3>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {done && (
        <p className="field-help ok-text" role="status">
          {done}
        </p>
      )}
      {!st && !error && <p className="field-help">Buscando…</p>}
      {st && !linked && (
        <>
          <p className="field-help">
            MercHosting no sabe qué modpack tiene este servidor. Los servidores creados desde un modpack con la versión 0.2.4 o
            posterior lo recuerdan solos. Si lo creaste antes (o lo importaste), dile cuál es y qué versión tienes.
          </p>
          <button type="button" className="btn btn-sm" onClick={() => setLinking(true)}>
            <Link2 size={14} aria-hidden="true" /> Vincular un modpack
          </button>
        </>
      )}
      {linked && (
        <>
          <p>
            <strong>{linked.title}</strong> {linked.version_number}{" "}
            <span className="field-help">
              · {SOURCE[linked.source]} · Minecraft {linked.mc_version}
              {linked.loader ? ` · ${linked.loader} ${linked.loader_version ?? ""}` : ""}
            </span>
          </p>
          {!st?.online && <p className="field-help warn-text">No se pudo consultar {SOURCE[linked.source]}: {st?.error}</p>}
          {st?.online && (
            <p className="field-help">
              {newest ? `Hay una versión más nueva: ${newest.version_number}.` : "Tienes la versión más nueva para esta versión de Minecraft."}
            </p>
          )}
          {compatible.length > 0 && (
            <div className="files-bar">
              <label className="sr-only" htmlFor="mp-version">
                Versión del modpack
              </label>
              <select id="mp-version" value={pick} onChange={(e) => setPick(e.target.value)} disabled={!!job || planBusy}>
                <option value="">Elige una versión…</option>
                {compatible.map((v) => (
                  <option key={v.version_id} value={v.version_id}>
                    {v.version_number}
                    {v.newer ? " (más nueva)" : " (anterior)"}
                  </option>
                ))}
              </select>
              <button type="button" className="btn btn-sm btn-go" disabled={!pick || !!job || planBusy} onClick={preview}>
                {planBusy ? "Comparando…" : "Ver qué cambia"}
              </button>
            </div>
          )}
          {st && st.versions.some((v) => !v.compatible) && (
            <p className="field-help">
              Las versiones para otra versión de Minecraft o de loader no se ofrecen: cambiarlas en un servidor existente puede romper el
              mundo. Para esas, crea un servidor nuevo desde el modpack.
            </p>
          )}
          <button
            type="button"
            className="link-btn"
            onClick={async () => {
              try {
                setSt(await sup.modpackUnlink(id));
              } catch (e) {
                setError(errorMessage(e));
              }
            }}
          >
            <Unlink size={13} aria-hidden="true" /> Olvidar este modpack
          </button>
        </>
      )}
      {job && (
        <div className="job-progress" role="status">
          <p>{job.message || "Actualizando…"}</p>
          {job.download && (
            <p className="field-help">
              {job.download.file} · {formatSize(job.download.done)}
              {job.download.total ? ` de ${formatSize(job.download.total)}` : ""}
            </p>
          )}
        </div>
      )}

      {plan && (
        <Dialog
          wide
          title={`Actualizar a ${plan.to_version}`}
          lead={`De ${plan.from_version} a ${plan.to_version} · Minecraft ${plan.mc_version}. Antes de cambiar nada se hace una copia completa del servidor; si algo sale mal, restáurala en Copias de seguridad.`}
          onClose={() => setPlan(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setPlan(null)}>
                Cancelar
              </button>
              <button type="button" className="btn btn-go" disabled={running || (plan.loader_mismatch && !accept)} onClick={apply}>
                Actualizar (con copia previa)
              </button>
            </>
          }
        >
          {running && <p className="field-help warn-text">Detén el servidor para actualizar el modpack.</p>}
          <p className="field-help">Descarga: {formatSize(plan.download_bytes)} desde los enlaces oficiales del modpack.</p>
          {plan.notes.map((n) => (
            <p key={n} className="field-help warn-text">
              {n}
            </p>
          ))}
          {plan.loader_mismatch && (
            <label className="switch-row">
              <input type="checkbox" checked={accept} onChange={(e) => setAccept(e.target.checked)} />
              <span>
                Entiendo que el loader no cambia ({plan.loader} {plan.loader_from} en vez de {plan.loader_to}) y que, si no arranca,
                restauraré la copia previa.
              </span>
            </label>
          )}
          {plan.changes.length === 0 ? (
            <p className="field-help">No cambia ningún archivo del servidor.</p>
          ) : (
            <div className="mp-changes">
              {ORDER.filter((k) => counts(k).length).map((k) => (
                <details key={k} className="mp-group">
                  <summary>
                    <b>{counts(k).length}</b> {CHANGE[k].label.toLowerCase()} <span className="field-help">· {CHANGE[k].help}</span>
                  </summary>
                  <ul>
                    {counts(k).map((c) => (
                      <li key={c.path} className="mono">
                        {c.path}
                      </li>
                    ))}
                  </ul>
                </details>
              ))}
            </div>
          )}
        </Dialog>
      )}

      {linking && (
        <LinkDialog
          sup={sup}
          id={id}
          mcVersion={mcVersion}
          onClose={() => setLinking(false)}
          onLinked={(s) => {
            setLinking(false);
            setSt(s);
            setPick(s.versions.find((v) => v.newer && v.compatible)?.version_id ?? "");
          }}
        />
      )}
    </section>
  );
}

function LinkDialog({
  sup,
  id,
  mcVersion,
  onClose,
  onLinked,
}: {
  sup: Supervisor;
  id: InstanceId;
  mcVersion: string | null;
  onClose: () => void;
  onLinked: (s: ModpackStatusInfo) => void;
}) {
  const [source, setSource] = useState<ContentSource>("modrinth");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ContentHit[] | null>(null);
  const [project, setProject] = useState<{ title: string; versions: ContentVersion[] } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run<T>(fn: () => Promise<T>, then: (v: T) => void) {
    setBusy(true);
    setError(null);
    try {
      then(await fn());
    } catch (e) {
      setError(errorMessage(e));
    }
    setBusy(false);
  }

  return (
    <Dialog
      wide
      title="Vincular un modpack"
      lead="Elige el modpack con el que creaste este servidor y la versión que tienes instalada. MercHosting comparará sus archivos con los del servidor; no cambia nada."
      onClose={onClose}
      footer={
        <button type="button" className="btn" onClick={onClose}>
          Cancelar
        </button>
      }
    >
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {project ? (
        <>
          <p>
            <strong>{project.title}</strong> · elige tu versión:
          </p>
          <ul className="rp-versions" aria-label="Versiones del modpack">
            {project.versions.map((v) => {
              const fits = !mcVersion || v.game_versions.includes(mcVersion);
              return (
                <li key={v.id} className="rp-version" data-fits={fits}>
                  <span className="rp-version-main">
                    <b>{v.version_number}</b>
                    <span className="field-help">Minecraft {v.game_versions.slice(0, 4).join(", ")}</span>
                  </span>
                  <button
                    type="button"
                    className="btn btn-sm btn-go"
                    disabled={busy || !fits}
                    aria-label={`Tengo la ${v.version_number}`}
                    onClick={() => run(() => sup.modpackLink(id, source, v.id), onLinked)}
                  >
                    Tengo esta
                  </button>
                </li>
              );
            })}
          </ul>
          <button type="button" className="btn btn-sm btn-quiet" onClick={() => setProject(null)}>
            Volver a buscar
          </button>
        </>
      ) : (
        <>
          <div className="files-bar">
            <div className="seg" role="group" aria-label="Catálogo">
              {(["modrinth", "curseforge"] as ContentSource[]).map((s) => (
                <button key={s} type="button" className="seg-btn" aria-pressed={source === s} onClick={() => (setSource(s), setHits(null))}>
                  {SOURCE[s]}
                </button>
              ))}
            </div>
          </div>
          <form
            className="console-search rp-search"
            role="search"
            onSubmit={(e) => {
              e.preventDefault();
              void run(() => sup.modpackSearch(source, query, "relevance"), (r) => setHits(r.hits));
            }}
          >
            <Search size={15} aria-hidden="true" />
            <input type="search" aria-label="Buscar el modpack" placeholder="Nombre del modpack" value={query} onChange={(e) => setQuery(e.target.value)} />
          </form>
          {hits && hits.length === 0 && <p className="field-help">No se encontró nada.</p>}
          {hits && (
            <ul className="rp-hits" aria-label="Modpacks">
              {hits.map((h) => (
                <li key={h.project_id}>
                  <button
                    type="button"
                    className="rp-hit"
                    disabled={busy}
                    onClick={() => run(() => sup.modpackProject(source, h.project_id), (d) => setProject({ title: d.project.title, versions: d.versions }))}
                  >
                    <span className="rp-hit-main">
                      <b>{h.title}</b>
                      <span className="field-help">{h.description}</span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </Dialog>
  );
}
