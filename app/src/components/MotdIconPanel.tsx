// Pestaña «MOTD e icono»: el mensaje y la imagen que se ven en la lista de servidores de
// Minecraft. Cualquier foto se convierte a 64×64 y se guarda como server-icon.png; el MOTD se
// edita con botones de colores y formato y se ve en vivo cómo quedará.

import { AlignCenter, Bold, Image as ImageIcon, Italic, RemoveFormatting, RotateCcw, Save, Sparkles, Strikethrough, Trash2, Underline, Upload } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { IconFit, IconPreview, InstanceId, ServerIconInfo } from "../api/types";
import { MercError } from "../api/types";
import { applyToSelection, centerLine, COLORS, FORMATS, fromEditor, hexFor, layout, SYMBOLS, toEditor, type Segment } from "../motd";

const IMAGE_EXT = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff"];
const OBF = "abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNOPQRSTUVWXYZ0123456789#$%&?";
const FORMAT_ICON: Record<string, React.ReactNode> = {
  l: <Bold size={15} aria-hidden="true" />,
  o: <Italic size={15} aria-hidden="true" />,
  n: <Underline size={15} aria-hidden="true" />,
  m: <Strikethrough size={15} aria-hidden="true" />,
  k: <Sparkles size={15} aria-hidden="true" />,
};

function useTick(on: boolean) {
  const [t, setT] = useState(0);
  useEffect(() => {
    if (!on) return;
    const h = setInterval(() => setT((x) => x + 1), 90);
    return () => clearInterval(h);
  }, [on]);
  return t;
}

function McText({ segs, tick }: { segs: Segment[]; tick: number }) {
  return (
    <>
      {segs.map((s, i) => {
        const color = hexFor(s);
        const deco = [s.underline && "underline", s.strike && "line-through"].filter(Boolean).join(" ");
        const text = s.obfuscated ? [...s.text].map((ch, j) => (ch === " " ? " " : OBF[(tick * 7 + j * 13 + i * 5) % OBF.length])).join("") : s.text;
        return (
          <span
            key={i}
            style={{
              color,
              fontWeight: s.bold ? 800 : undefined,
              fontStyle: s.italic ? "italic" : undefined,
              textDecoration: deco || undefined,
            }}
          >
            {text}
          </span>
        );
      })}
    </>
  );
}

interface Props {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  name: string;
  running: boolean;
  /** Jugadores conectados ahora (dato real; 0 si está apagado). */
  online: number;
  /** El icono se elige de archivos de esta PC: solo el dueño. */
  remote?: boolean;
  canEdit?: boolean;
}

export function MotdIconPanel({ api, sup, id, name, running, online, remote = false, canEdit = true }: Props) {
  const [lines, setLines] = useState<[string, string]>(["", ""]);
  const [saved, setSaved] = useState<string | null>(null);
  const [modified, setModified] = useState<number | null>(null);
  const [maxPlayers, setMaxPlayers] = useState<string>("20");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [focused, setFocused] = useState<0 | 1>(0);
  const inputs = [useRef<HTMLInputElement>(null), useRef<HTMLInputElement>(null)];
  const sel = useRef<[number, number]>([0, 0]);

  const [icon, setIcon] = useState<ServerIconInfo | null>(null);
  const [preview, setPreview] = useState<{ source: string; fit: IconFit; data: IconPreview } | null>(null);
  const [iconBusy, setIconBusy] = useState(false);
  const [iconError, setIconError] = useState<string | null>(null);
  const [iconNote, setIconNote] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const p = await sup.properties(id);
      const motd = p.entries.find((e) => e.key === "motd")?.value ?? "A Minecraft Server";
      const [a = "", b = ""] = toEditor(motd).split("\n");
      setLines([a, b]);
      setSaved(motd);
      setModified(p.modified_unix_ms);
      setMaxPlayers(p.entries.find((e) => e.key === "max-players")?.value ?? "20");
    } catch (e) {
      setError(errorMessage(e));
    }
    try {
      setIcon(await sup.serverIcon(id));
    } catch (e) {
      setIconError(errorMessage(e));
    }
  }, [sup, id]);

  useEffect(() => {
    load();
  }, [load]);

  const motd = fromEditor(lines[1] ? `${lines[0]}\n${lines[1]}` : lines[0]);
  const dirty = saved !== null && motd !== saved;
  const view = layout(motd);
  const hasObf = view.lines.some((l) => l.some((s) => s.obfuscated));
  const tick = useTick(hasObf);

  function remember(i: 0 | 1) {
    const el = inputs[i].current;
    if (el) sel.current = [el.selectionStart ?? el.value.length, el.selectionEnd ?? el.value.length];
    setFocused(i);
  }

  function setLine(i: 0 | 1, v: string) {
    setLines((l) => (i === 0 ? [v, l[1]] : [l[0], v]));
    setNote(null);
  }

  function apply(code: string) {
    const i = focused;
    const [s, e] = sel.current;
    const r = applyToSelection(lines[i], Math.min(s, lines[i].length), Math.min(e, lines[i].length), code);
    setLine(i, r.text);
    sel.current = [r.cursor, r.cursor];
    requestAnimationFrame(() => {
      const el = inputs[i].current;
      el?.focus();
      el?.setSelectionRange(r.cursor, r.cursor);
    });
  }

  function insert(text: string) {
    const i = focused;
    const [s, e] = sel.current;
    const v = lines[i];
    const t = v.slice(0, s) + text + v.slice(e);
    setLine(i, t);
    const c = s + text.length;
    sel.current = [c, c];
    requestAnimationFrame(() => {
      inputs[i].current?.focus();
      inputs[i].current?.setSelectionRange(c, c);
    });
  }

  async function save() {
    setSaving(true);
    setError(null);
    setNote(null);
    try {
      const p = await sup.setProperties(id, { motd }, modified);
      setSaved(motd);
      setModified(p.modified_unix_ms);
      setNote(running ? "MOTD guardado. Se verá en la lista después de reiniciar el servidor." : "MOTD guardado. Se verá al encender el servidor.");
    } catch (e) {
      setError(e instanceof MercError && e.code === "conflict" ? "Otro programa cambió server.properties. Recarga para ver lo nuevo." : errorMessage(e));
    } finally {
      setSaving(false);
    }
  }

  function reset() {
    if (saved === null) return;
    const [a = "", b = ""] = toEditor(saved).split("\n");
    setLines([a, b]);
    setNote(null);
  }

  async function pickImage() {
    setIconError(null);
    setIconNote(null);
    const source = await api.pickFile({ name: "Imágenes", extensions: IMAGE_EXT });
    if (!source) return;
    await showPreview(source, "crop");
  }

  async function showPreview(source: string, fit: IconFit) {
    setIconBusy(true);
    setIconError(null);
    try {
      setPreview({ source, fit, data: await sup.previewServerIcon(source, fit) });
    } catch (e) {
      setIconError(errorMessage(e));
    } finally {
      setIconBusy(false);
    }
  }

  async function useIcon() {
    if (!preview) return;
    setIconBusy(true);
    setIconError(null);
    try {
      const had = icon?.present;
      setIcon(await sup.setServerIcon(id, preview.source, preview.fit));
      setPreview(null);
      setIconNote(
        `Icono guardado como server-icon.png (64×64).${had ? " El anterior está en la papelera de MercHosting." : ""}${running ? " Se verá después de reiniciar el servidor." : ""}`,
      );
    } catch (e) {
      setIconError(errorMessage(e));
    } finally {
      setIconBusy(false);
    }
  }

  async function removeIcon() {
    setIconBusy(true);
    setIconError(null);
    try {
      setIcon(await sup.removeServerIcon(id));
      setIconNote("Icono quitado. Está en la papelera de MercHosting por si lo quieres recuperar.");
    } catch (e) {
      setIconError(errorMessage(e));
    } finally {
      setIconBusy(false);
    }
  }

  const shownIcon = preview?.data.png_base64 ?? (icon?.valid ? icon.png_base64 : null);
  const fileName = preview ? preview.source.split(/[\\/]/).pop() : null;

  return (
    <div className="admin-panel motd-panel" role="tabpanel" aria-label="MOTD e icono">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">
            <ImageIcon size={18} aria-hidden="true" /> MOTD e icono
          </h2>
          <p className="field-help">Lo que ven los jugadores en su lista de servidores antes de entrar.</p>
        </div>
      </div>

      <section className="props-card" aria-label="Vista previa">
        <h3>Vista previa</h3>
        <div className="mc-entry" data-testid="mc-preview">
          <div className="mc-icon">
            {shownIcon ? <img src={`data:image/png;base64,${shownIcon}`} alt="Icono del servidor" width={64} height={64} /> : <span aria-label="Sin icono">?</span>}
          </div>
          <div className="mc-body">
            <div className="mc-top">
              <span className="mc-name" translate="no">{name}</span>
              <span className="mc-players">
                {online}/{maxPlayers}
              </span>
            </div>
            {view.lines.map((l, i) => (
              <div key={i} className="mc-line" data-testid={`mc-line-${i + 1}`}>
                <McText segs={l} tick={tick} />
              </div>
            ))}
          </div>
        </div>
        <p className="field-help">
          Aproximada: el juego usa su propia letra. Lo que no cabe en una línea baja a la siguiente y solo se ven dos.
        </p>
        {view.hidden && (
          <p className="warn-text" role="status">
            El texto es más largo de lo que cabe: el final no se verá en la lista.
          </p>
        )}
      </section>

      <section className="props-card" aria-label="Icono del servidor">
        <h3>Icono</h3>
        {icon?.present && !icon.valid && !preview && (
          <p className="warn-text">
            El server-icon.png actual mide {icon.width}×{icon.height} o no es PNG: Minecraft no lo mostrará. Elige una imagen para
            arreglarlo.
          </p>
        )}
        {preview ? (
          <div className="icon-preview">
            <img src={`data:image/png;base64,${preview.data.png_base64}`} alt="Icono nuevo" width={128} height={128} className="pixelated" />
            <div>
              <p>
                <strong>{fileName}</strong> · {preview.data.format} {preview.data.source_width}×{preview.data.source_height} → 64×64
              </p>
              <fieldset className="seg-choice">
                <legend className="sr-only">Cómo encajar la imagen</legend>
                <label>
                  <input type="radio" name="icon-fit" checked={preview.fit === "crop"} disabled={iconBusy} onChange={() => showPreview(preview.source, "crop")} />
                  Recortar al centro
                </label>
                <label>
                  <input
                    type="radio"
                    name="icon-fit"
                    checked={preview.fit === "contain"}
                    disabled={iconBusy}
                    onChange={() => showPreview(preview.source, "contain")}
                  />
                  Imagen completa (con bordes transparentes)
                </label>
              </fieldset>
              <div className="row-actions">
                <button type="button" className="btn btn-sm btn-go" disabled={iconBusy} onClick={useIcon}>
                  <Save size={14} aria-hidden="true" /> Usar como icono
                </button>
                <button type="button" className="btn btn-sm btn-quiet" disabled={iconBusy} onClick={() => setPreview(null)}>
                  Cancelar
                </button>
              </div>
            </div>
          </div>
        ) : remote ? (
          <p className="field-help">Solo el dueño puede cambiar el icono (se elige de archivos de su PC).</p>
        ) : (
          <div className="row-actions">
            <button type="button" className="btn btn-sm btn-go" disabled={iconBusy} onClick={pickImage}>
              <Upload size={14} aria-hidden="true" /> {icon?.present ? "Cambiar imagen…" : "Elegir imagen…"}
            </button>
            {icon?.present && (
              <button type="button" className="btn btn-sm btn-quiet" disabled={iconBusy} onClick={removeIcon}>
                <Trash2 size={14} aria-hidden="true" /> Quitar icono
              </button>
            )}
          </div>
        )}
        {!remote && !preview && (
          <p className="field-help">
            Cualquier foto (PNG, JPG, GIF, WebP, BMP, ICO o TIFF) se convierte a 64×64 y se guarda como server-icon.png. Verás cómo
            queda antes de guardarla.
          </p>
        )}
        {iconNote && (
          <p className="ok-text" role="status">
            {iconNote}
          </p>
        )}
        {iconError && (
          <p className="form-error" role="alert">
            {iconError}
          </p>
        )}
      </section>

      <section className="props-card" aria-label="Editor de MOTD">
        <h3>MOTD</h3>
        {canEdit ? (
          <>
            <div className="motd-toolbar" role="toolbar" aria-label="Formato del MOTD">
              <div className="motd-colors">
                {COLORS.map((c) => (
                  <button
                    key={c.code}
                    type="button"
                    className="motd-swatch"
                    style={{ background: c.hex }}
                    title={`${c.name} (&${c.code})`}
                    aria-label={`Color ${c.name}`}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => apply(c.code)}
                  />
                ))}
              </div>
              <div className="motd-formats">
                {FORMATS.map((f) => (
                  <button
                    key={f.code}
                    type="button"
                    className="btn btn-sm btn-quiet"
                    title={`${f.name} (&${f.code})`}
                    aria-label={f.name}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => apply(f.code)}
                  >
                    {FORMAT_ICON[f.code]}
                  </button>
                ))}
                <button
                  type="button"
                  className="btn btn-sm btn-quiet"
                  title="Quitar formato (&r)"
                  aria-label="Quitar formato"
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => apply("r")}
                >
                  <RemoveFormatting size={15} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className="btn btn-sm btn-quiet"
                  title="Centrar la línea (aproximado)"
                  aria-label="Centrar línea"
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => setLine(focused, centerLine(lines[focused]))}
                >
                  <AlignCenter size={15} aria-hidden="true" />
                </button>
              </div>
              <div className="motd-symbols" aria-label="Símbolos">
                {SYMBOLS.map((s) => (
                  <button key={s} type="button" className="btn btn-sm btn-quiet" aria-label={`Insertar ${s}`} onMouseDown={(e) => e.preventDefault()} onClick={() => insert(s)}>
                    {s}
                  </button>
                ))}
              </div>
            </div>
            {([0, 1] as const).map((i) => (
              <div key={i} className="field">
                <label htmlFor={`motd-${i}`}>Línea {i + 1}</label>
                <input
                  id={`motd-${i}`}
                  ref={inputs[i]}
                  type="text"
                  className="mono"
                  spellCheck={false}
                  maxLength={400}
                  value={lines[i]}
                  onFocus={() => remember(i)}
                  onSelect={() => remember(i)}
                  onKeyUp={() => remember(i)}
                  onChange={(e) => {
                    setLine(i, e.target.value.replace(/[\r\n]/g, ""));
                    remember(i);
                  }}
                />
              </div>
            ))}
            <p className="field-help">
              Escribe y usa los botones: se aplican donde está el cursor o a lo que tengas seleccionado. Los códigos se ven como
              «&amp;» seguido de una letra (por ejemplo &amp;6 es dorado y &amp;l negrita).
            </p>
            <div className="row-actions">
              <button type="button" className="btn btn-sm btn-go" disabled={!dirty || saving} onClick={save}>
                <Save size={14} aria-hidden="true" /> {saving ? "Guardando…" : "Guardar MOTD"}
              </button>
              <button type="button" className="btn btn-sm btn-quiet" disabled={!dirty || saving} onClick={reset}>
                <RotateCcw size={14} aria-hidden="true" /> Deshacer cambios
              </button>
            </div>
          </>
        ) : (
          <p className="field-help">No tienes permiso para cambiar los ajustes de este servidor.</p>
        )}
        {note && (
          <p className="ok-text" role="status">
            {note}
          </p>
        )}
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
      </section>
    </div>
  );
}
