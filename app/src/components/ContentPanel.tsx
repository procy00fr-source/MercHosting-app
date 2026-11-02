// Pestaña Mods / Plugins / Datapacks: lo instalado (activar, desactivar, quitar, actualizar) y
// los catálogos (Modrinth, CurseForge, Hangar): buscar, ver la ficha, revisar el plan con
// dependencias e instalar.

import {
  ArrowUpCircle,
  Download,
  ExternalLink,
  Package,
  RefreshCw,
  Search,
  ShieldAlert,
  Trash2,
  TriangleAlert,
  X,
  Split,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, useRef } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type {
  ContentHit,
  ContentKind,
  ContentList,
  ContentPlan,
  ContentProject,
  ContentSource,
  ContentVersion,
  InstanceId,
  InstalledItem,
  Platform,
  SearchSort,
  Side,
  WorldOp,
} from "../api/types";
import { MercError } from "../api/types";
import { formatSize } from "../format";
import { Dialog } from "./Dialog";
import { ModSidesPanel } from "./ModSidesPanel";
import { modSidesEnabled } from "../modSidesPref";
import { baseName, dropSummary, uploadAll, useFileDrop } from "../useFileDrop";
import { uiLocale } from "../i18n/lang";

const LOADER_LABEL: Record<string, string> = {
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neoforge: "NeoForge",
  paper: "Paper",
  purpur: "Purpur",
  spigot: "Spigot",
  bukkit: "Bukkit",
  datapack: "Datapack",
};

const SOURCE_LABEL: Record<ContentSource, string> = { modrinth: "Modrinth", curseforge: "CurseForge", hangar: "Hangar" };

const NOUNS: Record<ContentKind, [string, string]> = {
  mod: ["mods", "Mods"],
  plugin: ["plugins", "Plugins"],
  datapack: ["datapacks", "Datapacks"],
};

const ICON_HOSTS = ["https://cdn.modrinth.com/", "https://media.forgecdn.net/", "https://hangarcdn.papermc.io/"];

/** Solo se muestran iconos de los catálogos (la ventana no carga imágenes de otros sitios). */
function safeIcon(url: string | null): string | null {
  return url && ICON_HOSTS.some((h) => url.startsWith(h)) ? url : null;
}

function Icon({ url, size = 40 }: { url: string | null; size?: number }) {
  const src = safeIcon(url);
  return (
    <span className="content-icon" style={{ width: size, height: size }}>
      {src ? <img src={src} alt="" width={size} height={size} loading="lazy" /> : <Package size={size * 0.5} aria-hidden="true" />}
    </span>
  );
}

function compact(n: number): string {
  return new Intl.NumberFormat(uiLocale(), { notation: "compact", maximumFractionDigits: 1 }).format(n);
}

function sideText(client: Side, server: Side): string | null {
  if (client === "unsupported" && server !== "unsupported") return "Solo servidor";
  if (client === "required" && server !== "unsupported") return "Servidor y jugadores";
  return null;
}

function shortDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleDateString(uiLocale(), { dateStyle: "medium" });
}

interface Props {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  platform: Platform;
  running: boolean;
  mcVersion: string | null;
  /** Abrir directamente el catálogo buscando esto (p. ej., desde el Diagnóstico). */
  initialQuery?: string;
}

type Picked = { project: string; source: ContentSource; hit?: ContentHit; versionId?: string };

export function ContentPanel({ api, sup, id, platform, running, mcVersion, initialQuery }: Props) {
  const kind = platform.kind;
  const [noun, Noun] = NOUNS[kind];
  const ext = kind === "datapack" ? ".zip" : ".jar";
  const [view, setView] = useState<"installed" | "search" | "sides">(initialQuery ? "search" : "installed");
  const [list, setList] = useState<ContentList | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [removeAsk, setRemoveAsk] = useState<InstalledItem | null>(null);
  const [picked, setPicked] = useState<Picked | null>(null);
  const [sidesOn, setSidesOn] = useState(() => modSidesEnabled(id));

  const loadInstalled = useCallback(async () => {
    setListError(null);
    try {
      setList(await sup.contentInstalled(id, kind));
    } catch (e) {
      setListError(errorMessage(e));
    }
  }, [sup, id, kind]);

  useEffect(() => {
    loadInstalled();
  }, [loadInstalled]);

  async function act(fn: () => Promise<ContentList>, msg?: string) {
    setBusy(true);
    setListError(null);
    setNote(null);
    try {
      setList(await fn());
      if (msg) setNote(msg);
    } catch (e) {
      setListError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const drop = useFileDrop(api, true, async (paths) => {
    setNote(null);
    setListError(null);
    if (running) {
      setListError(`Detén el servidor para añadir ${noun}: Minecraft los carga al encender.`);
      return;
    }
    const good = paths.filter((p) => p.toLowerCase().endsWith(ext));
    const skipped = paths.filter((p) => !p.toLowerCase().endsWith(ext)).map(baseName);
    if (!good.length) {
      setListError(`Solo se pueden soltar archivos ${ext} aquí.`);
      return;
    }
    setBusy(true);
    const r = await uploadAll((src) => sup.uploadFile(id, src, platform.folder), good);
    setBusy(false);
    await loadInstalled();
    setView("installed");
    const msg = dropSummary(r.ok, r.failed, skipped, platform.folder);
    if (r.failed.length || !r.ok.length) setListError(msg);
    else setNote(msg);
  });

  const loaders = platform.loaders.map((l) => LOADER_LABEL[l] ?? l).join(", ");
  const catalogs = platform.sources.map((x) => SOURCE_LABEL[x]).join(" y ");
  const items = list?.items ?? [];
  const updates = items.filter((i) => i.update && i.enabled).length;
  const clientOnly = items.filter((i) => i.enabled && i.server_side === "unsupported").length;

  return (
    <div className="admin-panel content-panel drop-zone" role="tabpanel" aria-label={Noun} data-dragging={drop.dragging}>
      {drop.dragging && (
        <div className="drop-overlay" role="status">
          {running ? (
            <p>Detén el servidor para añadir {noun}.</p>
          ) : (
            <p>
              Suelta para copiar {drop.count > 1 ? `${drop.count} archivos` : "el archivo"} {ext} a <span className="mono">{platform.folder}</span>.
              <br />
              <span className="field-help">No se sobrescribe nada que ya exista.</span>
            </p>
          )}
        </div>
      )}
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">{Noun}</h2>
          <p className="field-help">
            {kind === "datapack" ? "Del mundo activo" : `Para ${loaders}`}
            {mcVersion ? ` · Minecraft ${mcVersion}` : ""} · carpeta <span className="mono">{platform.folder}</span>
          </p>
        </div>
        <div className="seg" role="group" aria-label="Vista">
          <button type="button" className="seg-btn" aria-pressed={view === "installed"} onClick={() => setView("installed")}>
            Instalados{list ? ` (${items.length})` : ""}
          </button>
          <button type="button" className="seg-btn" aria-pressed={view === "search"} onClick={() => setView("search")}>
            <Search size={14} aria-hidden="true" /> Buscar en {catalogs}
          </button>
          {kind === "mod" && (
            <button type="button" className="seg-btn" aria-pressed={view === "sides"} onClick={() => setView("sides")}>
              <Split size={14} aria-hidden="true" /> Separar cliente/servidor
            </button>
          )}
        </div>
      </div>

      {running && (
        <p className="field-help warn-text">
          Los {noun} se cargan al encender: para instalar, activar, desactivar o quitar, detén el servidor primero.
        </p>
      )}
      {kind === "datapack" && (
        <p className="field-help">
          Cada mundo tiene sus datapacks: estos son los de <span className="mono">{platform.folder.replace(/\/datapacks$/, "")}</span>.
          Si cambias de mundo en Mundos, verás los del otro. Los que están en carpetas (no .zip) se gestionan en Archivos.
        </p>
      )}
      {platform.experimental && (
        <p className="field-help warn-text">
          Este servidor es híbrido (mods y plugins a la vez): muchos {noun} funcionan, pero la compatibilidad no está
          garantizada.
        </p>
      )}
      {note && (
        <p className="field-help ok-text" role="status">
          {note}
        </p>
      )}

      {view === "sides" ? (
        <ModSidesPanel sup={sup} id={id} running={running} onChanged={loadInstalled} onToggle={setSidesOn} />
      ) : view === "installed" ? (
        <>
          {kind === "mod" && sidesOn && clientOnly > 0 && (
            <div className="card sides-banner" role="note">
              <p>
                {clientOnly === 1 ? "Hay 1 mod" : `Hay ${clientOnly} mods`} que solo sirve{clientOnly === 1 ? "" : "n"} en el juego de los jugadores. En el
                servidor sobran y algunos impiden que arranque.
              </p>
              <button type="button" className="btn btn-sm" onClick={() => setView("sides")}>
                <Split size={14} aria-hidden="true" /> Revisarlos
              </button>
            </div>
          )}
          {listError && (
            <p className="form-error" role="alert">
              {listError}
            </p>
          )}
          {list && !list.online && (
            <p className="field-help warn-text">
              Sin conexión con {catalogs}: se ven los archivos, pero puede que no sus nombres ni si hay actualizaciones.
            </p>
          )}
          {list === null && !listError ? (
            <p className="wiz-loading" role="status">
              Revisando la carpeta {platform.folder}…
            </p>
          ) : items.length === 0 ? (
            <div className="card empty-small">
              <h2>Todavía no hay {noun}</h2>
              <p>
                Busca en {catalogs} e instálalos con sus dependencias. También puedes arrastrar tus {ext} desde el Explorador y
                soltarlos aquí (van a {platform.folder}).
              </p>
              <button type="button" className="btn btn-go" onClick={() => setView("search")}>
                <Search size={15} aria-hidden="true" /> Buscar {noun}
              </button>
            </div>
          ) : (
            <>
              <div className="files-bar">
                <p className="field-help">
                  {updates > 0 ? `${updates} con actualización disponible.` : `Todo al día según ${catalogs}.`}
                </p>
                <button type="button" className="btn btn-sm" onClick={loadInstalled} disabled={busy}>
                  <RefreshCw size={14} aria-hidden="true" /> Actualizar lista
                </button>
              </div>
              <ul className="content-list">
                {items.map((it) => (
                  <li key={it.file_name} className="card content-row" data-enabled={it.enabled}>
                    <Icon url={it.icon_url} />
                    <span className="content-main">
                      <span className="content-title">
                        {it.title ?? it.file_name}
                        {!it.enabled && <span className="badge badge-muted">Desactivado</span>}
                        {it.server_side === "unsupported" && <span className="badge badge-warn">Solo para jugadores</span>}
                      </span>
                      <span className="field-help">
                        {it.version_number ? `${it.version_number} · ` : ""}
                        <span className="mono">{it.file_name}</span> · {formatSize(it.size)}
                        {it.source ? ` · ${SOURCE_LABEL[it.source]}` : list?.online ? ` · no está en ${catalogs}` : ""}
                      </span>
                    </span>
                    <span className="content-actions">
                      {it.update && it.enabled && it.project_id && (
                        <button
                          type="button"
                          className="btn btn-sm btn-go"
                          disabled={running || busy}
                          onClick={() =>
                            setPicked({ project: it.project_id!, source: it.source ?? "modrinth", versionId: it.update!.version_id })
                          }
                        >
                          <ArrowUpCircle size={14} aria-hidden="true" /> Actualizar a {it.update.version_number}
                        </button>
                      )}
                      <label className="switch-row content-switch" title={it.enabled ? "Desactivar" : "Activar"}>
                        <input
                          type="checkbox"
                          aria-label={`${it.enabled ? "Desactivar" : "Activar"} ${it.title ?? it.file_name}`}
                          checked={it.enabled}
                          disabled={running || busy}
                          onChange={(e) =>
                            act(
                              () => sup.contentToggle(id, kind, it.file_name, e.target.checked),
                              e.target.checked ? "Activado. Se cargará al encender." : "Desactivado: no se cargará, pero sigue ahí.",
                            )
                          }
                        />
                      </label>
                      <button
                        type="button"
                        className="icon-btn icon-danger"
                        aria-label={`Quitar ${it.title ?? it.file_name}`}
                        title="Quitar (va a la papelera)"
                        disabled={running || busy}
                        onClick={() => setRemoveAsk(it)}
                      >
                        <Trash2 size={15} />
                      </button>
                    </span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      ) : (
        <SearchView sup={sup} id={id} platform={platform} initialQuery={initialQuery} onPick={(hit, source) => setPicked({ project: hit.project_id, source, hit })} />
      )}

      <p className="field-help content-risk">
        <ShieldAlert size={14} aria-hidden="true" />{" "}
        {kind === "datapack"
          ? "Los datapacks cambian las reglas y el contenido del mundo. Haz una copia del mundo antes de probar uno nuevo."
          : `Los ${noun} son programas que se ejecutan en tu PC con el servidor. Instala solo los que conozcas.`}{" "}
        MercHosting comprueba que cada archivo sea el que publica {catalogs}.
      </p>

      {removeAsk && (
        <Dialog
          title={`Quitar «${removeAsk.title ?? removeAsk.file_name}»`}
          lead="Va a la papelera de MercHosting: lo puedes restaurar desde Archivos › Papelera y copias. Si otros mods lo necesitan, dejarán de funcionar."
          onClose={() => setRemoveAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setRemoveAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  const it = removeAsk;
                  setRemoveAsk(null);
                  act(() => sup.contentRemove(id, kind, it.file_name), `«${it.title ?? it.file_name}» está en la papelera.`);
                }}
              >
                Mover a la papelera
              </button>
            </>
          }
        >
          <p className="path">
            {platform.folder}/{removeAsk.file_name}
          </p>
        </Dialog>
      )}

      {picked && (
        <ProjectDialog
          api={api}
          sup={sup}
          id={id}
          platform={platform}
          running={running}
          picked={picked}
          onClose={() => setPicked(null)}
          onInstalled={(l, msg) => {
            setPicked(null);
            setList(l);
            setNote(msg);
            setView("installed");
          }}
        />
      )}
    </div>
  );
}

function SearchView({
  sup,
  id,
  platform,
  initialQuery,
  onPick,
}: {
  sup: Supervisor;
  id: InstanceId;
  platform: Platform;
  initialQuery?: string;
  onPick: (h: ContentHit, source: ContentSource) => void;
}) {
  const [source, setSource] = useState<ContentSource>(platform.sources[0] ?? "modrinth");
  const [query, setQuery] = useState(initialQuery ?? "");
  const [sort, setSort] = useState<SearchSort>("relevance");
  const [hits, setHits] = useState<ContentHit[] | null>(null);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const noun = NOUNS[platform.kind][0];
  // Los mods muestran CurseForge aunque falte la clave (como «Próximamente»).
  const options: ContentSource[] = platform.kind === "mod" ? ["modrinth", "curseforge"] : platform.sources;

  const run = useCallback(
    async (q: string, s: SearchSort, offset: number) => {
      setLoading(true);
      setError(null);
      if (offset === 0) setHits(null);
      try {
        const page = await sup.contentSearch(id, platform.kind, source, q, s, offset);
        setHits((cur) => (offset === 0 ? page.hits : [...(cur ?? []), ...page.hits]));
        setTotal(page.total);
      } catch (e) {
        setError(errorMessage(e));
      } finally {
        setLoading(false);
      }
    },
    [sup, id, platform.kind, source],
  );

  // Al abrir (o al cambiar de catálogo) se muestran los más populares compatibles.
  const firstQuery = useRef(initialQuery ?? "");
  useEffect(() => {
    const q = firstQuery.current;
    firstQuery.current = "";
    setQuery(q);
    const s: SearchSort = q ? "relevance" : "downloads";
    run(q, s, 0);
    setSort(s);
  }, [run]);

  return (
    <div className="content-search">
      {options.length > 1 && (
        <div className="files-bar">
          <div className="seg" role="group" aria-label="Catálogo">
            {options.map((x) => {
              const available = platform.sources.includes(x);
              return (
                <button
                  key={x}
                  type="button"
                  className="seg-btn"
                  aria-pressed={source === x}
                  disabled={!available}
                  title={available ? undefined : "Estará disponible cuando MercHosting tenga su clave oficial de CurseForge"}
                  onClick={() => setSource(x)}
                >
                  {SOURCE_LABEL[x]}
                  {!available && <span className="badge badge-muted">Próximamente</span>}
                </button>
              );
            })}
          </div>
          {source === "hangar" ? (
            <p className="field-help">
              Hangar es el catálogo oficial de plugins de PaperMC. Si un autor publica el archivo en otra web, se te dirá que lo
              descargues desde su página.
            </p>
          ) : source === "curseforge" ? (
            <p className="field-help">Algunos autores de CurseForge no permiten descargar sus mods desde otras apps: se te avisará.</p>
          ) : (
            platform.kind === "mod" &&
            !platform.sources.includes("curseforge") && (
              <p className="field-help">CurseForge llegará próximamente (falta la clave oficial de MercHosting).</p>
            )
          )}
        </div>
      )}
      <form
        className="files-bar"
        onSubmit={(e) => {
          e.preventDefault();
          const s = query.trim() && sort === "downloads" ? "relevance" : sort;
          setSort(s);
          run(query, s, 0);
        }}
      >
        <input
          className="content-query"
          type="search"
          aria-label={`Buscar ${noun}`}
          placeholder={`Buscar ${noun} compatibles…`}
          value={query}
          maxLength={100}
          onChange={(e) => setQuery(e.target.value)}
        />
        <select
          aria-label="Ordenar por"
          value={sort}
          onChange={(e) => {
            const s = e.target.value as SearchSort;
            setSort(s);
            run(query, s, 0);
          }}
        >
          <option value="relevance">Más relevantes</option>
          <option value="downloads">Más descargados</option>
          <option value="updated">Actualizados hace poco</option>
          <option value="newest">Más nuevos</option>
        </select>
        <button type="submit" className="btn btn-sm btn-go" disabled={loading}>
          <Search size={14} aria-hidden="true" /> Buscar
        </button>
      </form>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {hits === null ? (
        loading && (
          <p className="wiz-loading" role="status">
            Consultando {SOURCE_LABEL[source]}…
          </p>
        )
      ) : hits.length === 0 ? (
        <div className="card empty-small">
          <h2>Sin resultados</h2>
          <p>No hay {noun} compatibles con este servidor para esa búsqueda.</p>
        </div>
      ) : (
        <>
          <p className="field-help">
            {compact(total)} {noun} compatibles
          </p>
          <ul className="content-grid">
            {hits.map((h) => {
              const side = sideText(h.client_side, h.server_side);
              return (
                <li key={h.project_id}>
                  <button type="button" className="card content-card" onClick={() => onPick(h, source)} aria-label={`Ver ${h.title}`}>
                    <Icon url={h.icon_url} size={48} />
                    <span className="content-main">
                      <span className="content-title">{h.title}</span>
                      <span className="field-help">
                        de {h.author} · <Download size={11} aria-hidden="true" /> {compact(h.downloads)}
                      </span>
                      <span className="content-desc">{h.description}</span>
                      {side && <span className="badge badge-muted">{side}</span>}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
          {hits.length < total && (
            <button type="button" className="btn" disabled={loading} onClick={() => run(query, sort, hits.length)}>
              {loading ? "Cargando…" : "Cargar más"}
            </button>
          )}
        </>
      )}
    </div>
  );
}

export function ProjectDialog({
  api,
  sup,
  id,
  platform,
  running,
  picked,
  onClose,
  onInstalled,
}: {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  platform: Platform;
  running: boolean;
  picked: Picked;
  onClose: () => void;
  onInstalled: (l: ContentList, msg: string) => void;
}) {
  const kind = platform.kind;
  const [project, setProject] = useState<ContentProject | null>(null);
  const [versions, setVersions] = useState<ContentVersion[] | null>(null);
  const [versionId, setVersionId] = useState<string>(picked.versionId ?? "");
  const [plan, setPlan] = useState<ContentPlan | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [op, setOp] = useState<WorldOp | null>(null);

  useEffect(() => {
    let alive = true;
    sup.contentProject(id, kind, picked.source, picked.project).then(
      (d) => {
        if (!alive) return;
        setProject(d.project);
        setVersions(d.versions);
        setVersionId((cur) => cur || (d.versions.find((v) => v.version_type === "release") ?? d.versions[0])?.id || "");
      },
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [sup, id, kind, picked.project, picked.source]);

  // Cada vez que cambia la versión elegida se recalcula el plan.
  useEffect(() => {
    if (!versionId) return;
    let alive = true;
    setPlan(null);
    sup.contentPlan(id, kind, picked.source, versionId).then(
      (p) => alive && setPlan(p),
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [sup, id, kind, versionId, picked.source]);

  useEffect(() => {
    if (!installing) return;
    const t = setInterval(async () => {
      try {
        setOp((await sup.adminState(id)).world_op);
      } catch {
        /* siguiente ciclo */
      }
    }, 400);
    return () => clearInterval(t);
  }, [installing, sup, id]);

  const version = useMemo(() => versions?.find((v) => v.id === versionId) ?? null, [versions, versionId]);

  async function install() {
    if (!plan) return;
    setInstalling(true);
    setError(null);
    try {
      const l = await sup.contentInstall(id, kind, picked.source, versionId);
      const extra = plan.items.length - 1;
      onInstalled(
        l,
        `Instalado: ${plan.items[0].title} ${plan.items[0].version_number}${extra > 0 ? ` y ${extra} dependencia${extra === 1 ? "" : "s"}` : ""}. Se cargará al encender.`,
      );
    } catch (e) {
      setError(
        e instanceof MercError && e.code === "conflict" && /cancelada/.test(e.message)
          ? "Instalación cancelada. No se cambió nada."
          : errorMessage(e),
      );
      setInstalling(false);
      setOp(null);
    }
  }

  const title = project?.title ?? picked.hit?.title ?? "Cargando…";
  const blocked = !plan || plan.blockers.length > 0 || running || installing;

  return (
    <Dialog
      wide
      title={title}
      lead={project?.description ?? picked.hit?.description}
      onClose={() => !installing && onClose()}
      footer={
        <>
          {project && (
            <button type="button" className="btn btn-quiet" onClick={() => api.openUrl(project.page_url)}>
              <ExternalLink size={14} aria-hidden="true" /> Ver en {SOURCE_LABEL[picked.source]}
            </button>
          )}
          <span className="dialog-spacer" />
          {installing ? (
            <button type="button" className="btn" onClick={() => sup.cancelWorldOp(id).catch(() => undefined)}>
              <X size={14} aria-hidden="true" /> Cancelar
            </button>
          ) : (
            <button type="button" className="btn btn-quiet" onClick={onClose}>
              Cerrar
            </button>
          )}
          <button type="button" className="btn btn-go" disabled={blocked} onClick={install}>
            <Download size={15} aria-hidden="true" />
            {installing ? "Instalando…" : plan && plan.items.length > 1 ? `Instalar (${plan.items.length} archivos)` : "Instalar"}
          </button>
        </>
      }
    >
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {project && (
        <div className="content-facts">
          <Icon url={project.icon_url} size={56} />
          <span>
            <Download size={12} aria-hidden="true" /> {compact(project.downloads)} descargas · {SOURCE_LABEL[picked.source]}
            {project.license ? ` · Licencia ${project.license}` : ""}
            {sideText(project.client_side, project.server_side) ? ` · ${sideText(project.client_side, project.server_side)}` : ""}
          </span>
        </div>
      )}
      {versions && versions.length === 0 && (
        <p className="form-error" role="alert">
          No hay ninguna versión de «{title}» para este servidor ({platform.loaders.join("/")}).
        </p>
      )}
      {versions && versions.length > 0 && (
        <div className="field">
          <label htmlFor="content-version">Versión</label>
          <select id="content-version" value={versionId} disabled={installing} onChange={(e) => setVersionId(e.target.value)}>
            {versions.map((v) => (
              <option key={v.id} value={v.id}>
                {v.version_number}
                {v.version_type !== "release" ? ` (${v.version_type === "beta" ? "beta" : "alfa"})` : ""} · {shortDate(v.date_published)}
              </option>
            ))}
          </select>
          {version && version.version_type !== "release" && (
            <p className="field-help warn-text">Es una versión de prueba: puede tener errores.</p>
          )}
        </div>
      )}

      {installing && op && op.kind === "install" && (
        <div className="dl" role="status">
          <div className="dl-head">
            <span>Descargando y verificando…</span>
            <span>
              {formatSize(op.done_bytes)}
              {op.total_bytes ? ` de ${formatSize(op.total_bytes)}` : ""}
            </span>
          </div>
          <div
            className="dl-bar"
            role="progressbar"
            aria-label="Instalación"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={op.total_bytes ? Math.min(100, Math.round((op.done_bytes / op.total_bytes) * 100)) : undefined}
          >
            <span style={{ width: op.total_bytes ? `${Math.min(100, (op.done_bytes / op.total_bytes) * 100)}%` : "35%" }} />
          </div>
        </div>
      )}

      {versionId && !plan && !error && (
        <p className="wiz-loading" role="status">
          Revisando dependencias…
        </p>
      )}
      {plan && (
        <section className="content-plan" aria-label="Qué se va a instalar">
          <h3 className="rule-group-title">Qué se va a instalar</h3>
          <ul className="plan-list">
            {plan.items.map((it) => (
              <li key={it.version_id}>
                <Icon url={it.icon_url} size={28} />
                <span className="content-main">
                  <strong>
                    {it.title} <span className="field-help">{it.version_number}</span>
                  </strong>
                  <span className="field-help">
                    {it.reason} · {formatSize(it.size)}
                    {it.replaces ? ` · reemplaza ${it.replaces} (va a la papelera)` : ""}
                  </span>
                </span>
              </li>
            ))}
          </ul>
          {plan.already_installed.length > 0 && (
            <p className="field-help">Ya tienes: {plan.already_installed.join(", ")}.</p>
          )}
          {plan.optional.length > 0 && (
            <p className="field-help">
              Opcionales (no se instalan solos): {plan.optional.map((o) => o.title).join(", ")}. Puedes buscarlos si quieres sus
              extras.
            </p>
          )}
          {plan.warnings.map((w) => (
            <p key={w} className="field-help warn-text">
              <TriangleAlert size={13} aria-hidden="true" /> {w}
            </p>
          ))}
          {plan.blockers.map((b) => (
            <p key={b} className="form-error" role="alert">
              {b}
            </p>
          ))}
          {running && plan.blockers.length === 0 && (
            <p className="field-help warn-text">Detén el servidor para instalar.</p>
          )}
        </section>
      )}

      {version?.changelog && (
        <details className="content-changelog">
          <summary>Cambios de la versión {version.version_number}</summary>
          <div className="md-plain">{version.changelog}</div>
        </details>
      )}
      {project?.body && (
        <details className="content-changelog">
          <summary>Descripción completa</summary>
          <div className="md-plain">{project.body}</div>
        </details>
      )}
    </Dialog>
  );
}
