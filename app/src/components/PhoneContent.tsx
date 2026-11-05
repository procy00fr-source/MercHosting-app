// Mods y plugins de un servidor del celular, desde Modrinth (API pública, sin clave) y, para
// mods, desde CurseForge cuando la app se compiló con su clave oficial (si no, «Próximamente»).
// Instala con el plan verificado de merc_phone::content (dependencias + hash de cada descarga);
// lo que se reemplaza o se quita va a la papelera de la app.

import { ExternalLink, Search, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi } from "../api/client";
import { errorMessage } from "../api/client";
import type { PhoneApi, PhoneContentInfo, PhoneContentProgress } from "../api/phone";
import type { ContentPlan, ContentSource, InstalledItem, SearchPage } from "../api/types";
import { Dialog } from "./Dialog";

function mb(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

interface Props {
  phone: PhoneApi;
  api: MercApi;
  id: string;
  /** El servidor está encendido: se puede buscar, pero no cambiar nada. */
  running: boolean;
  onError: (msg: string) => void;
}

export function PhoneContent({ phone, api, id, running, onError }: Props) {
  const [info, setInfo] = useState<PhoneContentInfo | null>(null);
  const [items, setItems] = useState<InstalledItem[] | null>(null);
  const [query, setQuery] = useState("");
  const [page, setPage] = useState<SearchPage | null>(null);
  const [searching, setSearching] = useState(false);
  const [source, setSource] = useState<ContentSource>("modrinth");
  const [planFor, setPlanFor] = useState<{ title: string; version_id: string; plan: ContentPlan; source: ContentSource } | null>(null);
  const [progress, setProgress] = useState<PhoneContentProgress | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const loadInstalled = useCallback(async () => {
    try {
      setItems(await phone.contentInstalled(id));
    } catch (e) {
      setItems([]);
      onError(errorMessage(e));
    }
  }, [phone, id, onError]);

  const search = useCallback(
    async (q: string, offset = 0, from: ContentSource = "modrinth") => {
      setSearching(true);
      if (offset === 0) setPage(null);
      try {
        const r = await phone.contentSearch(id, q, offset, from);
        setPage((prev) => (offset > 0 && prev ? { ...r, hits: [...prev.hits, ...r.hits] } : r));
      } catch (e) {
        onError(errorMessage(e));
      } finally {
        setSearching(false);
      }
    },
    [phone, id, onError],
  );

  useEffect(() => {
    let alive = true;
    phone.contentInfo(id).then(
      (i) => {
        if (!alive) return;
        setInfo(i);
        if (i.platform) {
          loadInstalled();
          search("");
        }
      },
      (e) => alive && onError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [phone, id, loadInstalled, search, onError]);

  useEffect(() => {
    if (!progress?.running) return;
    const t = setInterval(() => phone.contentProgress().then(setProgress, () => undefined), 600);
    return () => clearInterval(t);
  }, [phone, progress?.running]);

  async function askPlan(projectId: string, title: string) {
    setBusy(projectId);
    try {
      const r = await phone.contentPlan(id, projectId, source);
      setPlanFor({ title, ...r, source });
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  async function install() {
    if (!planFor) return;
    setProgress({ running: true, done: 0, total: planFor.plan.download_bytes });
    try {
      await phone.contentInstall(id, planFor.version_id, planFor.source);
      setPlanFor(null);
      await loadInstalled();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setProgress(null);
    }
  }

  async function act(key: string, fn: () => Promise<unknown>) {
    setBusy(key);
    try {
      await fn();
      await loadInstalled();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  if (info === null) return null;
  const platform = info.platform;
  const what = platform?.kind === "mod" ? "Mods" : "Plugins";
  const hasCurseforge = platform?.sources.includes("curseforge") ?? false;
  const sourceName = source === "curseforge" ? "CurseForge" : "Modrinth";
  const pickSource = (s: ContentSource) => {
    if (s === source) return;
    setSource(s);
    void search(query.trim(), 0, s);
  };

  return (
    <div className="card props-card" aria-label={platform ? what : "Mods y plugins"} role="region">
      <h2 className="panel-title">{platform ? what : "Mods y plugins"}</h2>
      <div className="seg" role="group" aria-label="Catálogo">
        <button type="button" className="seg-btn" aria-pressed={source === "modrinth"} onClick={() => pickSource("modrinth")}>
          Modrinth
        </button>
        {hasCurseforge ? (
          <button type="button" className="seg-btn" aria-pressed={source === "curseforge"} onClick={() => pickSource("curseforge")}>
            CurseForge
          </button>
        ) : (
          <button
            type="button"
            className="seg-btn"
            aria-pressed={false}
            disabled
            title={
              platform?.kind === "plugin"
                ? "En el celular, CurseForge es solo para mods (servidores Fabric)"
                : "Estará disponible cuando MercHosting tenga su clave oficial de CurseForge"
            }
          >
            CurseForge <span className="badge badge-muted">{platform?.kind === "plugin" ? "Solo mods" : "Próximamente"}</span>
          </button>
        )}
      </div>
      {!platform ? (
        <p className="field-help">
          Vanilla no admite mods ni plugins. Crea un servidor Paper o Purpur (plugins) o Fabric (mods) para usar el catálogo.
        </p>
      ) : (
        <>
          <p className="field-help">
            Para {platform.loaders[0]} · Minecraft {info.mc_version} · se guardan en <span className="mono">{platform.folder}</span>. Pocos{" "}
            {what.toLowerCase()}: el celular rinde menos que una PC.
          </p>
          {running && <p className="warn-text">Detén el servidor para instalar, desactivar o quitar {what.toLowerCase()}.</p>}

          <h3 className="field-title">Instalados</h3>
          {items === null ? (
            <p className="wiz-loading">Cargando…</p>
          ) : items.length === 0 ? (
            <p className="field-help">Todavía no hay {what.toLowerCase()} en este servidor.</p>
          ) : (
            <ul className="mobile-servers" aria-label={`${what} instalados`}>
              {items.map((it) => (
                <li key={it.file_name} className="content-row">
                  <span>
                    <strong translate="no">{it.title ?? it.file_name}</strong>
                    <span className="field-help">
                      {it.version_number ?? it.file_name}
                      {!it.enabled && " · desactivado"}
                    </span>
                  </span>
                  <span className="row-actions">
                    <label className="switch-row">
                      <input
                        type="checkbox"
                        checked={it.enabled}
                        disabled={running || busy !== null}
                        onChange={(e) => act(it.file_name, () => phone.contentSetEnabled(id, it.file_name, e.target.checked))}
                      />
                      <span>Activado</span>
                    </label>
                    <button
                      type="button"
                      className="btn btn-sm btn-quiet"
                      disabled={running || busy !== null}
                      aria-label={`Quitar ${it.title ?? it.file_name}`}
                      onClick={() => act(it.file_name, () => phone.contentRemove(id, it.file_name))}
                    >
                      <Trash2 size={14} aria-hidden="true" /> Quitar
                    </button>
                  </span>
                </li>
              ))}
            </ul>
          )}
          <p className="field-help">«Quitar» lo manda a la papelera de la app (no se borra).</p>

          <h3 className="field-title">Buscar en {sourceName}</h3>
          <form
            className="join-form"
            onSubmit={(e) => {
              e.preventDefault();
              search(query.trim(), 0, source);
            }}
          >
            <label className="field">
              <span className="field-title">Nombre</span>
              <input type="search" value={query} onChange={(e) => setQuery(e.target.value)} maxLength={100} autoCapitalize="off" />
            </label>
            <button type="submit" className="btn btn-sm" disabled={searching}>
              <Search size={14} aria-hidden="true" /> Buscar
            </button>
          </form>
          {page === null ? (
            <p className="wiz-loading">{searching ? "Buscando…" : ""}</p>
          ) : page.hits.length === 0 ? (
            <p className="field-help">No hay resultados para esta versión de Minecraft.</p>
          ) : (
            <ul className="mobile-servers" aria-label={`Resultados de ${sourceName}`}>
              {page.hits.map((h) => (
                <li key={h.project_id} className="content-row">
                  <span>
                    <strong translate="no">{h.title}</strong>
                    <span className="field-help">
                      de {h.author} · {h.downloads.toLocaleString("es")} descargas
                    </span>
                    <span className="field-help">{h.description}</span>
                  </span>
                  <span className="row-actions">
                    <button
                      type="button"
                      className="btn btn-sm btn-go"
                      disabled={running || busy !== null || progress !== null}
                      aria-label={`Instalar ${h.title}`}
                      onClick={() => askPlan(h.project_id, h.title)}
                    >
                      {busy === h.project_id ? "Revisando…" : "Instalar"}
                    </button>
                    <button
                      type="button"
                      className="btn btn-sm btn-quiet"
                      aria-label={`Ver ${h.title} en ${sourceName}`}
                      onClick={() =>
                        api.openUrl(
                          source === "curseforge"
                            ? `https://www.curseforge.com/minecraft/mc-mods/${encodeURIComponent(h.slug)}`
                            : `https://modrinth.com/project/${encodeURIComponent(h.slug)}`,
                        )
                      }
                    >
                      <ExternalLink size={14} aria-hidden="true" />
                    </button>
                  </span>
                </li>
              ))}
            </ul>
          )}
          {page && page.hits.length < page.total && (
            <button type="button" className="btn btn-sm" disabled={searching} onClick={() => search(query.trim(), page.hits.length, source)}>
              Ver más
            </button>
          )}
          {!hasCurseforge && platform.kind === "mod" && (
            <p className="field-help">CurseForge llega cuando MercHosting tenga su clave oficial. Mientras tanto, solo Modrinth.</p>
          )}
          {source === "curseforge" && (
            <p className="field-help">
              Si el autor de un mod de CurseForge no permite descargarlo desde otras apps, MercHosting te lo dice y no lo instala.
            </p>
          )}
        </>
      )}

      {planFor && (
        <Dialog
          title={`Instalar ${planFor.title}`}
          lead={`Se descargan ${mb(planFor.plan.download_bytes)} y se comprueba cada archivo con su huella.`}
          onClose={progress ? () => undefined : () => setPlanFor(null)}
          footer={
            progress ? (
              <button type="button" className="btn btn-quiet" onClick={() => phone.contentCancel()}>
                Cancelar descarga
              </button>
            ) : (
              <>
                <button type="button" className="btn btn-quiet" onClick={() => setPlanFor(null)}>
                  Cancelar
                </button>
                <button type="button" className="btn btn-go" disabled={planFor.plan.blockers.length > 0} onClick={install}>
                  Instalar
                </button>
              </>
            )
          }
        >
          <ul className="field-help">
            {planFor.plan.items.map((it) => (
              <li key={it.version_id}>
                <strong translate="no">{it.title}</strong> {it.version_number} · {it.reason}
                {it.replaces ? ` · reemplaza ${it.replaces} (va a la papelera)` : ""}
              </li>
            ))}
          </ul>
          {planFor.plan.already_installed.length > 0 && (
            <p className="field-help">Ya instalado: {planFor.plan.already_installed.join(", ")}.</p>
          )}
          {planFor.plan.warnings.map((w) => (
            <p key={w} className="warn-text">
              {w}
            </p>
          ))}
          {planFor.plan.blockers.map((b) => (
            <p key={b} className="form-error" role="alert">
              {b}
            </p>
          ))}
          {progress && (
            <p className="field-help" role="status">
              Descargando… {progress.total ? `${Math.round((progress.done / progress.total) * 100)}%` : ""}
            </p>
          )}
        </Dialog>
      )}
    </div>
  );
}
