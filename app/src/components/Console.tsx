import { Search, Send, Star, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { ConsoleLine, InstanceId } from "../api/types";
import { type Danger, dangerOf, loadFavorites, saveFavorites } from "../consoleCommands";
import { Dialog } from "./Dialog";
import { uiLocale } from "../i18n/lang";

const MAX_LINES = 3000;
const POLL_MS = 400;

/** Une líneas nuevas sin duplicar (por número de secuencia) y con límite. */
export function mergeLines(prev: ConsoleLine[], incoming: ConsoleLine[]): ConsoleLine[] {
  if (incoming.length === 0) return prev;
  const last = prev.length ? prev[prev.length - 1].seq : 0;
  const fresh = incoming.filter((l) => l.seq > last);
  if (fresh.length === 0) return prev;
  const all = prev.concat(fresh);
  return all.length > MAX_LINES ? all.slice(all.length - MAX_LINES) : all;
}

/**
 * Lee la consola del supervisor de forma incremental. Solo se reinicia si cambia el servidor
 * (antes se reiniciaba en cada refresco de la lista y la consola parpadeaba).
 */
export function useConsoleLines(id: InstanceId, sup: Supervisor, enabled = true): ConsoleLine[] {
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const supRef = useRef(sup);
  supRef.current = sup;

  useEffect(() => {
    let alive = true;
    let after = 0;
    setLines([]);
    if (!enabled) return;
    async function tick() {
      try {
        for (let i = 0; i < 5 && alive; i++) {
          const r = await supRef.current.console(id, after);
          if (!alive || r.lines.length === 0) break;
          after = r.next_after;
          setLines((prev) => mergeLines(prev, r.lines));
          if (r.lines.length < 2000) break;
        }
      } catch {
        // La pérdida de conexión ya se muestra en la barra lateral; aquí se reintenta.
      }
    }
    tick();
    const t = setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [id, enabled]);

  return lines;
}

export type Level = "info" | "warn" | "error" | "system";

/** Nivel de una línea: lo que dice Minecraft/Forge/Paper entre corchetes, o el canal. */
export function levelOf(l: ConsoleLine): Level {
  if (l.stream === "system") return "system";
  const m = /\[(?:[^\]]*[/\s])?(INFO|WARN|WARNING|ERROR|FATAL|SEVERE|DEBUG|TRACE)\]/.exec(l.text);
  const tag = m?.[1];
  if (tag === "WARN" || tag === "WARNING") return "warn";
  if (tag === "ERROR" || tag === "FATAL" || tag === "SEVERE") return "error";
  if (tag) return "info";
  if (l.stream === "stderr") return "error";
  if (/^\s*(at |Caused by:|\S+(Exception|Error)(:|$))/.test(l.text)) return "error";
  return "info";
}

const clock = (ms: number) => new Date(ms).toLocaleTimeString(uiLocale(), { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });

function Line({ l, time = false }: { l: ConsoleLine; time?: boolean }) {
  const level = levelOf(l);
  return (
    <div className={`ln ln-${l.stream} ln-${level}`}>
      {time && <span className="ln-time">{clock(l.at_unix_ms)}</span>}
      {l.stream === "system" ? <span className="ln-tag">MercHosting</span> : null}
      {l.text}
    </div>
  );
}

/** Vista breve de las últimas líneas (pestaña Resumen). */
export function ConsolePeek({ lines, count = 7 }: { lines: ConsoleLine[]; count?: number }) {
  const last = lines.slice(-count);
  return (
    <div className="peek" aria-label="Últimas líneas de la consola">
      {last.length === 0 ? <p className="muted">Todavía no hay actividad en la consola.</p> : last.map((l) => <Line key={l.seq} l={l} />)}
    </div>
  );
}

interface Props {
  id: InstanceId;
  sup: Supervisor;
  lines: ConsoleLine[];
  canSend: boolean;
  disabledReason: string;
}

const LEVELS: { key: Exclude<Level, "system">; label: string }[] = [
  { key: "info", label: "Info" },
  { key: "warn", label: "Avisos" },
  { key: "error", label: "Errores" },
];

export function Console({ id, sup, lines, canSend, disabledReason }: Props) {
  const [filter, setFilter] = useState("");
  const [follow, setFollow] = useState(true);
  const [showTime, setShowTime] = useState(false);
  const [hidden, setHidden] = useState<Set<Level>>(new Set());
  const [command, setCommand] = useState("");
  const [sendError, setSendError] = useState<string | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [histPos, setHistPos] = useState(-1);
  const [favs, setFavs] = useState<string[]>(() => loadFavorites(id));
  const [confirm, setConfirm] = useState<{ text: string; danger: Danger } | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => setFavs(loadFavorites(id)), [id]);

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines, follow, filter, hidden]);

  async function deliver(text: string) {
    setSendError(null);
    try {
      await sup.sendCommand(id, text);
      setHistory((h) => [text, ...h.filter((x) => x !== text)].slice(0, 50));
      setHistPos(-1);
      setCommand("");
    } catch (err) {
      setSendError(errorMessage(err));
    }
  }

  async function send(e: React.FormEvent) {
    e.preventDefault();
    const text = command.trim();
    if (!text) return;
    const danger = dangerOf(text);
    if (danger) setConfirm({ text, danger });
    else await deliver(text);
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "ArrowUp" && history.length) {
      e.preventDefault();
      const p = Math.min(histPos + 1, history.length - 1);
      setHistPos(p);
      setCommand(history[p]);
    } else if (e.key === "ArrowDown" && histPos >= 0) {
      e.preventDefault();
      const p = histPos - 1;
      setHistPos(p);
      setCommand(p >= 0 ? history[p] : "");
    }
  }

  function setFavList(next: string[]) {
    setFavs(next);
    saveFavorites(id, next);
  }
  const current = command.trim().replace(/^\/+/, "");
  const isFav = !!current && favs.includes(current);

  const counts = useMemo(() => {
    const c: Record<Level, number> = { info: 0, warn: 0, error: 0, system: 0 };
    lines.forEach((l) => c[levelOf(l)]++);
    return c;
  }, [lines]);

  const q = filter.trim().toLowerCase();
  const shown = lines.filter((l) => !hidden.has(levelOf(l)) && (!q || l.text.toLowerCase().includes(q)));

  const toggleLevel = (k: Level) =>
    setHidden((h) => {
      const n = new Set(h);
      if (n.has(k)) n.delete(k);
      else n.add(k);
      return n;
    });

  return (
    <section className="console card" aria-label="Consola del servidor">
      <div className="console-bar">
        <div className="console-search">
          <Search size={15} aria-hidden="true" />
          <input
            type="search"
            placeholder="Filtrar líneas"
            aria-label="Filtrar líneas de la consola"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
        <div className="console-levels" role="group" aria-label="Niveles">
          {LEVELS.map((lv) => (
            <button
              key={lv.key}
              type="button"
              className={`lvl-chip lvl-${lv.key}`}
              aria-pressed={!hidden.has(lv.key)}
              onClick={() => toggleLevel(lv.key)}
              title={hidden.has(lv.key) ? `Mostrar ${lv.label.toLowerCase()}` : `Ocultar ${lv.label.toLowerCase()}`}
            >
              {lv.label} <span className="lvl-n">{counts[lv.key]}</span>
            </button>
          ))}
        </div>
        <label className="toggle toggle-sm">
          <input type="checkbox" checked={showTime} onChange={(e) => setShowTime(e.target.checked)} />
          <span className="toggle-track" aria-hidden="true" />
          Hora
        </label>
        <label className="toggle toggle-sm">
          <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />
          <span className="toggle-track" aria-hidden="true" />
          Seguir el final
        </label>
      </div>
      <div className="console-lines" ref={box} tabIndex={0} aria-label="Salida de la consola">
        {shown.length === 0 ? (
          <p className="muted console-empty">
            {lines.length === 0 ? "Aquí aparecerá lo que escriba el servidor cuando lo inicies." : "Ninguna línea coincide con el filtro."}
          </p>
        ) : (
          shown.map((l) => <Line key={l.seq} l={l} time={showTime} />)
        )}
      </div>
      {favs.length > 0 && (
        <div className="console-favs" role="list" aria-label="Comandos favoritos">
          {favs.map((f) => (
            <span key={f} role="listitem" className="fav-chip">
              <button
                type="button"
                className="fav-use"
                title="Poner en la línea de comando (no se envía hasta que pulses Enviar)"
                onClick={() => {
                  setCommand(f);
                  input.current?.focus();
                }}
              >
                /{f}
              </button>
              <button type="button" className="fav-x" aria-label={`Quitar /${f} de favoritos`} onClick={() => setFavList(favs.filter((x) => x !== f))}>
                <X size={12} aria-hidden="true" />
              </button>
            </span>
          ))}
        </div>
      )}
      <form className="console-input" onSubmit={send}>
        <span className="prompt" aria-hidden="true">
          /
        </span>
        <label htmlFor="console-command" className="sr-only">
          Comando de Minecraft
        </label>
        <input
          id="console-command"
          ref={input}
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          onKeyDown={onKeyDown}
          disabled={!canSend}
          placeholder={canSend ? "Escribe un comando, por ejemplo: say Hola a todos" : disabledReason}
          autoComplete="off"
          spellCheck={false}
        />
        <button
          type="button"
          className="btn btn-icon-only btn-quiet"
          disabled={!current}
          aria-pressed={isFav}
          aria-label={isFav ? "Quitar de favoritos" : "Guardar en favoritos"}
          title={isFav ? "Quitar de favoritos" : "Guardar este comando en favoritos"}
          onClick={() => setFavList(isFav ? favs.filter((x) => x !== current) : [...favs, current])}
        >
          <Star size={16} aria-hidden="true" fill={isFav ? "currentColor" : "none"} />
        </button>
        <button type="submit" className="btn btn-icon-only" disabled={!canSend || !command.trim()} aria-label="Enviar">
          <Send size={16} aria-hidden="true" />
        </button>
      </form>
      {sendError && (
        <p className="form-error console-error" role="alert">
          {sendError}
        </p>
      )}
      {confirm && (
        <Dialog
          title="¿Enviar este comando?"
          lead={<span className="mono">/{confirm.text.replace(/^\/+/, "")}</span>}
          onClose={() => setConfirm(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirm(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  const t = confirm.text;
                  setConfirm(null);
                  void deliver(t);
                }}
              >
                Enviar igualmente
              </button>
            </>
          }
        >
          <p>{confirm.danger.why}</p>
          {confirm.danger.instead && <p className="field-help">{confirm.danger.instead}</p>}
        </Dialog>
      )}
    </section>
  );
}
