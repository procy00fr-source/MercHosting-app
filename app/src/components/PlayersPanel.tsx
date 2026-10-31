// Pestaña Jugadores: conectados, lista blanca, operadores y baneados.
// Los cambios se hacen con los comandos oficiales de Minecraft (servidor encendido).

import { Ban, Crown, ShieldCheck, ShieldOff, TriangleAlert, UserMinus, UserPlus, Users } from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId, PlayerAction, PlayerLists } from "../api/types";
import { Dialog } from "./Dialog";
import { PlayerActionsDialog, PlayerHistoryCard } from "./PlayerExtras";

const POLL_MS = 3000;
const NAME_RE = /^[A-Za-z0-9_]{1,16}$/;

const CONFIRM_TEXT: Partial<Record<PlayerAction, { title: string; lead: string; button: string; reason: boolean }>> = {
  op: {
    title: "Dar permisos de operador",
    lead: "Un operador puede usar todos los comandos del servidor: cambiar el modo de juego, teletransportar, banear y detener el servidor. Dáselo solo a personas de confianza.",
    button: "Dar OP",
    reason: false,
  },
  kick: {
    title: "Expulsar jugador",
    lead: "Se desconecta del servidor ahora. Puede volver a entrar cuando quiera.",
    button: "Expulsar",
    reason: true,
  },
  ban: {
    title: "Banear jugador",
    lead: "No podrá volver a entrar hasta que lo perdones. Si está conectado, se le expulsa.",
    button: "Banear",
    reason: true,
  },
};

interface Props {
  sup: Supervisor;
  id: InstanceId;
}

function Section({ title, icon, count, children }: { title: string; icon: ReactNode; count: number; children: ReactNode }) {
  return (
    <section className="card players-card">
      <header className="players-head">
        <span className="players-icon">{icon}</span>
        <h2>{title}</h2>
        <span className="players-count">{count}</span>
      </header>
      {children}
    </section>
  );
}

export function PlayersPanel({ sup, id }: Props) {
  const [data, setData] = useState<{ lists: PlayerLists; online: string[]; can_act: boolean } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<{ action: PlayerAction; name: string } | null>(null);
  const [reason, setReason] = useState("");
  const [done, setDone] = useState<string | null>(null);
  const [actionsFor, setActionsFor] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setData(await sup.players(id));
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  useEffect(() => {
    load();
    const t = setInterval(load, POLL_MS);
    return () => clearInterval(t);
  }, [load]);

  async function act(action: PlayerAction, who: string, why: string | null = null) {
    setBusy(true);
    setError(null);
    try {
      await sup.playerAction(id, action, who, why);
      setDone(`Enviado al servidor: ${who}`);
      if (action === "whitelist_add" || action === "op" || action === "ban") setName("");
      // Minecraft escribe sus archivos un instante después del comando.
      setTimeout(load, 700);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  function request(action: PlayerAction, who: string) {
    if (CONFIRM_TEXT[action]) {
      setReason("");
      setConfirm({ action, name: who });
    } else {
      act(action, who);
    }
  }

  const canAct = !!data?.can_act && !busy;
  const nameOk = NAME_RE.test(name.trim());
  const c = confirm ? CONFIRM_TEXT[confirm.action]! : null;

  return (
    <div className="admin-panel" role="tabpanel" aria-label="Jugadores">
      {data && !data.can_act && (
        <div className="wiz-alert">
          <TriangleAlert size={18} aria-hidden="true" />
          <p>
            Enciende el servidor para cambiar estas listas. MercHosting usa los comandos oficiales de Minecraft, así el propio
            juego comprueba los nombres y guarda sus archivos.
          </p>
        </div>
      )}

      <form
        className="player-add"
        onSubmit={(e) => {
          e.preventDefault();
          if (nameOk && canAct) act("whitelist_add", name.trim());
        }}
      >
        <label htmlFor="player-name" className="sr-only">
          Nombre del jugador
        </label>
        <input
          id="player-name"
          type="text"
          placeholder="Nombre exacto del jugador"
          maxLength={16}
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button type="submit" className="btn btn-go" disabled={!nameOk || !canAct}>
          <UserPlus size={16} aria-hidden="true" /> Añadir a la lista blanca
        </button>
        <button type="button" className="btn" disabled={!nameOk || !canAct} onClick={() => request("op", name.trim())}>
          <Crown size={16} aria-hidden="true" /> Dar OP
        </button>
        <button type="button" className="btn btn-danger-ghost" disabled={!nameOk || !canAct} onClick={() => request("ban", name.trim())}>
          <Ban size={16} aria-hidden="true" /> Banear
        </button>
      </form>
      {name && !nameOk && <p className="form-error">Solo letras, números y _ (máximo 16).</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {done && !error && (
        <p className="field-help" role="status">
          {done}
        </p>
      )}
      {data?.lists.warnings.map((w) => (
        <p key={w} className="field-help warn-text">
          {w}
        </p>
      ))}

      {!data ? (
        <p className="wiz-loading" role="status">
          Cargando jugadores…
        </p>
      ) : (
        <div className="players-grid">
          <Section title="Conectados" icon={<Users size={16} aria-hidden="true" />} count={data.online.length}>
            {data.online.length === 0 ? (
              <p className="field-help">Nadie conectado ahora.</p>
            ) : (
              <ul className="player-list">
                {data.online.map((p) => (
                  <li key={p}>
                    <span className="player-name">{p}</span>
                    <span className="player-actions">
                      <button type="button" className="btn btn-sm" disabled={!canAct} onClick={() => setActionsFor(p)} aria-label={`Más acciones con ${p}`}>
                        Más…
                      </button>
                      <button type="button" className="btn btn-sm" disabled={!canAct} onClick={() => request("kick", p)}>
                        Expulsar
                      </button>
                      <button type="button" className="btn btn-sm btn-danger-ghost" disabled={!canAct} onClick={() => request("ban", p)}>
                        Banear
                      </button>
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section title="Lista blanca" icon={<ShieldCheck size={16} aria-hidden="true" />} count={data.lists.whitelist.length}>
            {data.lists.whitelist.length === 0 ? (
              <p className="field-help">Vacía. Solo se usa si activas «Lista blanca» en Ajustes.</p>
            ) : (
              <ul className="player-list">
                {data.lists.whitelist.map((p) => (
                  <li key={p.name}>
                    <span className="player-name" translate="no">{p.name}</span>
                    <button
                      type="button"
                      className="btn btn-sm btn-quiet"
                      disabled={!canAct}
                      aria-label={`Quitar a ${p.name} de la lista blanca`}
                      onClick={() => request("whitelist_remove", p.name)}
                    >
                      <UserMinus size={14} aria-hidden="true" /> Quitar
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section title="Operadores" icon={<Crown size={16} aria-hidden="true" />} count={data.lists.ops.length}>
            {data.lists.ops.length === 0 ? (
              <p className="field-help">Nadie tiene permisos de operador.</p>
            ) : (
              <ul className="player-list">
                {data.lists.ops.map((p) => (
                  <li key={p.name}>
                    <span className="player-name">
                      <span translate="no">{p.name}</span>
                      {p.level !== null && <span className="player-meta">nivel {p.level}</span>}
                    </span>
                    <button
                      type="button"
                      className="btn btn-sm btn-quiet"
                      disabled={!canAct}
                      aria-label={`Quitar OP a ${p.name}`}
                      onClick={() => request("deop", p.name)}
                    >
                      <ShieldOff size={14} aria-hidden="true" /> Quitar OP
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section title="Baneados" icon={<Ban size={16} aria-hidden="true" />} count={data.lists.banned.length}>
            {data.lists.banned.length === 0 ? (
              <p className="field-help">Nadie baneado.</p>
            ) : (
              <ul className="player-list">
                {data.lists.banned.map((p) => (
                  <li key={p.name}>
                    <span className="player-name">
                      <span translate="no">{p.name}</span>
                      {p.reason && <span className="player-meta">{p.reason}</span>}
                    </span>
                    <button
                      type="button"
                      className="btn btn-sm btn-quiet"
                      disabled={!canAct}
                      aria-label={`Perdonar a ${p.name}`}
                      onClick={() => request("pardon", p.name)}
                    >
                      Perdonar
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {data.lists.banned_ips.length > 0 && (
              <p className="field-help">IPs baneadas: {data.lists.banned_ips.length}</p>
            )}
          </Section>
          <PlayerHistoryCard sup={sup} id={id} pollKey={data.online.join(",")} />
        </div>
      )}

      {actionsFor && data && (
        <PlayerActionsDialog
          sup={sup}
          id={id}
          name={actionsFor}
          online={data.online}
          onClose={() => setActionsFor(null)}
          onDone={(m) => {
            setActionsFor(null);
            setDone(m);
          }}
        />
      )}

      {confirm && c && (
        <Dialog
          title={`${c.title}: ${confirm.name}`}
          lead={c.lead}
          onClose={() => setConfirm(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirm(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className={confirm.action === "op" ? "btn btn-go" : "btn btn-stop"}
                onClick={() => {
                  const x = confirm;
                  setConfirm(null);
                  act(x.action, x.name, c.reason ? reason.trim() || null : null);
                }}
              >
                {c.button}
              </button>
            </>
          }
        >
          {c.reason && (
            <div className="field">
              <label htmlFor="player-reason">Motivo (opcional, lo verá el jugador)</label>
              <input id="player-reason" type="text" maxLength={120} value={reason} onChange={(e) => setReason(e.target.value)} />
            </div>
          )}
        </Dialog>
      )}
    </div>
  );
}
