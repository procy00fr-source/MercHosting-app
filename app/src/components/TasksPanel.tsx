// Pestaña Tareas programadas: copias automáticas, reinicios y apagados con aviso, encender,
// comandos y mensajes, a ciertas horas o cada cierto tiempo. Las ejecuta el supervisor aunque la
// ventana esté cerrada.

import {
  Archive,
  CalendarClock,
  CirclePlay,
  MessageSquare,
  Pencil,
  Play,
  Plus,
  Power,
  RefreshCw,
  RotateCw,
  SquareTerminal,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId, PlayersPolicy, Schedule, ScheduledTask, TaskAction, TaskDraft, TaskList } from "../api/types";
import { Dialog } from "./Dialog";
import { uiLocale } from "../i18n/lang";

const DAYS = ["L", "M", "X", "J", "V", "S", "D"];
const DAY_NAMES = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];

type Kind = "backup_world" | "backup_full" | "restart" | "stop" | "start" | "command" | "announce";

const KIND_LABEL: Record<Kind, string> = {
  backup_world: "Copia del mundo",
  backup_full: "Copia completa",
  restart: "Reiniciar el servidor",
  stop: "Apagar el servidor",
  start: "Encender el servidor",
  command: "Comando de consola",
  announce: "Mensaje en el chat",
};

function kindOf(a: TaskAction): Kind {
  if (a.type === "backup") return a.scope === "world" ? "backup_world" : "backup_full";
  return a.type;
}

function notice(s: number): string {
  if (s === 0) return "sin aviso";
  return s >= 60 && s % 60 === 0 ? `aviso de ${s / 60} min` : `aviso de ${s} s`;
}

export function describeAction(a: TaskAction): string {
  switch (a.type) {
    case "backup":
      return a.scope === "world" ? "Copia del mundo" : "Copia completa";
    case "restart":
    case "stop":
      return `${a.type === "restart" ? "Reiniciar" : "Apagar"} (${notice(a.notice_seconds)}${
        a.if_players === "skip" ? ", solo si no hay nadie" : ", avisando a los jugadores"
      })`;
    case "start":
      return "Encender el servidor";
    case "command":
      return `Comando: ${a.command}`;
    case "announce":
      return `Mensaje: «${a.message}»`;
  }
}

export function describeSchedule(s: Schedule): string {
  if (s.type === "every") {
    return s.minutes % 60 === 0 ? `Cada ${s.minutes / 60} h` : `Cada ${s.minutes} min`;
  }
  const days =
    s.days.length === 0 || s.days.length === 7
      ? "Todos los días"
      : s.days.length === 5 && [1, 2, 3, 4, 5].every((d) => s.days.includes(d))
        ? "De lunes a viernes"
        : s.days.map((d) => DAY_NAMES[d - 1]).join(", ").replace(/^./, (c) => c.toUpperCase());
  return `${days} a las ${s.times.join(", ")}`;
}

function when(ms: number): string {
  return new Date(ms).toLocaleString(uiLocale(), { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

function offsetText(min: number): string {
  const sign = min < 0 ? "−" : "+";
  const a = Math.abs(min);
  return `UTC${sign}${String(Math.floor(a / 60)).padStart(2, "0")}:${String(a % 60).padStart(2, "0")}`;
}

function ActionIcon({ a }: { a: TaskAction }) {
  const p = { size: 20, "aria-hidden": true } as const;
  switch (a.type) {
    case "backup":
      return <Archive {...p} />;
    case "restart":
      return <RotateCw {...p} />;
    case "stop":
      return <Power {...p} />;
    case "start":
      return <CirclePlay {...p} />;
    case "command":
      return <SquareTerminal {...p} />;
    case "announce":
      return <MessageSquare {...p} />;
  }
}

/** Estado del editor (con textos tal como los escribe la persona). */
interface Form {
  id: string | null;
  enabled: boolean;
  kind: Kind;
  noticeSeconds: number;
  ifPlayers: PlayersPolicy;
  text: string;
  mode: "daily" | "every";
  times: string[];
  days: number[];
  every: string;
  unit: "min" | "h";
}

function formFrom(t: ScheduledTask | null, preset?: Partial<Form>): Form {
  const base: Form = {
    id: null,
    enabled: true,
    kind: "backup_world",
    noticeSeconds: 300,
    ifPlayers: "skip",
    text: "",
    mode: "daily",
    times: ["04:00"],
    days: [],
    every: "1",
    unit: "h",
  };
  if (!t) return { ...base, ...preset };
  const a = t.action;
  const s = t.schedule;
  return {
    id: t.id,
    enabled: t.enabled,
    kind: kindOf(a),
    noticeSeconds: a.type === "restart" || a.type === "stop" ? a.notice_seconds : 300,
    ifPlayers: a.type === "restart" || a.type === "stop" ? a.if_players : "skip",
    text: a.type === "command" ? a.command : a.type === "announce" ? a.message : "",
    mode: s.type,
    times: s.type === "daily" ? s.times : ["04:00"],
    days: s.type === "daily" ? s.days : [],
    every: s.type === "every" ? String(s.minutes % 60 === 0 ? s.minutes / 60 : s.minutes) : "1",
    unit: s.type === "every" && s.minutes % 60 !== 0 ? "min" : "h",
  };
}

function draftFrom(f: Form): TaskDraft | string {
  let action: TaskAction;
  switch (f.kind) {
    case "backup_world":
      action = { type: "backup", scope: "world" };
      break;
    case "backup_full":
      action = { type: "backup", scope: "full" };
      break;
    case "restart":
    case "stop":
      action = { type: f.kind, notice_seconds: f.noticeSeconds, if_players: f.ifPlayers };
      break;
    case "start":
      action = { type: "start" };
      break;
    case "command":
      if (!f.text.trim()) return "Escribe el comando.";
      action = { type: "command", command: f.text.trim() };
      break;
    case "announce":
      if (!f.text.trim()) return "Escribe el mensaje.";
      action = { type: "announce", message: f.text.trim() };
      break;
  }
  let schedule: Schedule;
  if (f.mode === "daily") {
    const times = f.times.map((t) => t.trim()).filter(Boolean);
    if (times.length === 0) return "Elige al menos una hora.";
    if (times.some((t) => !/^\d{1,2}:\d{2}$/.test(t))) return "Las horas van como HH:MM.";
    schedule = { type: "daily", times, days: f.days };
  } else {
    const n = Number(f.every);
    const minutes = f.unit === "h" ? n * 60 : n;
    if (!Number.isInteger(n) || minutes < 5 || minutes > 10080) return "Entre 5 minutos y 7 días.";
    schedule = { type: "every", minutes };
  }
  return { id: f.id, enabled: f.enabled, action, schedule };
}

const PRESETS: { label: string; form: Partial<Form> }[] = [
  { label: "Copia diaria a las 04:00", form: { kind: "backup_world", mode: "daily", times: ["04:00"] } },
  {
    label: "Reinicio diario con aviso",
    form: { kind: "restart", noticeSeconds: 300, ifPlayers: "notify", mode: "daily", times: ["05:00"] },
  },
  { label: "Anuncio cada hora", form: { kind: "announce", mode: "every", every: "1", unit: "h" } },
];

interface Props {
  sup: Supervisor;
  id: InstanceId;
}

export function TasksPanel({ sup, id }: Props) {
  const [list, setList] = useState<TaskList | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [form, setForm] = useState<Form | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [del, setDel] = useState<ScheduledTask | null>(null);

  const load = useCallback(async () => {
    try {
      setList(await sup.tasks(id));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  useEffect(() => {
    load();
  }, [load]);

  // Mientras alguna se ejecuta (o tras «Ejecutar ahora»), se refresca para ver el resultado.
  const anyRunning = list?.items.some((t) => t.running) ?? false;
  useEffect(() => {
    if (!anyRunning) return;
    const t = setInterval(load, 1000);
    return () => clearInterval(t);
  }, [anyRunning, load]);

  async function act(fn: () => Promise<TaskList>, msg: string) {
    setError(null);
    setNote(null);
    try {
      setList(await fn());
      setNote(msg);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function save() {
    if (!form) return;
    const d = draftFrom(form);
    if (typeof d === "string") {
      setFormError(d);
      return;
    }
    setSaving(true);
    setFormError(null);
    try {
      setList(await sup.saveTask(id, d));
      setNote(form.id ? "Tarea guardada." : "Tarea creada.");
      setForm(null);
    } catch (e) {
      setFormError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  }

  const items = list?.items ?? [];
  const toggleDay = (d: number) =>
    form && setForm({ ...form, days: form.days.includes(d) ? form.days.filter((x) => x !== d) : [...form.days, d].sort() });

  return (
    <div className="admin-panel tasks-panel" role="tabpanel" aria-label="Tareas programadas">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">Tareas programadas</h2>
          <p className="field-help">
            Usan la hora de esta PC{list ? ` (${offsetText(list.utc_offset_minutes)})` : ""}. Las hace el supervisor aunque cierres
            la ventana, pero la PC tiene que estar encendida: si a esa hora estaba apagada o dormida, la tarea se salta y queda
            anotado.
          </p>
        </div>
        <button type="button" className="btn btn-sm" onClick={load}>
          <RefreshCw size={14} aria-hidden="true" /> Actualizar
        </button>
        <button
          type="button"
          className="btn btn-sm btn-go"
          onClick={() => {
            setFormError(null);
            setForm(formFrom(null));
          }}
        >
          <Plus size={14} aria-hidden="true" /> Nueva tarea
        </button>
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

      {list === null && !error ? (
        <p className="wiz-loading" role="status">
          Leyendo tareas…
        </p>
      ) : items.length === 0 ? (
        <div className="card empty-small">
          <h2>Todavía no hay tareas</h2>
          <p>Empieza con una de estas (podrás cambiarla antes de guardar):</p>
          <div className="files-bar">
            {PRESETS.map((p) => (
              <button
                key={p.label}
                type="button"
                className="btn btn-sm"
                onClick={() => {
                  setFormError(null);
                  setForm(formFrom(null, p.form));
                }}
              >
                <CalendarClock size={14} aria-hidden="true" /> {p.label}
              </button>
            ))}
          </div>
        </div>
      ) : (
        <ul className="world-list" aria-label="Tareas">
          {items.map((t) => (
            <li key={t.id} className="card world-card task-card" data-enabled={t.enabled}>
              <span className="world-icon">
                <ActionIcon a={t.action} />
              </span>
              <span className="world-main">
                <span className="world-name">
                  {describeAction(t.action)}
                  {!t.enabled && <span className="badge badge-muted">Pausada</span>}
                  {t.running && <span className="badge">Ejecutándose…</span>}
                </span>
                <span className="world-meta">
                  {describeSchedule(t.schedule)}
                  {t.enabled && t.next_run_unix_ms ? ` · Próxima: ${when(t.next_run_unix_ms)}` : ""}
                </span>
                {t.last_run && (
                  <span className="world-meta">
                    <span className={t.last_run.outcome === "ok" ? "ok-text" : "warn-text"}>
                      {t.last_run.outcome === "ok" ? "Hecha" : t.last_run.outcome === "skipped" ? "Omitida" : "Falló"}
                    </span>{" "}
                    {when(t.last_run.at_unix_ms)}
                    {t.last_run.manual ? " (a mano)" : ""} · {t.last_run.message}
                  </span>
                )}
              </span>
              <span className="world-actions">
                <label className="switch-row task-switch" title={t.enabled ? "Pausar" : "Activar"}>
                  <input
                    type="checkbox"
                    checked={t.enabled}
                    aria-label={`${t.enabled ? "Pausar" : "Activar"}: ${describeAction(t.action)}`}
                    onChange={(e) =>
                      act(
                        () => sup.saveTask(id, { id: t.id, enabled: e.target.checked, action: t.action, schedule: t.schedule }),
                        e.target.checked ? "Tarea activada." : "Tarea pausada: no se ejecutará hasta que la actives.",
                      )
                    }
                  />
                </label>
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={t.running}
                  aria-label={`Ejecutar ahora: ${describeAction(t.action)}`}
                  onClick={() => act(() => sup.runTaskNow(id, t.id), "Ejecutando… el resultado aparece en la tarea.")}
                >
                  <Play size={14} aria-hidden="true" /> Ahora
                </button>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={`Editar: ${describeAction(t.action)}`}
                  onClick={() => {
                    setFormError(null);
                    setForm(formFrom(t));
                  }}
                >
                  <Pencil size={16} aria-hidden="true" />
                </button>
                <button type="button" className="icon-btn" aria-label={`Borrar: ${describeAction(t.action)}`} onClick={() => setDel(t)}>
                  <Trash2 size={16} aria-hidden="true" />
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}

      {form && (
        <Dialog
          title={form.id ? "Editar tarea" : "Nueva tarea"}
          lead="Elige qué hacer y cuándo."
          onClose={() => setForm(null)}
          wide
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setForm(null)}>
                Cancelar
              </button>
              <button type="button" className="btn btn-go" disabled={saving} onClick={save}>
                {saving ? "Guardando…" : "Guardar tarea"}
              </button>
            </>
          }
        >
          <div className="task-form">
            <div className="field">
              <label htmlFor="tk-kind">Qué hacer</label>
              <select id="tk-kind" value={form.kind} onChange={(e) => setForm({ ...form, kind: e.target.value as Kind, text: "" })}>
                {(Object.keys(KIND_LABEL) as Kind[]).map((k) => (
                  <option key={k} value={k}>
                    {KIND_LABEL[k]}
                  </option>
                ))}
              </select>
            </div>

            {(form.kind === "restart" || form.kind === "stop") && (
              <>
                <div className="field">
                  <label htmlFor="tk-notice">Aviso en el chat antes de {form.kind === "restart" ? "reiniciar" : "apagar"}</label>
                  <select id="tk-notice" value={form.noticeSeconds} onChange={(e) => setForm({ ...form, noticeSeconds: Number(e.target.value) })}>
                    {[0, 60, 120, 300, 600].map((s) => (
                      <option key={s} value={s}>
                        {s === 0 ? "Sin aviso" : `${s / 60} min`}
                      </option>
                    ))}
                  </select>
                </div>
                <fieldset className="field">
                  <legend>Si hay jugadores conectados</legend>
                  <label className="radio-row">
                    <input
                      type="radio"
                      name="tk-players"
                      checked={form.ifPlayers === "skip"}
                      onChange={() => setForm({ ...form, ifPlayers: "skip" })}
                    />
                    No hacerlo esa vez
                  </label>
                  <label className="radio-row">
                    <input
                      type="radio"
                      name="tk-players"
                      checked={form.ifPlayers === "notify"}
                      onChange={() => setForm({ ...form, ifPlayers: "notify" })}
                    />
                    Avisarles y hacerlo igual (al menos 30 s de aviso)
                  </label>
                </fieldset>
              </>
            )}
            {form.kind === "command" && (
              <div className="field">
                <label htmlFor="tk-text">Comando (sin la barra)</label>
                <input
                  id="tk-text"
                  type="text"
                  className="mono"
                  maxLength={256}
                  placeholder="weather clear"
                  value={form.text}
                  onChange={(e) => setForm({ ...form, text: e.target.value })}
                />
                <p className="field-help">Para apagar o reiniciar usa sus propias tareas: avisan y respetan a los jugadores.</p>
              </div>
            )}
            {form.kind === "announce" && (
              <div className="field">
                <label htmlFor="tk-text">Mensaje</label>
                <input
                  id="tk-text"
                  type="text"
                  maxLength={200}
                  placeholder="¡Evento de construcción a las 20:00!"
                  value={form.text}
                  onChange={(e) => setForm({ ...form, text: e.target.value })}
                />
              </div>
            )}
            {(form.kind === "command" || form.kind === "announce" || form.kind === "restart" || form.kind === "stop") && (
              <p className="field-help">Si a esa hora el servidor está apagado, esta tarea se salta.</p>
            )}

            <div className="field">
              <span className="field-label">Cuándo</span>
              <div className="seg" role="group" aria-label="Cuándo">
                <button type="button" className="seg-btn" aria-pressed={form.mode === "daily"} onClick={() => setForm({ ...form, mode: "daily" })}>
                  A ciertas horas
                </button>
                <button type="button" className="seg-btn" aria-pressed={form.mode === "every"} onClick={() => setForm({ ...form, mode: "every" })}>
                  Cada cierto tiempo
                </button>
              </div>
            </div>
            {form.mode === "daily" ? (
              <>
                <div className="field">
                  <span className="field-label">Horas</span>
                  <div className="files-bar">
                    {form.times.map((t, i) => (
                      <span key={i} className="time-chip">
                        <input
                          type="time"
                          aria-label={`Hora ${i + 1}`}
                          value={t}
                          onChange={(e) => setForm({ ...form, times: form.times.map((x, j) => (j === i ? e.target.value : x)) })}
                        />
                        {form.times.length > 1 && (
                          <button
                            type="button"
                            className="icon-btn"
                            aria-label={`Quitar hora ${i + 1}`}
                            onClick={() => setForm({ ...form, times: form.times.filter((_, j) => j !== i) })}
                          >
                            <Trash2 size={14} aria-hidden="true" />
                          </button>
                        )}
                      </span>
                    ))}
                    {form.times.length < 24 && (
                      <button type="button" className="btn btn-sm" onClick={() => setForm({ ...form, times: [...form.times, "12:00"] })}>
                        <Plus size={14} aria-hidden="true" /> Añadir hora
                      </button>
                    )}
                  </div>
                </div>
                <div className="field">
                  <span className="field-label">Días (ninguno marcado = todos)</span>
                  <div className="seg day-picker" role="group" aria-label="Días">
                    {DAYS.map((d, i) => (
                      <button
                        key={d}
                        type="button"
                        className="seg-btn"
                        aria-label={DAY_NAMES[i]}
                        aria-pressed={form.days.includes(i + 1)}
                        onClick={() => toggleDay(i + 1)}
                      >
                        {d}
                      </button>
                    ))}
                  </div>
                </div>
              </>
            ) : (
              <div className="field">
                <label htmlFor="tk-every">Cada</label>
                <div className="files-bar">
                  <input
                    id="tk-every"
                    type="text"
                    inputMode="numeric"
                    value={form.every}
                    onChange={(e) => setForm({ ...form, every: e.target.value })}
                  />
                  <select aria-label="Unidad" value={form.unit} onChange={(e) => setForm({ ...form, unit: e.target.value as "min" | "h" })}>
                    <option value="min">minutos</option>
                    <option value="h">horas</option>
                  </select>
                </div>
                <p className="field-help">La primera vez es al pasar ese tiempo desde que guardas.</p>
              </div>
            )}
            <label className="switch-row">
              <input type="checkbox" checked={form.enabled} onChange={(e) => setForm({ ...form, enabled: e.target.checked })} />
              <span>
                <strong>Activa</strong>
              </span>
            </label>
            {formError && (
              <p className="form-error" role="alert">
                {formError}
              </p>
            )}
          </div>
        </Dialog>
      )}

      {del && (
        <Dialog
          title="Borrar tarea"
          lead={`«${describeAction(del.action)}» (${describeSchedule(del.schedule).toLowerCase()}) deja de ejecutarse.`}
          onClose={() => setDel(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setDel(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  const t = del;
                  setDel(null);
                  act(() => sup.deleteTask(id, t.id), "Tarea borrada.");
                }}
              >
                Borrar tarea
              </button>
            </>
          }
        />
      )}
    </div>
  );
}
