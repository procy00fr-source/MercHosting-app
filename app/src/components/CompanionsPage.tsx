// Compañeros: compartir la administración de servidores con amigos que también usan MercHosting
// en su PC (sin cuentas). Tu PC decide cada acción con los permisos que elijas aquí.

import { Check, Copy, Handshake, KeyRound, LogIn, Pause, Play, RefreshCw, ShieldAlert, Trash2, UserMinus, UserPlus } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { CompanionInfo, CompanionsInfo, InstanceId, InstanceSummary, NewInvite, Perm, RemoteHost } from "../api/types";
import { FILES_WARNING, PERMS, PERM_LABEL, ROLE_HELP, ROLE_LABEL, ROLE_PERMS, roleOf, type RoleName } from "../companions";
import { Dialog } from "./Dialog";
import { uiLocale } from "../i18n/lang";

const POLL_MS = 4000;

function when(ms: number | null) {
  if (!ms) return "nunca";
  return new Date(ms).toLocaleString(uiLocale(), { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

/** Permisos nuevos que piden advertencia (archivos reales) o confirmación reforzada (alto riesgo). */
export function risky(perms: Perm[], before: Perm[]) {
  const added = PERMS.filter((p) => perms.includes(p.perm) && !before.includes(p.perm));
  return { files: added.filter((p) => p.files), high: added.filter((p) => p.high) };
}

export function PermsPicker({
  value,
  onChange,
  before = [],
  ack,
  onAck,
}: {
  value: Perm[];
  onChange: (p: Perm[]) => void;
  before?: Perm[];
  ack: boolean;
  onAck: (v: boolean) => void;
}) {
  const role = roleOf(value);
  const r = risky(value, before);
  const groups = [...new Set(PERMS.map((p) => p.group))];
  function set(perm: Perm, on: boolean) {
    // «Ver el estado» va siempre: sin él no se puede mostrar nada.
    if (perm === "status") return;
    const next = on ? [...value, perm] : value.filter((p) => p !== perm);
    onChange([...new Set<Perm>(["status", ...next])]);
    onAck(false);
  }
  return (
    <div className="perms-picker">
      <fieldset className="role-row">
        <legend>Rol</legend>
        {(["observador", "moderador", "operador", "administrador", "personalizado"] as RoleName[]).map((name) => (
          <label key={name} className="role-option" data-on={role === name}>
            <input
              type="radio"
              name="rol"
              checked={role === name}
              onChange={() => {
                if (name !== "personalizado") {
                  onChange([...ROLE_PERMS[name]]);
                  onAck(false);
                }
              }}
              disabled={name === "personalizado" && role !== "personalizado"}
            />
            <span>
              <strong>{ROLE_LABEL[name]}</strong>
              <span className="field-help">{ROLE_HELP[name]}</span>
            </span>
          </label>
        ))}
      </fieldset>
      <div className="perm-groups">
        {groups.map((g) => (
          <fieldset key={g} className="perm-group">
            <legend>{g}</legend>
            {PERMS.filter((p) => p.group === g).map((p) => (
              <label key={p.perm} className="check">
                <input
                  type="checkbox"
                  checked={value.includes(p.perm)}
                  disabled={p.perm === "status"}
                  onChange={(e) => set(p.perm, e.target.checked)}
                />
                <span>
                  {p.label}
                  {p.high && <span className="badge badge-warn perm-risk">Alto riesgo</span>}
                </span>
              </label>
            ))}
          </fieldset>
        ))}
      </div>
      {(r.files.length > 0 || r.high.length > 0) && (
        <div className="notice notice-warn perm-warning" role="alert">
          <ShieldAlert size={18} aria-hidden="true" className="notice-icon" />
          <div>
            {r.files.length > 0 && <p>{FILES_WARNING}</p>}
            <p>Vas a conceder:</p>
            <ul>
              {[...r.files, ...r.high.filter((h) => !r.files.includes(h))].map((p) => (
                <li key={p.perm}>
                  <strong>{p.label}</strong>
                  {p.perm === "content" &&
                    ": podrá poner archivos JAR que Java ejecuta en tu PC con tu usuario de Windows. Un mod malicioso podría dañar tu PC; MercHosting no puede aislarlo."}
                  {p.perm === "console_command" &&
                    ": podrá enviar cualquier comando de Minecraft (incluidos /op y /stop). No puede ejecutar programas de Windows."}
                  {p.perm === "files_edit" && ": podrá cambiar y subir archivos al servidor (se guarda una copia antes de cada cambio)."}
                  {p.perm === "files_delete" && ": podrá mandar archivos a la papelera de MercHosting."}
                  {p.perm === "backups_restore" && ": podrá volver el mundo a una copia anterior (se guarda una copia del estado actual antes)."}
                  {p.perm === "op" && ": podrá dar poderes de operador dentro del juego."}
                  {p.perm === "files_read" && ": podrá leer y descargar a su PC los archivos del servidor (configuraciones y registros incluidos)."}
                </li>
              ))}
            </ul>
            <label className="check">
              <input type="checkbox" checked={ack} onChange={(e) => onAck(e.target.checked)} />
              <span>Entiendo los riesgos y confío en esta persona</span>
            </label>
          </div>
        </div>
      )}
    </div>
  );
}

function InviteDialog({
  sup,
  servers,
  onClose,
  onCreated,
}: {
  sup: Supervisor;
  servers: InstanceSummary[];
  onClose: () => void;
  onCreated: () => void;
}) {
  const [server, setServer] = useState<InstanceId>(servers[0]?.instance.id ?? "");
  const [perms, setPerms] = useState<Perm[]>([...ROLE_PERMS.observador]);
  const [minutes, setMinutes] = useState(60);
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [made, setMade] = useState<NewInvite | null>(null);
  const [copied, setCopied] = useState(false);
  const r = risky(perms, []);
  const needsAck = r.files.length > 0 || r.high.length > 0;

  async function create() {
    setBusy(true);
    setError(null);
    try {
      setMade(await sup.createInvite(server, perms, minutes));
      onCreated();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  if (made)
    return (
      <Dialog
        title="Código de invitación"
        lead="Envíaselo a tu compañero por un chat privado. Solo se muestra ahora, sirve una sola vez y caduca solo."
        onClose={onClose}
        footer={
          <button type="button" className="btn btn-go" onClick={onClose}>
            Listo
          </button>
        }
      >
        <div className="invite-code">
          <code aria-label="Código de invitación">{made.code}</code>
          <button
            type="button"
            className="btn btn-sm"
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(made.code);
                setCopied(true);
              } catch {
                setError("No se pudo copiar. Selecciona el código y cópialo a mano.");
              }
            }}
          >
            {copied ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />} {copied ? "Copiado" : "Copiar"}
          </button>
        </div>
        <ul className="plain-list">
          <li>
            Servidor: <strong translate="no">{made.invite.server_name}</strong> · rol {ROLE_LABEL[roleOf(made.invite.perms)]}.
          </li>
          <li>Caduca: {when(made.invite.expires_at_unix_ms)}.</li>
          <li>Tu compañero lo pega en su MercHosting, en Compañeros › «Administrar servidores de un amigo».</li>
          <li>Cuando se una, aquí verás su nombre y la huella de su PC. Puedes quitarle el acceso cuando quieras.</li>
        </ul>
        {error && <p className="form-error">{error}</p>}
      </Dialog>
    );

  return (
    <Dialog
      wide
      title="Invitar a un compañero"
      lead="Tu compañero podrá administrar ese servidor desde MercHosting en su PC, solo con los permisos que marques. Tu PC revisa cada acción."
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            Cancelar
          </button>
          <button type="button" className="btn btn-go" disabled={busy || !server || (needsAck && !ack)} onClick={create}>
            {busy ? "Creando…" : "Crear código"}
          </button>
        </>
      }
    >
      <label className="field">
        <span className="field-title">Servidor</span>
        <select value={server} onChange={(e) => setServer(e.target.value)}>
          {servers.map((s) => (
            <option key={s.instance.id} value={s.instance.id} translate="no">
              {s.instance.name}
            </option>
          ))}
        </select>
      </label>
      <PermsPicker value={perms} onChange={setPerms} ack={ack} onAck={setAck} />
      <label className="field">
        <span className="field-title">El código caduca en</span>
        <select value={minutes} onChange={(e) => setMinutes(Number(e.target.value))}>
          <option value={30}>30 minutos</option>
          <option value={60}>1 hora</option>
          <option value={360}>6 horas</option>
          <option value={1440}>24 horas</option>
        </select>
      </label>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}

function EditDialog({
  sup,
  companion,
  server,
  onClose,
}: {
  sup: Supervisor;
  companion: CompanionInfo;
  server: { id: InstanceId; name: string; perms: Perm[] };
  onClose: () => void;
}) {
  const [perms, setPerms] = useState<Perm[]>(server.perms);
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const r = risky(perms, server.perms);
  const needsAck = r.files.length > 0 || r.high.length > 0;
  return (
    <Dialog
      wide
      title={`Permisos de ${companion.name} en «${server.name}»`}
      lead="Los cambios valen desde ya: si estaba conectado, su app se vuelve a conectar con los permisos nuevos."
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            Cancelar
          </button>
          <button
            type="button"
            className="btn btn-go"
            disabled={busy || (needsAck && !ack)}
            onClick={async () => {
              setBusy(true);
              try {
                await sup.setCompanionPerms(companion.companion, server.id, perms);
                onClose();
              } catch (e) {
                setError(errorMessage(e));
                setBusy(false);
              }
            }}
          >
            Guardar
          </button>
        </>
      }
    >
      <PermsPicker value={perms} onChange={setPerms} before={server.perms} ack={ack} onAck={setAck} />
      {error && <p className="form-error">{error}</p>}
    </Dialog>
  );
}

interface Props {
  sup: Supervisor;
  servers: InstanceSummary[];
  /** Abre un servidor de un amigo. */
  onOpenRemote: (host: RemoteHost, id: InstanceId) => void;
}

export function CompanionsPage({ sup, servers, onOpenRemote }: Props) {
  const [info, setInfo] = useState<CompanionsInfo | null>(null);
  const [hosts, setHosts] = useState<RemoteHost[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [inviting, setInviting] = useState(false);
  const [editing, setEditing] = useState<{ c: CompanionInfo; s: { id: InstanceId; name: string; perms: Perm[] } } | null>(null);
  const [removing, setRemoving] = useState<CompanionInfo | null>(null);
  const [code, setCode] = useState("");
  const [myName, setMyName] = useState("");

  const load = useCallback(async () => {
    try {
      const [i, h] = await Promise.all([sup.companions(), sup.remoteHosts()]);
      setInfo(i);
      setHosts(h);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup]);

  useEffect(() => {
    load();
    const t = setInterval(load, POLL_MS);
    return () => clearInterval(t);
  }, [load]);

  async function run(key: string, fn: () => Promise<unknown>) {
    setBusy(key);
    setError(null);
    try {
      await fn();
      await load();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="admin-panel companions-page" aria-label="Compañeros">
      <header className="page-head">
        <div>
          <p className="page-eyebrow">Compartir</p>
          <h1 className="page-title">Compañeros</h1>
          <p className="field-help">
            Amigos que también usan MercHosting pueden ayudarte a administrar tus servidores desde su PC, con los permisos que tú
            elijas. No hacen falta cuentas ni pagar nada.
          </p>
        </div>
      </header>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <section className="card props-card" aria-label="Tu PC">
        <h2>
          <Handshake size={16} aria-hidden="true" /> Tus servidores: compartir con compañeros
        </h2>
        <ul className="plain-list">
          <li>
            La conexión va directa entre las dos PCs y cifrada de punta a punta. Si el router no lo permite, pasa por relés públicos
            gratuitos de n0 (los creadores de iroh), que solo ven datos cifrados y no tienen garantía de funcionamiento.
          </li>
          <li>Es un canal aparte: no usa el túnel de Minecraft ni Playit.</li>
          <li>Tu PC tiene que estar encendida, conectada a Internet y con MercHosting en marcha (abierto o en segundo plano).</li>
          <li>Tu compañero nunca entra en Windows: solo puede hacer, en el servidor que le des, lo que marques.</li>
        </ul>
        {info === null ? (
          <p className="wiz-loading">Cargando…</p>
        ) : (
          <>
            <div className="files-bar">
              {info.enabled ? (
                <span className="ok-text">
                  <Check size={14} aria-hidden="true" /> Compartir está activado
                </span>
              ) : (
                <span className="field-help">Compartir está desactivado: nadie puede conectarse a tu PC.</span>
              )}
              <button
                type="button"
                className={info.enabled ? "btn btn-sm btn-quiet" : "btn btn-go"}
                disabled={busy !== null}
                onClick={() => run("enable", () => sup.setCompanionsEnabled(!info.enabled))}
              >
                {info.enabled ? <Pause size={14} aria-hidden="true" /> : <Play size={14} aria-hidden="true" />}{" "}
                {info.enabled ? "Pausar el acceso de todos" : "Permitir compañeros"}
              </button>
              {info.enabled && (
                <button
                  type="button"
                  className="btn btn-sm btn-go"
                  disabled={busy !== null || servers.length === 0}
                  onClick={() => setInviting(true)}
                >
                  <UserPlus size={14} aria-hidden="true" /> Invitar a un compañero
                </button>
              )}
            </div>
            {info.fingerprint && (
              <p className="field-help">
                <KeyRound size={13} aria-hidden="true" /> Huella de esta PC: <code>{info.fingerprint}</code>
              </p>
            )}
            {info.error && <p className="warn-text">{info.error}</p>}

            {info.invites.length > 0 && (
              <>
                <h3 className="sub-title">Invitaciones sin usar</h3>
                <ul className="row-list" aria-label="Invitaciones sin usar">
                  {info.invites.map((i) => (
                    <li key={i.invite_id}>
                      <span>
                        <strong translate="no">{i.server_name}</strong> · {ROLE_LABEL[roleOf(i.perms)]} · caduca {when(i.expires_at_unix_ms)}
                      </span>
                      <button
                        type="button"
                        className="btn btn-sm btn-quiet"
                        disabled={busy !== null}
                        onClick={() => run(`inv-${i.invite_id}`, () => sup.cancelInvite(i.invite_id))}
                      >
                        Cancelar
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}

            <h3 className="sub-title">Compañeros</h3>
            {info.companions.length === 0 ? (
              <p className="field-help">Todavía nadie. Crea un código de invitación y envíaselo a tu compañero.</p>
            ) : (
              <ul className="companion-list" aria-label="Compañeros">
                {info.companions.map((c) => (
                  <li key={c.companion} className="companion">
                    <div className="companion-head">
                      <span className="layer-dot" data-on={c.connected} aria-hidden="true" />
                      <strong>{c.name}</strong>
                      <span className="field-help">
                        PC {c.fingerprint} · {c.connected ? "conectado ahora" : `visto por última vez: ${when(c.last_seen_unix_ms)}`}
                      </span>
                      <button
                        type="button"
                        className="btn btn-sm btn-quiet"
                        disabled={busy !== null}
                        onClick={() => setRemoving(c)}
                        aria-label={`Quitar a ${c.name}`}
                      >
                        <UserMinus size={14} aria-hidden="true" /> Quitar
                      </button>
                    </div>
                    {c.servers.length === 0 ? (
                      <p className="field-help">Sin acceso a ningún servidor.</p>
                    ) : (
                      <ul className="row-list">
                        {c.servers.map((s) => (
                          <li key={s.id}>
                            <span>
                              <strong translate="no">{s.name}</strong> · {ROLE_LABEL[roleOf(s.perms)]}
                              <span className="field-help"> ({s.perms.map((p) => PERM_LABEL[p].toLowerCase()).join(", ")})</span>
                            </span>
                            <button type="button" className="btn btn-sm" onClick={() => setEditing({ c, s })}>
                              Cambiar permisos
                            </button>
                            <button
                              type="button"
                              className="btn btn-sm btn-quiet"
                              disabled={busy !== null}
                              onClick={() => run(`rm-${c.companion}-${s.id}`, () => sup.setCompanionPerms(c.companion, s.id, []))}
                            >
                              Quitar acceso
                            </button>
                          </li>
                        ))}
                      </ul>
                    )}
                  </li>
                ))}
              </ul>
            )}
            <p className="field-help">Todo lo que hagan tus compañeros queda en la Actividad de cada servidor, con su nombre.</p>
          </>
        )}
      </section>

      <section className="card props-card" aria-label="Servidores de amigos">
        <h2>
          <LogIn size={16} aria-hidden="true" /> Administrar servidores de un amigo
        </h2>
        <form
          className="join-form"
          onSubmit={(e) => {
            e.preventDefault();
            run("join", async () => {
              await sup.joinWithCode(code.trim(), myName.trim());
              setCode("");
            });
          }}
        >
          <label className="field">
            <span className="field-title">Código que te dio tu amigo</span>
            <input type="text" value={code} onChange={(e) => setCode(e.target.value)} placeholder="MH1-…" spellCheck={false} />
          </label>
          <label className="field">
            <span className="field-title">Tu nombre (así te verá)</span>
            <input type="text" value={myName} onChange={(e) => setMyName(e.target.value)} maxLength={32} />
          </label>
          <button type="submit" className="btn btn-go" disabled={busy !== null || !code.trim() || !myName.trim()}>
            {busy === "join" ? "Conectando…" : "Unirme"}
          </button>
        </form>
        {hosts && hosts.length > 0 && (
          <ul className="companion-list" aria-label="PCs de amigos">
            {hosts.map((h) => (
              <li key={h.host} className="companion">
                <div className="companion-head">
                  <strong>{h.name || "PC de tu amigo"}</strong>
                  <span className="field-help">
                    PC {h.fingerprint}
                    {h.last_ok_unix_ms ? ` · respondió: ${when(h.last_ok_unix_ms)}` : ""}
                  </span>
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={busy !== null}
                    onClick={() => run(`ref-${h.host}`, () => sup.refreshHost(h.host))}
                  >
                    <RefreshCw size={14} aria-hidden="true" /> Actualizar
                  </button>
                  <button
                    type="button"
                    className="btn btn-sm btn-quiet"
                    disabled={busy !== null}
                    onClick={() => run(`forget-${h.host}`, () => sup.forgetHost(h.host))}
                    aria-label={`Olvidar ${h.name}`}
                  >
                    <Trash2 size={14} aria-hidden="true" /> Olvidar
                  </button>
                </div>
                {h.last_error && <p className="warn-text">{h.last_error}</p>}
                {h.servers.length === 0 ? (
                  <p className="field-help">No tienes acceso a ningún servidor de esta PC.</p>
                ) : (
                  <ul className="row-list">
                    {h.servers.map((s) => (
                      <li key={s.id}>
                        <span>
                          <strong translate="no">{s.name}</strong> · {ROLE_LABEL[roleOf(s.perms)]}
                        </span>
                        <button type="button" className="btn btn-sm btn-go" onClick={() => onOpenRemote(h, s.id)}>
                          Abrir
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      {inviting && <InviteDialog sup={sup} servers={servers} onClose={() => setInviting(false)} onCreated={load} />}
      {editing && (
        <EditDialog
          sup={sup}
          companion={editing.c}
          server={editing.s}
          onClose={() => {
            setEditing(null);
            load();
          }}
        />
      )}
      {removing && (
        <Dialog
          title={`Quitar a ${removing.name}`}
          lead="Pierde el acceso a todos tus servidores al momento; si está conectado, se corta su conexión. Para volver a darle acceso necesitará un código nuevo."
          onClose={() => setRemoving(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setRemoving(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  const c = removing;
                  setRemoving(null);
                  run(`del-${c.companion}`, () => sup.removeCompanion(c.companion));
                }}
              >
                Quitar
              </button>
            </>
          }
        />
      )}
    </div>
  );
}
