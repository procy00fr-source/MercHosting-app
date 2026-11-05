// Pestaña Addons (solo servidores Bedrock): behavior packs y resource packs.
// Se instalan desde un .mcpack/.mcaddon de esta PC (botón o arrastrando) o desde CurseForge
// (si la instalación tiene la clave oficial; la descarga se comprueba con su huella), se activan
// en el mundo del servidor y se pueden desactivar o quitar (a la papelera, nunca se borran).

import { Boxes, ExternalLink, Package, Search, TriangleAlert, Upload } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { AddonImportResult, AddonPack, AddonsInfo, InstanceId, SearchPage } from "../api/types";
import { baseName, useFileDrop } from "../useFileDrop";
import { Dialog } from "./Dialog";

interface Props {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  running: boolean;
  remote: boolean;
  /** Puede activar, desactivar y quitar (permiso de contenido). */
  canManage: boolean;
}

const ADDON_EXT = ["mcpack", "mcaddon"];

export function isAddonFile(path: string): boolean {
  const l = path.toLowerCase();
  return ADDON_EXT.some((e) => l.endsWith(`.${e}`));
}

/** Una frase por resultado (cada una se traduce por separado). */
export function importSummary(r: AddonImportResult): string[] {
  const parts: string[] = [];
  if (r.installed.length) parts.push(`Instalado y activado: ${r.installed.join(", ")}.`);
  if (r.replaced.length) parts.push(`Actualizado (la versión anterior está en la papelera): ${r.replaced.join(", ")}.`);
  if (r.unchanged.length) parts.push(`Ya estaba con la misma versión: ${r.unchanged.join(", ")}.`);
  return parts;
}

function PackRow({
  p,
  disabled,
  canManage,
  onToggle,
  onRemove,
}: {
  p: AddonPack;
  disabled: boolean;
  canManage: boolean;
  onToggle: (on: boolean) => void;
  onRemove: () => void;
}) {
  return (
    <li className="addon-row">
      <label className="switch-row">
        <input
          type="checkbox"
          checked={p.active}
          disabled={disabled || !canManage}
          aria-label={`Activo en el mundo: ${p.name}`}
          onChange={(e) => onToggle(e.target.checked)}
        />
        <span>
          <span className="addon-meta">
            <strong translate="no">{p.name}</strong>
            <span className="badge badge-muted">{p.kind === "behavior" ? "Comportamiento" : "Recursos"}</span>
            {p.scripts && <span className="badge">Scripts</span>}
            <span className="field-help">v{p.version.join(".")}</span>
          </span>
          {p.description && (
            <span className="field-help" translate="no">
              {p.description}
            </span>
          )}
        </span>
      </label>
      {canManage && (
        <button type="button" className="btn btn-sm btn-quiet" disabled={disabled} onClick={onRemove} aria-label={`Quitar ${p.name}`}>
          Quitar
        </button>
      )}
    </li>
  );
}

/** Página de un addon de Bedrock en CurseForge. */
export function curseforgeAddonUrl(slug: string): string {
  return `https://www.curseforge.com/minecraft-bedrock/addons/${encodeURIComponent(slug)}`;
}

function CurseForgeAddons({
  api,
  sup,
  id,
  disabled,
  canManage,
  running,
  onInstalled,
}: {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  disabled: boolean;
  canManage: boolean;
  running: boolean;
  onInstalled: (r: AddonsInfo) => void;
}) {
  const [query, setQuery] = useState("");
  const [page, setPage] = useState<SearchPage | null>(null);
  const [searching, setSearching] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function search(q: string, offset = 0) {
    setSearching(true);
    setError(null);
    try {
      const r = await sup.addonSearch(id, q, q ? "relevance" : "downloads", offset);
      setPage((old) => (offset > 0 && old ? { ...r, hits: [...old.hits, ...r.hits] } : r));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSearching(false);
    }
  }

  async function install(projectId: string) {
    setInstalling(projectId);
    setError(null);
    try {
      onInstalled(await sup.addonInstallCurseforge(id, projectId));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setInstalling(null);
    }
  }

  return (
    <section className="card props-card" aria-label="Buscar addons en CurseForge">
      <h2>Buscar addons en CurseForge</h2>
      <form
        className="join-form"
        onSubmit={(e) => {
          e.preventDefault();
          void search(query.trim());
        }}
      >
        <label className="field">
          <span className="field-title">Nombre del addon</span>
          <input type="search" value={query} onChange={(e) => setQuery(e.target.value)} maxLength={100} placeholder="Por ejemplo: muebles" />
        </label>
        <button type="submit" className="btn btn-sm" disabled={searching}>
          <Search size={14} aria-hidden="true" /> {query.trim() ? "Buscar" : "Ver populares"}
        </button>
      </form>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {searching && page === null && <p className="wiz-loading">Buscando…</p>}
      {page &&
        (page.hits.length === 0 ? (
          <p className="field-help">No hay addons con ese nombre.</p>
        ) : (
          <ul className="addon-list" aria-label="Resultados de CurseForge">
            {page.hits.map((h) => (
              <li key={h.project_id} className="addon-row">
                <span>
                  <span className="addon-meta">
                    <strong translate="no">{h.title}</strong>
                    <span className="field-help">
                      de <span translate="no">{h.author}</span> · {h.downloads.toLocaleString("es")} descargas
                    </span>
                  </span>
                  {h.description && (
                    <span className="field-help" translate="no">
                      {h.description}
                    </span>
                  )}
                </span>
                <span className="row-actions">
                  {canManage && (
                    <button
                      type="button"
                      className="btn btn-sm btn-go"
                      disabled={disabled || installing !== null}
                      title={running ? "Detén el servidor para instalar addons." : undefined}
                      aria-label={`Instalar ${h.title}`}
                      onClick={() => void install(h.project_id)}
                    >
                      {installing === h.project_id ? "Instalando…" : "Instalar"}
                    </button>
                  )}
                  <button
                    type="button"
                    className="btn btn-sm btn-quiet"
                    aria-label={`Ver ${h.title} en CurseForge`}
                    onClick={() => api.openUrl(curseforgeAddonUrl(h.slug))}
                  >
                    <ExternalLink size={14} aria-hidden="true" />
                  </button>
                </span>
              </li>
            ))}
          </ul>
        ))}
      {page && page.hits.length < page.total && (
        <button type="button" className="btn btn-sm" disabled={searching} onClick={() => void search(query.trim(), page.hits.length)}>
          Ver más
        </button>
      )}
      <p className="field-help">
        Se instala la versión más nueva del addon y se comprueba con su huella antes de activarla. Si el autor no permite descargarlo
        desde otras apps, ábrelo en CurseForge, descárgalo y arrástralo aquí.
      </p>
    </section>
  );
}

export function AddonsPanel({ api, sup, id, running, remote, canManage }: Props) {
  const [info, setInfo] = useState<AddonsInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [msgs, setMsgs] = useState<string[]>([]);
  const [notes, setNotes] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [removing, setRemoving] = useState<AddonPack | null>(null);

  const load = useCallback(async () => {
    try {
      setInfo(await sup.addons(id));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  useEffect(() => {
    load();
  }, [load]);

  async function run(f: () => Promise<AddonsInfo>) {
    setBusy(true);
    setError(null);
    try {
      const r = await f();
      setInfo(r);
      return r;
    } catch (e) {
      setError(errorMessage(e));
      return null;
    } finally {
      setBusy(false);
    }
  }

  async function importFiles(paths: string[]) {
    setMsgs([]);
    setNotes([]);
    const skipped = paths.filter((p) => !isAddonFile(p)).map(baseName);
    const msgs: string[] = [];
    const extra: string[] = [];
    for (const p of paths.filter(isAddonFile)) {
      const r = await run(() => sup.addonImport(id, p));
      if (r?.last_import) {
        msgs.push(...importSummary(r.last_import));
        extra.push(...r.last_import.notes);
      }
    }
    if (skipped.length) msgs.push(`Ignorados (no son .mcpack ni .mcaddon): ${skipped.join(", ")}.`);
    setMsgs(msgs);
    setNotes(extra);
  }

  async function pick() {
    const p = await api.pickFile({ name: "Addons de Minecraft Bedrock", extensions: ADDON_EXT });
    if (p) await importFiles([p]);
  }

  const canImport = !remote;
  const drop = useFileDrop(api, canImport, (paths) => {
    if (running) {
      setError("Detén el servidor para instalar addons.");
      return;
    }
    void importFiles(paths);
  });

  const own = info?.packs.filter((p) => !p.bundled) ?? [];
  const bundled = info?.packs.filter((p) => p.bundled) ?? [];

  return (
    <div className="admin-panel drop-zone" role="tabpanel" aria-label="Addons" data-dragging={drop.dragging}>
      {drop.dragging && (
        <div className="drop-overlay" role="status">
          {running ? <p>Detén el servidor para instalar addons.</p> : <p>Suelta tus .mcpack o .mcaddon para instalarlos y activarlos en el mundo.</p>}
        </div>
      )}
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">Addons</h2>
          <p className="field-help">
            Behavior packs (comportamiento) y resource packs (texturas y sonidos) de Minecraft Bedrock. Los jugadores los descargan
            solos al entrar.
          </p>
        </div>
        {canImport && (
          <button type="button" className="btn btn-go" disabled={busy || running} title={running ? "Detén el servidor para instalar addons." : undefined} onClick={pick}>
            <Upload size={15} aria-hidden="true" /> Instalar addon
          </button>
        )}
      </div>
      {running && <p className="field-help">Detén el servidor para instalar, activar o quitar addons.</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {msgs.map((m) => (
        <p key={m} className="ok-text" role="status">
          {m}
        </p>
      ))}
      {notes.map((n) => (
        <p key={n} className="warn-text">
          <TriangleAlert size={13} aria-hidden="true" /> {n}
        </p>
      ))}

      {info === null ? (
        !error && <p className="wiz-loading">Leyendo los addons…</p>
      ) : (
        <>
          {!info.world_exists && (
            <p className="field-help">
              {`El mundo «${info.level_name}» se crea la primera vez que enciendas el servidor; los addons que instales ya quedan activados para él.`}
            </p>
          )}
          <section className="card props-card" aria-label="Tus addons">
            <h2>
              <Package size={16} aria-hidden="true" /> Tus addons
            </h2>
            {own.length === 0 ? (
              <p className="field-help">
                {canImport ? "Todavía no hay addons. Pulsa «Instalar addon» o arrastra aquí un .mcpack o .mcaddon." : "Todavía no hay addons."}
              </p>
            ) : (
              <ul className="addon-list">
                {own.map((p) => (
                  <PackRow
                    key={p.uuid}
                    p={p}
                    disabled={busy || running}
                    canManage={canManage}
                    onToggle={(on) => void run(() => sup.addonSetActive(id, p.uuid, on))}
                    onRemove={() => setRemoving(p)}
                  />
                ))}
              </ul>
            )}
            <p className="field-help">
              Solo instala addons de fuentes en las que confíes. Los que usan scripts pueden necesitar activar experimentos en el
              mundo.
            </p>
          </section>

          {bundled.length > 0 && (
            <details className="card props-card">
              <summary>
                <Boxes size={15} aria-hidden="true" /> {`Incluidos con el servidor (${bundled.length})`}
              </summary>
              <p className="field-help">Paquetes que trae el propio servidor de Mojang. Se muestran solo para consultar.</p>
              <ul className="plain-list">
                {bundled.map((p) => (
                  <li key={p.uuid}>
                    <span translate="no">{p.name}</span> · v{p.version.join(".")}
                  </li>
                ))}
              </ul>
            </details>
          )}

          {info.curseforge ? (
            <CurseForgeAddons
              api={api}
              sup={sup}
              id={id}
              running={running}
              canManage={canManage}
              disabled={busy || running}
              onInstalled={(r) => {
                setInfo(r);
                setMsgs(r.last_import ? importSummary(r.last_import) : []);
                setNotes(r.last_import?.notes ?? []);
              }}
            />
          ) : (
            <section className="card props-card" aria-label="Buscar addons">
              <h2>Buscar addons gratis</h2>
              <button
                type="button"
                className="btn btn-go"
                disabled
                title="Estará disponible cuando MercHosting tenga su clave oficial de CurseForge"
              >
                Buscar en CurseForge <span className="badge badge-muted">Próximamente</span>
              </button>
              <p className="field-help">
                Buscar e instalar addons de CurseForge desde aquí llega cuando MercHosting tenga su clave oficial. Mientras tanto,
                descárgalos de su página y arrástralos a esta pestaña.
              </p>
            </section>
          )}
        </>
      )}

      {removing && (
        <Dialog
          title={`¿Quitar ${removing.name}?`}
          lead="Se desactiva en el mundo y su carpeta va a la papelera de MercHosting (la puedes recuperar desde Archivos)."
          onClose={() => setRemoving(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setRemoving(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                disabled={busy}
                onClick={async () => {
                  const uuid = removing.uuid;
                  setRemoving(null);
                  await run(() => sup.addonRemove(id, uuid));
                }}
              >
                Quitar
              </button>
            </>
          }
        >
          <p className="field-help">Si el mundo guardó datos de este addon, se conservan; al volver a instalarlo los recupera.</p>
        </Dialog>
      )}
    </div>
  );
}
