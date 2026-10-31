// Jugadores: acciones con alguien conectado (mensaje privado, modo de juego, teletransporte) y el
// historial que ve MercHosting (última conexión, sesiones, tiempo jugado) con notas privadas.

import { History, MessageSquare, NotebookPen, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { GameMode, InstanceId, PlayerCommand, PlayerRecord } from "../api/types";
import { Dialog } from "./Dialog";

const MODES: [GameMode, string][] = [
  ["survival", "Supervivencia"],
  ["creative", "Creativo"],
  ["adventure", "Aventura"],
  ["spectator", "Espectador"],
];

export function PlayerActionsDialog({
  sup,
  id,
  name,
  online,
  onClose,
  onDone,
}: {
  sup: Supervisor;
  id: InstanceId;
  name: string;
  online: string[];
  onClose: () => void;
  onDone: (msg: string) => void;
}) {
  const [kind, setKind] = useState<"message" | "mode" | "tp">("message");
  const [text, setText] = useState("");
  const [mode, setMode] = useState<GameMode>("survival");
  const others = online.filter((p) => p !== name);
  const [tpTo, setTpTo] = useState<"player" | "coords">(others.length ? "player" : "coords");
  const [target, setTarget] = useState(others[0] ?? "");
  const [xyz, setXyz] = useState({ x: "0", y: "64", z: "0" });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const num = (v: string) => (/^-?\d{1,8}$/.test(v.trim()) ? Number(v) : null);
  const coordsOk = num(xyz.x) !== null && num(xyz.y) !== null && num(xyz.z) !== null;
  const ready = kind === "message" ? text.trim().length > 0 : kind === "mode" ? true : tpTo === "player" ? !!target : coordsOk;

  async function send() {
    let cmd: PlayerCommand;
    let msg: string;
    if (kind === "message") {
      cmd = { type: "message", text: text.trim() };
      msg = `Mensaje enviado a ${name}.`;
    } else if (kind === "mode") {
      cmd = { type: "game_mode", mode };
      msg = `${name} ahora está en modo ${MODES.find((m) => m[0] === mode)![1].toLowerCase()}.`;
    } else if (tpTo === "player") {
      cmd = { type: "teleport_to", target };
      msg = `${name} fue llevado junto a ${target}.`;
    } else {
      cmd = { type: "teleport_coords", x: num(xyz.x)!, y: num(xyz.y)!, z: num(xyz.z)! };
      msg = `${name} fue llevado a ${xyz.x} ${xyz.y} ${xyz.z}.`;
    }
    setBusy(true);
    setError(null);
    try {
      await sup.playerCommand(id, name, cmd);
      onDone(msg);
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  }

  return (
    <Dialog
      title={`Acciones con ${name}`}
      lead="Se hacen con los comandos oficiales de Minecraft y quedan en la Actividad."
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            Cancelar
          </button>
          <button type="button" className="btn btn-go" disabled={!ready || busy} onClick={send}>
            {kind === "message" ? "Enviar mensaje" : kind === "mode" ? "Cambiar modo" : "Teletransportar"}
          </button>
        </>
      }
    >
      <div className="seg" role="group" aria-label="Acción">
        {(
          [
            ["message", "Mensaje"],
            ["mode", "Modo de juego"],
            ["tp", "Teletransportar"],
          ] as const
        ).map(([k, label]) => (
          <button key={k} type="button" aria-pressed={kind === k} onClick={() => setKind(k)}>
            {label}
          </button>
        ))}
      </div>
      {kind === "message" && (
        <div className="field">
          <label htmlFor="pl-msg">Mensaje privado (solo lo ve {name})</label>
          <input id="pl-msg" type="text" maxLength={200} value={text} onChange={(e) => setText(e.target.value)} />
        </div>
      )}
      {kind === "mode" && (
        <div className="field">
          <label htmlFor="pl-mode">Modo de juego</label>
          <select id="pl-mode" value={mode} onChange={(e) => setMode(e.target.value as GameMode)}>
            {MODES.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
          {mode === "creative" && <span className="field-help">En creativo puede conseguir cualquier objeto y volar.</span>}
        </div>
      )}
      {kind === "tp" && (
        <>
          <fieldset className="seg-choice">
            <legend className="sr-only">A dónde</legend>
            <label>
              <input type="radio" name="tp" checked={tpTo === "player"} disabled={!others.length} onChange={() => setTpTo("player")} />
              Junto a otro jugador {others.length ? "" : "(no hay nadie más conectado)"}
            </label>
            <label>
              <input type="radio" name="tp" checked={tpTo === "coords"} onChange={() => setTpTo("coords")} />A unas coordenadas
            </label>
          </fieldset>
          {tpTo === "player" ? (
            <div className="field">
              <label htmlFor="pl-target">Jugador</label>
              <select id="pl-target" value={target} onChange={(e) => setTarget(e.target.value)}>
                {others.map((p) => (
                  <option key={p}>{p}</option>
                ))}
              </select>
            </div>
          ) : (
            <div className="coords-row">
              {(["x", "y", "z"] as const).map((k) => (
                <div className="field" key={k}>
                  <label htmlFor={`pl-${k}`}>{k.toUpperCase()}</label>
                  <input
                    id={`pl-${k}`}
                    type="text"
                    inputMode="numeric"
                    value={xyz[k]}
                    onChange={(e) => setXyz((c) => ({ ...c, [k]: e.target.value }))}
                  />
                </div>
              ))}
            </div>
          )}
          {tpTo === "coords" && !coordsOk && <span className="field-help">Escribe números enteros.</span>}
        </>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}

function ago(unix: number, now: number): string {
  const s = Math.max(0, now - unix);
  if (s < 60) return "hace un momento";
  if (s < 3600) return `hace ${Math.floor(s / 60)} min`;
  if (s < 86400) return `hace ${Math.floor(s / 3600)} h`;
  return new Date(unix * 1000).toLocaleDateString();
}

export function played(secs: number): string {
  if (secs < 60) return "menos de 1 min";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h > 0 ? `${h} h ${m} min` : `${m} min`;
}

export function PlayerHistoryCard({ sup, id, pollKey }: { sup: Supervisor; id: InstanceId; pollKey: unknown }) {
  const [list, setList] = useState<PlayerRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<{ name: string; note: string } | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  const load = useCallback(() => {
    sup.playerHistory(id).then(setList, (e) => setError(errorMessage(e)));
  }, [sup, id]);
  useEffect(() => {
    load();
  }, [load, pollKey]);

  const now = Math.floor(Date.now() / 1000);
  const total = (r: PlayerRecord) => r.total_secs + (r.online_since_unix ? Math.max(0, now - r.online_since_unix) : 0);

  return (
    <section className="card players-card history-card" aria-label="Historial de jugadores">
      <header className="players-head">
        <span className="players-icon">
          <History size={16} aria-hidden="true" />
        </span>
        <h2>Historial</h2>
        <span className="players-count">{list?.length ?? 0}</span>
      </header>
      <p className="field-help">
        Quién entró desde que MercHosting vigila este servidor (no lee registros antiguos). Los tiempos son aproximados. Las notas solo
        las ves tú.
      </p>
      {error && <p className="form-error">{error}</p>}
      {list?.length === 0 && <p className="field-help">Todavía no entró nadie.</p>}
      {list && list.length > 0 && (
        <table className="history-table">
          <thead>
            <tr>
              <th scope="col">Jugador</th>
              <th scope="col">Última conexión</th>
              <th scope="col">Veces</th>
              <th scope="col">Tiempo jugado</th>
              <th scope="col">Nota</th>
            </tr>
          </thead>
          <tbody>
            {list.map((r) => (
              <tr key={r.name}>
                <th scope="row">{r.name}</th>
                <td>{r.online_since_unix ? <span className="badge">Conectado</span> : r.sessions ? ago(r.last_seen_unix, now) : "—"}</td>
                <td>{r.sessions}</td>
                <td>{r.sessions ? played(total(r)) : "—"}</td>
                <td>
                  <button type="button" className="link-btn" onClick={() => setEditing({ name: r.name, note: r.note ?? "" })} aria-label={`Nota de ${r.name}`}>
                    {r.note ? r.note : <NotebookPen size={14} aria-hidden="true" />}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {list && list.length > 0 && (
        <button type="button" className="btn btn-sm btn-quiet" onClick={() => setConfirmClear(true)}>
          <Trash2 size={14} aria-hidden="true" /> Borrar historial y notas
        </button>
      )}

      {editing && (
        <Dialog
          title={`Nota sobre ${editing.name}`}
          lead="Privada: solo la ves tú (y los compañeros con permiso de jugadores)."
          onClose={() => setEditing(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setEditing(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                onClick={async () => {
                  try {
                    setList(await sup.setPlayerNote(id, editing.name, editing.note));
                    setEditing(null);
                  } catch (e) {
                    setError(errorMessage(e));
                  }
                }}
              >
                <MessageSquare size={14} aria-hidden="true" /> Guardar nota
              </button>
            </>
          }
        >
          <div className="field">
            <label htmlFor="pl-note">Nota</label>
            <textarea id="pl-note" maxLength={500} rows={4} value={editing.note} onChange={(e) => setEditing({ ...editing, note: e.target.value })} />
          </div>
        </Dialog>
      )}

      {confirmClear && (
        <Dialog
          title="Borrar historial y notas"
          lead="Se borra quién entró, cuándo y las notas de este servidor. No afecta a la lista blanca, operadores ni baneos."
          onClose={() => setConfirmClear(false)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirmClear(false)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={async () => {
                  setConfirmClear(false);
                  setList(await sup.clearPlayerHistory(id));
                }}
              >
                Borrar
              </button>
            </>
          }
        >
          <p className="field-help">No se puede deshacer.</p>
        </Dialog>
      )}
    </section>
  );
}
