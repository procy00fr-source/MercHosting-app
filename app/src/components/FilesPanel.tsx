// Pestaña Archivos: explorador limitado a la carpeta del servidor.
// Ver siempre; modificar solo con el servidor detenido. «Borrar» mueve a la papelera.

import {
  ArchiveRestore,
  ChevronRight,
  Copy,
  Download,
  File,
  FileText,
  Folder,
  FolderInput,
  FolderOpen,
  FolderPlus,
  House,
  Link2,
  PackageOpen,
  Pencil,
  RefreshCw,
  Save,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { FileEntry, InstanceId, Perm, SavedItem, TextFile } from "../api/types";
import { formatSize } from "../format";
import { CodeEditor } from "./CodeEditor";
import { Dialog } from "./Dialog";
import { FolderPicker } from "./FolderPicker";
import { dropSummary, uploadAll, useFileDrop } from "../useFileDrop";
import { uiLocale } from "../i18n/lang";

interface Props {
  /** Servidor de un amigo: sin abrir carpetas de su PC. */
  remote?: boolean;
  /** Permisos del compañero (en servidores propios, todo). */
  can?: (p: Perm) => boolean;
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  running: boolean;
}

const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name);

function formatDate(ms: number | null): string {
  if (!ms) return "";
  return new Date(ms).toLocaleString(uiLocale(), { dateStyle: "short", timeStyle: "short" });
}

type Ask =
  | { kind: "rename"; entry: FileEntry; value: string }
  | { kind: "mkdir"; value: string }
  | { kind: "trash"; entry: FileEntry }
  | { kind: "move" | "copy"; entry: FileEntry }
  | { kind: "extract"; entry: FileEntry };

export function FilesPanel({ api, sup, id, running, remote = false, can = () => true }: Props) {
  const [dir, setDir] = useState("");
  const [entries, setEntries] = useState<FileEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [open, setOpen] = useState<TextFile | null>(null);
  const [text, setText] = useState("");
  const [ask, setAsk] = useState<Ask | null>(null);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState<SavedItem[] | null>(null);
  const [savedOpen, setSavedOpen] = useState(false);
  const [purgeAsk, setPurgeAsk] = useState<SavedItem | null>(null);

  async function openSaved() {
    setSavedOpen(true);
    try {
      setSaved(await sup.savedItems(id));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function savedAction(fn: () => Promise<string | void>) {
    setBusy(true);
    setError(null);
    try {
      const msg = await fn();
      if (msg) setNote(msg);
      setSaved(await sup.savedItems(id));
      await load(dir);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const load = useCallback(
    async (d: string) => {
      setError(null);
      try {
        setEntries(await sup.listFiles(id, d));
        setDir(d);
      } catch (e) {
        setError(errorMessage(e));
      }
    },
    [sup, id],
  );

  useEffect(() => {
    load("");
  }, [load]);

  async function run(fn: () => Promise<string | void>) {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const msg = await fn();
      if (msg) setNote(msg);
      await load(dir);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function openFile(entry: FileEntry) {
    setError(null);
    try {
      const f = await sup.readFile(id, join(dir, entry.name));
      setOpen(f);
      setText(f.content);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function download(path: string, name: string) {
    const dest = await api.pickSavePath(name);
    if (!dest) return;
    await run(async () => {
      await sup.downloadFile(id, path, dest);
      return `Copia guardada en ${dest}`;
    });
  }

  async function upload() {
    const src = await api.pickFile();
    if (!src) return;
    await run(async () => `Subido: ${await sup.uploadFile(id, src, dir)}`);
  }

  async function saveOpen() {
    if (!open) return;
    setBusy(true);
    setError(null);
    try {
      const f = await sup.writeFile(id, open.path, text, open.modified_unix_ms);
      setOpen(f);
      setText(f.content);
      setNote(`Guardado ${f.path}. La versión anterior quedó en las copias de MercHosting.`);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const drop = useFileDrop(api, can("files_edit"), async (paths) => {
    if (running) {
      setNote(null);
      setError("Detén el servidor para copiar archivos.");
      return;
    }
    await run(async () => {
      const r = await uploadAll((src) => sup.uploadFile(id, src, dir), paths);
      const msg = dropSummary(r.ok, r.failed, [], dir || "la carpeta del servidor");
      if (r.failed.length) throw new Error(msg);
      return msg;
    });
  });

  const crumbs = dir ? dir.split("/") : [];
  const locked = running || busy;
  const dirty = !!open && text !== open.content;

  return (
    <div className="admin-panel files-panel drop-zone" role="tabpanel" aria-label="Archivos" data-dragging={drop.dragging}>
      {drop.dragging && (
        <div className="drop-overlay" role="status">
          {running ? (
            <p>Detén el servidor para copiar archivos.</p>
          ) : (
            <p>
              Suelta para copiar {drop.count > 1 ? `${drop.count} archivos` : "el archivo"} a{" "}
              <span className="mono">{dir || "la carpeta del servidor"}</span>.
              <br />
              <span className="field-help">No se sobrescribe nada que ya exista.</span>
            </p>
          )}
        </div>
      )}
      {running && (
        <p className="field-help warn-text">
          El servidor está encendido: puedes ver y descargar, pero para cambiar archivos deténlo antes (Minecraft los tiene
          abiertos).
        </p>
      )}

      <div className="files-bar">
        <nav className="crumbs" aria-label="Carpeta actual">
          <button type="button" className="crumb" onClick={() => load("")} aria-label="Carpeta del servidor">
            <House size={14} aria-hidden="true" />
          </button>
          {crumbs.map((c, i) => (
            <span key={i} className="crumb-wrap">
              <ChevronRight size={13} aria-hidden="true" />
              <button type="button" className="crumb" onClick={() => load(crumbs.slice(0, i + 1).join("/"))}>
                {c}
              </button>
            </span>
          ))}
        </nav>
        <button type="button" className="btn btn-sm" onClick={() => load(dir)} disabled={busy}>
          <RefreshCw size={14} aria-hidden="true" /> Actualizar
        </button>
        {!remote && (
          <button
            type="button"
            className="btn btn-sm"
            title="Abrir esta carpeta en el explorador de archivos"
            onClick={() => api.openServerFolder(id, dir).catch((e) => setError(errorMessage(e)))}
          >
            <FolderOpen size={14} aria-hidden="true" /> Ver carpeta del servidor
          </button>
        )}
        <button type="button" className="btn btn-sm" onClick={openSaved}>
          <ArchiveRestore size={14} aria-hidden="true" /> Papelera y copias
        </button>
        <button type="button" className="btn btn-sm" hidden={!can("files_edit")} onClick={() => setAsk({ kind: "mkdir", value: "" })} disabled={locked}>
          <FolderPlus size={14} aria-hidden="true" /> Nueva carpeta
        </button>
        {can("files_edit") && api.canTransfer !== false && (
          <button type="button" className="btn btn-sm btn-go" onClick={upload} disabled={locked}>
            <Upload size={14} aria-hidden="true" /> Subir archivo
          </button>
        )}
      </div>

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

      <div className={open ? "files-split" : undefined}>
        <div className="card file-table-wrap">
          {entries === null ? (
            <p className="wiz-loading" role="status">
              Leyendo la carpeta…
            </p>
          ) : entries.length === 0 ? (
            <p className="field-help file-empty">Carpeta vacía.</p>
          ) : (
            <table className="file-table">
              <thead>
                <tr>
                  <th>Nombre</th>
                  <th>Tamaño</th>
                  <th>Modificado</th>
                  <th>
                    <span className="sr-only">Acciones</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {entries.map((e) => (
                  <tr key={e.name} aria-selected={open?.path === join(dir, e.name)}>
                    <td>
                      {e.link ? (
                        <span className="file-name file-link" title="Enlace: por seguridad no se sigue">
                          <Link2 size={15} aria-hidden="true" /> {e.name}
                        </span>
                      ) : (
                        <button
                          type="button"
                          className="file-name"
                          disabled={!e.is_dir && !can("files_read")}
                          onClick={() => (e.is_dir ? load(join(dir, e.name)) : openFile(e))}
                        >
                          {e.is_dir ? (
                            <Folder size={15} aria-hidden="true" className="ic-folder" />
                          ) : /\.(txt|json|toml|properties|yml|yaml|cfg|conf|log|ini|md|mcmeta|snbt|js|csv)$/i.test(e.name) ? (
                            <FileText size={15} aria-hidden="true" />
                          ) : (
                            <File size={15} aria-hidden="true" />
                          )}
                          {e.name}
                        </button>
                      )}
                    </td>
                    <td className="num">{e.size !== null ? formatSize(e.size) : ""}</td>
                    <td className="num">{formatDate(e.modified_unix_ms)}</td>
                    <td className="file-actions">
                      {can("files_read") && !e.is_dir && !e.link && api.canTransfer !== false && (
                        <button
                          type="button"
                          className="icon-btn"
                          aria-label={`Descargar ${e.name}`}
                          title="Descargar una copia"
                          onClick={() => download(join(dir, e.name), e.name)}
                        >
                          <Download size={15} />
                        </button>
                      )}
                      {!e.link && can("files_edit") && (
                        <button
                          type="button"
                          className="icon-btn"
                          aria-label={`Renombrar ${e.name}`}
                          title="Renombrar"
                          disabled={locked}
                          onClick={() => setAsk({ kind: "rename", entry: e, value: e.name })}
                        >
                          <Pencil size={15} />
                        </button>
                      )}
                      {!e.link && can("files_edit") && (
                        <>
                          {!e.is_dir && /\.zip$/i.test(e.name) && (
                            <button
                              type="button"
                              className="icon-btn"
                              aria-label={`Extraer ${e.name}`}
                              title="Extraer en una carpeta nueva"
                              disabled={locked}
                              onClick={() => setAsk({ kind: "extract", entry: e })}
                            >
                              <PackageOpen size={15} />
                            </button>
                          )}
                          <button
                            type="button"
                            className="icon-btn"
                            aria-label={`Mover ${e.name} a otra carpeta`}
                            title="Mover a otra carpeta"
                            disabled={locked}
                            onClick={() => setAsk({ kind: "move", entry: e })}
                          >
                            <FolderInput size={15} />
                          </button>
                          <button
                            type="button"
                            className="icon-btn"
                            aria-label={`Copiar ${e.name} a otra carpeta`}
                            title="Copiar a otra carpeta"
                            disabled={locked}
                            onClick={() => setAsk({ kind: "copy", entry: e })}
                          >
                            <Copy size={15} />
                          </button>
                        </>
                      )}
                      {!e.link && can("files_delete") && (
                        <button
                          type="button"
                          className="icon-btn icon-danger"
                          aria-label={`Mover ${e.name} a la papelera`}
                          title="Mover a la papelera"
                          disabled={locked}
                          onClick={() => setAsk({ kind: "trash", entry: e })}
                        >
                          <Trash2 size={15} />
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        {open && (
          <div className="card file-editor">
            <header className="file-editor-head">
              <span className="mono">{open.path}</span>
              <span className="field-help">{formatSize(open.size)}</span>
              <button type="button" className="icon-btn" aria-label="Cerrar archivo" onClick={() => setOpen(null)}>
                <X size={16} />
              </button>
            </header>
            <CodeEditor path={open.path} label={`Contenido de ${open.path}`} value={text} readOnly={running} onChange={setText} />
            <footer className="file-editor-foot">
              {running ? (
                <span className="field-help">Solo lectura mientras el servidor está encendido.</span>
              ) : (
                <span className="field-help">{dirty ? "Cambios sin guardar" : "Sin cambios"}</span>
              )}
              <button type="button" className="btn btn-sm" onClick={() => setText(open.content)} disabled={!dirty || busy}>
                Deshacer
              </button>
              <button type="button" className="btn btn-sm btn-go" onClick={saveOpen} disabled={!dirty || locked}>
                <Save size={14} aria-hidden="true" /> Guardar
              </button>
            </footer>
          </div>
        )}
      </div>

      {savedOpen && (
        <Dialog
          title="Papelera y copias"
          wide
          lead="Lo que mandaste a la papelera y las versiones anteriores de los archivos que editaste. Está guardado fuera de la carpeta del servidor."
          onClose={() => setSavedOpen(false)}
          footer={
            <button type="button" className="btn" onClick={() => setSavedOpen(false)}>
              Cerrar
            </button>
          }
        >
          {running && <p className="field-help warn-text">Para restaurar, detén el servidor.</p>}
          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}
          {saved === null ? (
            <p className="wiz-loading" role="status">
              Cargando…
            </p>
          ) : saved.length === 0 ? (
            <p className="field-help">No hay nada guardado.</p>
          ) : (
            <ul className="saved-list">
              {saved.map((it) => (
                <li key={it.id}>
                  <span className={it.kind === "trash" ? "saved-kind trash" : "saved-kind copy"}>
                    {it.kind === "trash" ? "Papelera" : "Versión anterior"}
                  </span>
                  <span className="saved-main">
                    <span className="mono">{it.path}</span>
                    <span className="field-help">
                      {formatSize(it.size_bytes)} · {formatDate(it.saved_unix_ms)}
                    </span>
                  </span>
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={running || busy}
                    aria-label={`Restaurar ${it.path} (${it.kind === "trash" ? "papelera" : "versión anterior"})`}
                    onClick={() => savedAction(async () => `Restaurado: ${await sup.restoreSaved(id, it.id)}`)}
                  >
                    Restaurar
                  </button>
                  <button
                    type="button"
                    className="icon-btn icon-danger"
                    aria-label={`Borrar definitivamente ${it.path}`}
                    title="Borrar definitivamente"
                    disabled={busy}
                    onClick={() => setPurgeAsk(it)}
                  >
                    <Trash2 size={15} />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </Dialog>
      )}

      {purgeAsk && (
        <Dialog
          title="Borrar definitivamente"
          lead={`«${purgeAsk.path}» se eliminará de tu PC y no se podrá recuperar.`}
          onClose={() => setPurgeAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setPurgeAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  const it = purgeAsk;
                  setPurgeAsk(null);
                  savedAction(async () => {
                    await sup.purgeSaved(id, it.id);
                    return `Borrado: ${it.path}`;
                  });
                }}
              >
                Borrar para siempre
              </button>
            </>
          }
        >
          <p className="field-help">{formatSize(purgeAsk.size_bytes)}</p>
        </Dialog>
      )}

      {ask?.kind === "rename" && (
        <Dialog
          title={`Renombrar «${ask.entry.name}»`}
          onClose={() => setAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                disabled={!ask.value.trim() || ask.value === ask.entry.name}
                onClick={() => {
                  const a = ask;
                  setAsk(null);
                  run(async () => `Renombrado a ${await sup.renameFile(id, join(dir, a.entry.name), a.value.trim())}`);
                }}
              >
                Renombrar
              </button>
            </>
          }
        >
          <div className="field">
            <label htmlFor="rename-input">Nombre nuevo</label>
            <input id="rename-input" type="text" value={ask.value} onChange={(e) => setAsk({ ...ask, value: e.target.value })} />
            {/\.jar$/i.test(ask.entry.name) && (
              <p className="field-help">Truco: para desactivar un mod sin borrarlo, añade «.desactivado» al final del nombre.</p>
            )}
          </div>
        </Dialog>
      )}

      {ask?.kind === "mkdir" && (
        <Dialog
          title="Nueva carpeta"
          onClose={() => setAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                disabled={!ask.value.trim()}
                onClick={() => {
                  const a = ask;
                  setAsk(null);
                  run(async () => `Carpeta creada: ${await sup.makeDir(id, join(dir, a.value.trim()))}`);
                }}
              >
                Crear
              </button>
            </>
          }
        >
          <div className="field">
            <label htmlFor="mkdir-input">Nombre</label>
            <input id="mkdir-input" type="text" value={ask.value} onChange={(e) => setAsk({ ...ask, value: e.target.value })} />
          </div>
        </Dialog>
      )}

      {(ask?.kind === "move" || ask?.kind === "copy") && (
        <FolderPicker
          sup={sup}
          id={id}
          item={join(dir, ask.entry.name)}
          isDir={ask.entry.is_dir}
          mode={ask.kind}
          start={dir}
          onClose={() => setAsk(null)}
          onPick={(dest) => {
            const a = ask;
            setAsk(null);
            const from = join(dir, a.entry.name);
            if (a.kind === "move" && open && (open.path === from || open.path.startsWith(`${from}/`))) setOpen(null);
            run(async () =>
              a.kind === "move" ? `Movido a ${await sup.moveFile(id, from, dest)}` : `Copiado a ${await sup.copyFile(id, from, dest)}`,
            );
          }}
        />
      )}

      {ask?.kind === "extract" && (
        <Dialog
          title={`Extraer «${ask.entry.name}»`}
          lead={`Se crea la carpeta «${ask.entry.name.replace(/\.zip$/i, "")}» aquí mismo con el contenido del zip. No se sobrescribe nada; si esa carpeta ya existe, no se extrae.`}
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
                  run(async () => {
                    const r = await sup.extractZip(id, join(dir, a.entry.name));
                    const skip = r.skipped.length
                      ? ` Se omitieron ${r.skipped.length} entrada${r.skipped.length > 1 ? "s" : ""} inseguras (rutas fuera de la carpeta, enlaces o nombres no válidos en Windows).`
                      : "";
                    return `Extraído en ${r.folder}: ${r.files} archivo${r.files === 1 ? "" : "s"}, ${formatSize(r.bytes)}.${skip}`;
                  });
                }}
              >
                Extraer
              </button>
            </>
          }
        >
          <p className="field-help">
            Por seguridad se ignoran las entradas que intentan salir de la carpeta, los enlaces y los nombres que Windows no admite.
          </p>
        </Dialog>
      )}

      {ask?.kind === "trash" && (
        <Dialog
          title={`Mover «${ask.entry.name}» a la papelera`}
          lead={
            ask.entry.is_dir
              ? "La carpeta y todo su contenido salen del servidor y se guardan en la papelera de MercHosting. No se borra nada de tu PC."
              : "El archivo sale del servidor y se guarda en la papelera de MercHosting. No se borra nada de tu PC."
          }
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
                  const a = ask;
                  setAsk(null);
                  if (open && (open.path === join(dir, a.entry.name) || open.path.startsWith(`${join(dir, a.entry.name)}/`))) setOpen(null);
                  run(async () => `Movido a la papelera: ${await sup.trashFile(id, join(dir, a.entry.name))}`);
                }}
              >
                Mover a la papelera
              </button>
            </>
          }
        >
          {ask.entry.is_dir && /^(world|world_nether|world_the_end)$/i.test(ask.entry.name) && (
            <p className="form-error">Atención: esta carpeta parece un mundo. Sin ella, el servidor creará un mundo nuevo.</p>
          )}
        </Dialog>
      )}
    </div>
  );
}
