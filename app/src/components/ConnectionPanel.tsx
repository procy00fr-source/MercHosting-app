// Pestaña Conexión: cómo entran los amigos. En la misma red, con la IP local; por Internet,
// con Playit.gg (servicio externo y gratuito, con la cuenta de la propia persona). La dirección
// pública solo se puede copiar después de probarla de verdad.

import { Activity, CheckCircle2, Copy, ExternalLink, Globe, House, Link2, Mic, Power, RefreshCw, RotateCw, TriangleAlert, Unlink } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { AddressTest, ConnLayer, ConnectionInfo, InstanceId, LayerState, PlayitInfo, VoiceInfo } from "../api/types";
import { Dialog } from "./Dialog";
import { uiLocale } from "../i18n/lang";

const CLAIM_TEXT: Record<string, string> = {
  waiting_for_visit: "Abre el enlace y entra con tu cuenta de playit.gg (o créala, es gratis).",
  waiting_for_approval: "Ahora aprueba «MercHosting» en la página de playit.gg.",
  approved: "Aprobado. Terminando…",
  rejected: "Se rechazó la vinculación.",
  expired: "El enlace caducó.",
};

interface Props {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  online: boolean;
  /** El servidor está apagado (se puede cambiar su puerto). */
  stopped: boolean;
  /** Servidor de un amigo: solo su estado y su chat de voz (Playit es de la PC del dueño). */
  remote?: boolean;
}

function portOf(destination: string): number | null {
  const n = Number(destination.split(":").pop());
  return Number.isInteger(n) && n > 0 && n < 65536 ? n : null;
}

const LAYER_LABEL: Record<string, string> = {
  server: "Servidor (Java)",
  agent: "Agente de Playit",
  tunnel: "Túnel de Minecraft",
  external: "Prueba desde Internet",
  voice: "Chat de voz",
};

const STATE_LABEL: Record<LayerState, string> = {
  operativo: "Operativo",
  degradado: "Degradado",
  caido: "Caído",
  desconocido: "Desconocido",
};

function hour(ms: number) {
  return new Date(ms).toLocaleTimeString(uiLocale(), { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function LayersCard({ layers }: { layers: ConnLayer[] }) {
  if (layers.length === 0) return null;
  return (
    <section className="card props-card" aria-label="Estado de la conexión">
      <h2>
        <Activity size={16} aria-hidden="true" /> Estado ahora
      </h2>
      <ul className="layer-list">
        {layers.map((l) => (
          <li key={l.key} data-state={l.state} aria-label={`${LAYER_LABEL[l.key] ?? l.key}: ${STATE_LABEL[l.state]}`}>
            <span className="layer-dot" aria-hidden="true" />
            <strong>{LAYER_LABEL[l.key] ?? l.key}</strong>
            <span className="layer-state">{STATE_LABEL[l.state]}</span>
            <span className="field-help layer-detail">{l.detail}</span>
            <span className="field-help layer-time">comprobado a las {hour(l.checked_at_unix_ms)}</span>
          </li>
        ))}
      </ul>
      <p className="field-help">Cada capa se mide de verdad; si algo no se puede medir, aparece como «Desconocido».</p>
    </section>
  );
}

function hasPort(a: string) {
  return /:\d+$/.test(a.trim());
}

function TestLine({ t }: { t: AddressTest | undefined }) {
  if (!t) return <span className="badge badge-muted">Sin probar</span>;
  if (t.result)
    return (
      <span className="ok-text">
        <CheckCircle2 size={13} aria-hidden="true" /> Funciona: responde en {t.result.latency_ms} ms
        {t.result.version ? ` · ${t.result.version}` : ""} · {t.result.players_online}/{t.result.players_max} jugadores
      </span>
    );
  return (
    <span className="warn-text">
      <TriangleAlert size={13} aria-hidden="true" /> No responde: {t.error}
    </span>
  );
}

export function ConnectionPanel({ api, sup, id, online, stopped, remote = false }: Props) {
  const [conn, setConn] = useState<ConnectionInfo | null>(null);
  const [playit, setPlayit] = useState<PlayitInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [askUnlink, setAskUnlink] = useState(false);

  const load = useCallback(async () => {
    try {
      const [c, p] = await Promise.all([sup.connection(id), remote ? Promise.resolve(null) : sup.playitStatus()]);
      setConn(c);
      setPlayit(p);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id, remote]);

  useEffect(() => {
    load();
    const t = setInterval(load, 3000);
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

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(text);
      setTimeout(() => setCopied(null), 2000);
    } catch {
      setError("No se pudo copiar. Selecciona la dirección y cópiala a mano.");
    }
  }

  const testOf = (a: string) => conn?.tests.find((t) => t.address === a);
  const agent = playit?.agent ?? null;
  const phase = agent?.phase ?? null;
  const port = conn?.port ?? null;
  const voice: VoiceInfo | null = conn?.voice ?? null;
  const voiceTunnel = (t: { display_address: string }) => !!voice?.tunnels.some((v) => v.display_address === t.display_address);
  const otherTunnels = (agent?.tunnels ?? []).filter(
    (t) => !conn?.tunnels.some((c) => c.display_address === t.display_address) && !voiceTunnel(t),
  );
  const [voiceAsk, setVoiceAsk] = useState<{ title: string; lead: string; change: { voice_host?: string; port?: number } } | null>(null);
  // Túneles que van a un puerto que no usa ningún otro servidor: seguramente eran para este.
  const usedBy = (p: number) => conn?.ports_in_use.find((u) => u.port === p)?.server ?? null;
  const strayTunnels = otherTunnels.filter((t) => {
    const p = portOf(t.destination);
    return p !== null && p !== port && !t.is_disabled;
  });

  async function usePort(p: number) {
    const props = await sup.properties(id);
    await sup.setProperties(id, { "server-port": String(p) }, props.modified_unix_ms);
  }

  return (
    <div className="admin-panel connection-panel" role="tabpanel" aria-label="Conexión">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">Conexión</h2>
          <p className="field-help">
            Cómo entran tus amigos a este servidor{port ? ` (puerto ${port})` : ""}. Tu PC y el servidor tienen que estar encendidos.
          </p>
        </div>
        <button type="button" className="btn btn-sm" onClick={load}>
          <RefreshCw size={14} aria-hidden="true" /> Actualizar
        </button>
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      {conn && <LayersCard layers={conn.layers} />}

      {!remote && (
      <>
      <section className="card props-card" aria-label="En tu misma red">
        <h2>
          <House size={16} aria-hidden="true" /> En tu misma red (casa o LAN)
        </h2>
        {conn === null ? (
          <p className="wiz-loading">Buscando…</p>
        ) : conn.lan.length === 0 ? (
          <p className="field-help">No se encontró una dirección de red local en esta PC.</p>
        ) : (
          conn.lan.map((a) => (
            <div key={a} className="address-row">
              <code className="address">{a}</code>
              <button type="button" className="btn btn-sm" onClick={() => copy(a)} aria-label={`Copiar ${a}`}>
                <Copy size={14} aria-hidden="true" /> {copied === a ? "Copiada" : "Copiar"}
              </button>
              <button
                type="button"
                className="btn btn-sm"
                disabled={!online || busy !== null}
                onClick={() => run(`t-${a}`, () => sup.testAddress(id, a))}
                aria-label={`Probar ${a}`}
              >
                {busy === `t-${a}` ? "Probando…" : "Probar"}
              </button>
              <TestLine t={testOf(a)} />
            </div>
          ))
        )}
        <p className="field-help">Sirve para quien está conectado a tu mismo router. Por Internet, usa Playit.</p>
      </section>

      <section className="card props-card" aria-label="Amigos por Internet">
        <h2>
          <Globe size={16} aria-hidden="true" /> Amigos por Internet · Playit.gg
        </h2>
        {playit === null ? (
          <p className="wiz-loading">Consultando…</p>
        ) : !playit.supported ? (
          <p className="field-help">El agente oficial de Playit que usa MercHosting es para Windows.</p>
        ) : !playit.enabled ? (
          <>
            <p>
              Playit.gg es un <strong>servicio externo y gratuito</strong> que crea una dirección pública que llega a tu PC, sin
              abrir puertos del router (también funciona con CGNAT).
            </p>
            <ul className="plain-list">
              <li>Usarás tu propia cuenta de playit.gg; MercHosting no usa ninguna cuenta compartida.</li>
              <li>
                MercHosting descarga el agente oficial de Playit (versión {playit.agent_version}), comprueba su huella y lo deja
                funcionando en segundo plano, sin ventanas.
              </li>
              <li>Tus amigos solo podrán entrar mientras tu PC, MercHosting y el servidor estén encendidos.</li>
              <li>Si usas Simple Voice Chat, aquí mismo te guiamos con su túnel de voz.</li>
            </ul>
            <button type="button" className="btn btn-go" disabled={busy !== null} onClick={() => run("enable", () => sup.playitEnable())}>
              <Power size={15} aria-hidden="true" /> {busy === "enable" ? "Descargando y verificando el agente…" : "Activar Playit"}
            </button>
          </>
        ) : !playit.running ? (
          <>
            <p className="warn-text">
              <TriangleAlert size={14} aria-hidden="true" /> El agente de Playit no responde
              {playit.agent_error ? `: ${playit.agent_error}` : "."}
            </p>
            <button type="button" className="btn btn-sm" disabled={busy !== null} onClick={() => run("enable", () => sup.playitEnable())}>
              <RefreshCw size={14} aria-hidden="true" /> Reintentar
            </button>
          </>
        ) : phase === "waiting_for_secret" || phase === "invalid_secret" ? (
          <>
            {phase === "invalid_secret" && (
              <p className="warn-text">La vinculación anterior ya no es válida{agent?.message ? `: ${agent.message}` : ""}. Vuelve a vincular.</p>
            )}
            {playit.claim && !playit.claim.done ? (
              <div className="claim-box" aria-label="Vinculación">
                <p>
                  <strong>Paso 1.</strong> {CLAIM_TEXT[playit.claim.state] ?? ""}
                </p>
                <div className="address-row">
                  <code className="address">{playit.claim.url}</code>
                  <button type="button" className="btn btn-sm btn-go" onClick={() => api.openUrl(playit.claim!.url)}>
                    <ExternalLink size={14} aria-hidden="true" /> Abrir en el navegador
                  </button>
                </div>
                {playit.claim.error && <p className="warn-text">{playit.claim.error}</p>}
                <p className="field-help">Esta página se actualiza sola cuando apruebes.</p>
              </div>
            ) : (
              <>
                <p>El agente está listo. Ahora vincúlalo con tu cuenta de playit.gg.</p>
                <button type="button" className="btn btn-go" disabled={busy !== null} onClick={() => run("link", () => sup.playitLink())}>
                  <Link2 size={15} aria-hidden="true" /> Vincular mi cuenta de Playit
                </button>
              </>
            )}
          </>
        ) : phase === "over_limit" || phase === "error" ? (
          <p className="warn-text">
            <TriangleAlert size={14} aria-hidden="true" />{" "}
            {phase === "over_limit" ? "Tu cuenta de Playit superó el límite de agentes de su plan." : "El agente informa un error."}{" "}
            {agent?.message ?? ""}
          </p>
        ) : phase !== "running" ? (
          <p className="wiz-loading">El agente se está conectando…</p>
        ) : (
          <>
            {conn && conn.tunnels.length > 0 ? (
              conn.tunnels.map((t) => {
                const tested = testOf(t.display_address);
                const ok = !!tested?.result;
                return (
                  <div key={t.display_address} className="tunnel-box">
                    <p className="field-help">Dirección para tus amigos</p>
                    <div className="address-row">
                      <code className="address address-big">{t.display_address}</code>
                      <button
                        type="button"
                        className="btn btn-sm"
                        disabled={!ok}
                        title={ok ? undefined : "Pruébala antes de compartirla"}
                        onClick={() => copy(t.display_address)}
                        aria-label={`Copiar ${t.display_address}`}
                      >
                        <Copy size={14} aria-hidden="true" /> {copied === t.display_address ? "Copiada" : "Copiar"}
                      </button>
                      <button
                        type="button"
                        className="btn btn-sm btn-go"
                        disabled={!online || busy !== null || t.is_disabled}
                        onClick={() => run(`t-${t.display_address}`, () => sup.testAddress(id, t.display_address))}
                        aria-label={`Probar ${t.display_address}`}
                      >
                        {busy === `t-${t.display_address}` ? "Probando…" : "Probar conexión"}
                      </button>
                    </div>
                    {t.is_disabled ? (
                      <p className="warn-text">Playit tiene este túnel desactivado{t.disabled_reason ? `: ${t.disabled_reason}` : ""}.</p>
                    ) : (
                      <p>
                        <TestLine t={tested} />
                      </p>
                    )}
                    {!online && <p className="field-help">Enciende el servidor para probar la dirección.</p>}
                    <p className="field-help">
                      La prueba es real: MercHosting entra por la dirección pública como lo haría el juego y mide cuánto tarda en
                      responder.
                    </p>
                  </div>
                );
              })
            ) : strayTunnels.length > 0 ? (
              strayTunnels.map((t) => {
                const p = portOf(t.destination)!;
                const other = usedBy(p);
                return (
                  <div key={t.display_address} className="claim-box" aria-label="Puerto distinto">
                    <p className="warn-text">
                      <TriangleAlert size={14} aria-hidden="true" /> Tu túnel <strong>{t.display_address}</strong> lleva al puerto{" "}
                      <strong>{p}</strong>, pero este servidor usa el <strong>{port ?? "?"}</strong>. Por eso no se conecta.
                    </p>
                    {other ? (
                      <p className="field-help">
                        El puerto {p} lo usa tu servidor «{other}»: ese túnel es suyo. Para este, crea otro túnel hacia el puerto{" "}
                        {port ?? "del servidor"}.
                      </p>
                    ) : (
                      <>
                        <p>Arréglalo de una de estas dos formas:</p>
                        <ol>
                          <li>
                            Que este servidor use el puerto {p}:{" "}
                            <button
                              type="button"
                              className="btn btn-sm btn-go"
                              disabled={!stopped || busy !== null}
                              onClick={() => run("port", () => usePort(p))}
                            >
                              Usar el puerto {p} en este servidor
                            </button>
                            {!stopped && <span className="field-help"> (detén el servidor primero)</span>}
                          </li>
                          <li>
                            O en playit.gg abre el túnel › <strong>Origin Configuration</strong> y cambia el puerto local a{" "}
                            <code>{port}</code>. MercHosting lo detecta solo.
                          </li>
                        </ol>
                      </>
                    )}
                  </div>
                );
              })
            ) : (
              <div className="claim-box" aria-label="Crear el túnel">
                <p>
                  <strong>Falta el túnel de este servidor.</strong> Playit no permite crearlo desde otras aplicaciones con una cuenta
                  gratuita, así que es un paso tuyo, una sola vez:
                </p>
                <ol>
                  <li>Abre tu cuenta de playit.gg.</li>
                  <li>
                    Crea un túnel de tipo <strong>Minecraft Java</strong> para este agente, con dirección local <code>127.0.0.1</code>{" "}
                    y puerto <code>{port ?? "del servidor"}</code>.
                  </li>
                  <li>Vuelve aquí: MercHosting lo detecta solo en unos segundos.</li>
                </ol>
                <button
                  type="button"
                  className="btn btn-go btn-sm"
                  disabled={busy !== null}
                  onClick={() => run("login", async () => api.openUrl(await sup.playitLoginUrl()))}
                >
                  <ExternalLink size={14} aria-hidden="true" /> Abrir mi cuenta de playit.gg
                </button>
                {agent && agent.pending.length > 0 && (
                  <p className="field-help">Playit está preparando: {agent.pending.join(" · ")}</p>
                )}
              </div>
            )}
            {otherTunnels.length > 0 && strayTunnels.length === 0 && (
              <p className="field-help">
                Otros túneles de tu cuenta (van a otros puertos):{" "}
                {otherTunnels.map((t) => `${t.display_address} → ${t.destination}`).join(" · ")}
              </p>
            )}
            {agent && agent.notices.length > 0 && (
              <ul className="alert-list" aria-label="Avisos de Playit">
                {agent.notices.map((n, i) => (
                  <li key={i} data-level="warning">
                    <strong>Aviso de Playit</strong>
                    <span className="field-help">{n.message}</span>
                    {n.resolve_link?.startsWith("https://playit.gg/") && (
                      <button type="button" className="btn btn-sm" onClick={() => api.openUrl(n.resolve_link!)}>
                        <ExternalLink size={13} aria-hidden="true" /> Resolver
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            )}
            <div className="files-bar">
              <button
                type="button"
                className="btn btn-sm"
                disabled={busy !== null}
                onClick={() => run("login", async () => api.openUrl(await sup.playitLoginUrl()))}
              >
                <ExternalLink size={14} aria-hidden="true" /> Mi cuenta de playit.gg
              </button>
              <button type="button" className="btn btn-sm btn-quiet" disabled={busy !== null} onClick={() => setAskUnlink(true)}>
                <Unlink size={14} aria-hidden="true" /> Desvincular esta PC
              </button>
            </div>
          </>
        )}
        {playit?.enabled && (
          <div className="files-bar">
            <span className="field-help">
              Agente oficial de Playit {agent?.version ?? playit.agent_version}
              {playit.running ? " en marcha" : ""}.
            </span>
            <button type="button" className="btn btn-sm btn-quiet" disabled={busy !== null} onClick={() => run("disable", () => sup.playitDisable())}>
              <Power size={14} aria-hidden="true" /> Desactivar Playit
            </button>
          </div>
        )}
      </section>
      </>
      )}

      {voice && (
        <section className="card props-card" aria-label="Chat de voz">
          <h2>
            <Mic size={16} aria-hidden="true" /> Chat de voz · Simple Voice Chat
          </h2>
          {voice.installed ? (
            <dl className="voice-facts">
              <dt>Archivo</dt>
              <dd>
                <code>{voice.config ?? "todavía no existe"}</code>
              </dd>
              <dt>Puerto de voz (UDP)</dt>
              <dd>
                {voice.port ?? "—"}
                {voice.port_setting === -1 ? " (el mismo de Minecraft)" : ""}
              </dd>
              <dt>voice_host</dt>
              <dd>{voice.voice_host ? <code>{voice.voice_host}</code> : <span className="field-help">vacío</span>}</dd>
              {voice.tunnels.length > 0 && (
                <>
                  <dt>Túnel de voz</dt>
                  <dd>
                    {voice.tunnels.map((t) => (
                      <span key={t.display_address}>
                        <code>{t.display_address}</code> → {t.destination}
                        {t.is_disabled ? " (desactivado)" : ""}{" "}
                      </span>
                    ))}
                  </dd>
                </>
              )}
              {voice.listening !== null && (
                <>
                  <dt>En esta sesión</dt>
                  <dd>la consola dice que escucha en el puerto {voice.listening}</dd>
                </>
              )}
            </dl>
          ) : null}
          <ul className="alert-list" aria-label="Diagnóstico de la voz">
            {voice.problems.map((p) => (
              <li key={p.code} data-level={p.level === "error" ? "critical" : p.level === "warning" ? "warning" : "info"}>
                <span>{p.message}</span>
                {(p.code === "host_missing" || p.code === "host_mismatch") &&
                  voice.tunnels
                    .filter((t) => !t.is_disabled && hasPort(t.display_address))
                    .map((t) => (
                      <button
                        key={t.display_address}
                        type="button"
                        className="btn btn-sm btn-go"
                        disabled={busy !== null}
                        onClick={() =>
                          setVoiceAsk({
                            title: "Escribir la dirección de la voz",
                            lead: `Se cambia «voice_host» de ${voice.voice_host ? `«${voice.voice_host}»` : "vacío"} a «${t.display_address}» en ${voice.config}. MercHosting guarda antes una copia del archivo (Archivos › Copias). Se aplica al reiniciar el servidor.`,
                            change: { voice_host: t.display_address },
                          })
                        }
                      >
                        Usar {t.display_address} para la voz
                      </button>
                    ))}
                {(p.code === "shared_port" || p.code === "port_conflict") && voice.suggested_port !== null && (
                  <button
                    type="button"
                    className="btn btn-sm btn-go"
                    disabled={busy !== null}
                    onClick={() =>
                      setVoiceAsk({
                        title: "Cambiar el puerto de la voz",
                        lead: `Se cambia «port» de ${voice.port_setting ?? "?"} a ${voice.suggested_port} en ${voice.config}. MercHosting guarda antes una copia del archivo. Si usas Playit, el túnel de voz tendrá que ir a este puerto nuevo. Se aplica al reiniciar el servidor.`,
                        change: { port: voice.suggested_port! },
                      })
                    }
                  >
                    Usar el puerto {voice.suggested_port} para la voz
                  </button>
                )}
                {p.code === "no_tunnel" && !remote && (
                  <>
                    <ol>
                      <li>Abre tu cuenta de playit.gg y crea un túnel nuevo para este agente.</li>
                      <li>
                        Tipo <strong>Simple Voice Chat</strong>, IP local <code>127.0.0.1</code>, puerto local <code>{voice.port}</code>.
                      </li>
                      <li>Vuelve aquí: MercHosting lo detecta y te ofrece escribir su dirección en la configuración.</li>
                    </ol>
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={busy !== null}
                      onClick={() => run("login", async () => api.openUrl(await sup.playitLoginUrl()))}
                    >
                      <ExternalLink size={13} aria-hidden="true" /> Abrir mi cuenta de playit.gg
                    </button>
                  </>
                )}
                {p.code === "leftover_tunnel" && !remote && (
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={busy !== null}
                    onClick={() => run("login", async () => api.openUrl(await sup.playitLoginUrl()))}
                  >
                    <ExternalLink size={13} aria-hidden="true" /> Abrir mi cuenta de playit.gg
                  </button>
                )}
                {p.code === "restart_needed" && online && (
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={busy !== null}
                    onClick={() => run("restart", () => sup.restartWithNotice(id, 60))}
                  >
                    <RotateCw size={13} aria-hidden="true" /> Reiniciar avisando a los jugadores (1 min)
                  </button>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      {voiceAsk && (
        <Dialog
          title={voiceAsk.title}
          lead={voiceAsk.lead}
          onClose={() => setVoiceAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setVoiceAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                onClick={() => {
                  const change = voiceAsk.change;
                  setVoiceAsk(null);
                  run("voice", () => sup.setVoiceConfig(id, change));
                }}
              >
                Guardar
              </button>
            </>
          }
        />
      )}

      {askUnlink && (
        <Dialog
          title="Desvincular esta PC de Playit"
          lead="El agente olvida tu cuenta y tus amigos dejan de poder entrar por la dirección de Playit hasta que vuelvas a vincular. Los túneles siguen en tu cuenta de playit.gg."
          onClose={() => setAskUnlink(false)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setAskUnlink(false)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  setAskUnlink(false);
                  run("unlink", () => sup.playitUnlink());
                }}
              >
                Desvincular
              </button>
            </>
          }
        />
      )}
    </div>
  );
}
