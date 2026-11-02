// «En este celular» (fase 2): un servidor de Minecraft JAVA que corre dentro del propio celular
// Android, en un servicio en primer plano con notificación. Antes de crear nada hay que aceptar
// uno por uno los avisos de riesgo. El servidor oficial de Bedrock no existe para Android: para
// jugadores de Bedrock se crea un Paper con Geyser y Floodgate. Mods y plugins de Modrinth.

import { AlertTriangle, ExternalLink, HardDriveDownload, Link2, Play, Plus, Power, RefreshCw, Send, Square, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi } from "../api/client";
import { errorMessage } from "../api/client";
import type { CatalogEntry } from "../api/types";
import type { CreateProgress, LocalServer, LocalStatus, PhoneApi, PhoneFlavor, PhoneInfo, PhonePlayit, PhoneWarning } from "../api/phone";
import { PHONE_FLAVORS, PHONE_WARNINGS } from "../api/phone";
import { Dialog } from "./Dialog";
import { PhoneContent } from "./PhoneContent";

const EULA_URL = "https://aka.ms/MinecraftEULA";

export const WARNING_TEXT: Record<PhoneWarning, { title: string; text: string }> = {
  bateria: {
    title: "Gasta mucha batería",
    text: "Un servidor usa el procesador todo el tiempo. Con la batería sola dura poco: úsalo enchufado.",
  },
  calor: {
    title: "El celular se calienta",
    text: "Si se calienta mucho, Android baja la velocidad o cierra apps. No lo tapes ni lo dejes al sol ni cargando debajo de una almohada.",
  },
  cierre: {
    title: "Android puede cerrarlo",
    text: "Aunque tenga su notificación, Android (o el ahorro de batería del fabricante) puede cerrar el servidor sin avisar. Haz copias de seguridad.",
  },
  cargador_wifi: {
    title: "Mejor con cargador y wifi",
    text: "Para jugar un rato largo, déjalo enchufado y conectado a un wifi estable.",
  },
  datos: {
    title: "Gasta datos móviles",
    text: "Con datos móviles cada jugador consume tu plan. Con Playit también pasa todo el tráfico por Internet.",
  },
  rendimiento: {
    title: "Rinde menos que una PC",
    text: "Es para pocos amigos (2 a 5) y pocos mods o plugins. Si va lento, baja la distancia de visión o usa tu PC.",
  },
  desgaste: {
    title: "Desgasta el celular",
    text: "Mucho calor y carga por horas envejecen la batería. Si es tu celular de todos los días, úsalo con moderación.",
  },
};

/** Lo que se elige al crear: un tipo de Java o «Java + Bedrock» (Paper con Geyser y Floodgate). */
type PhoneChoice = PhoneFlavor | "bedrock";
const CHOICES: PhoneChoice[] = ["paper", "bedrock", ...PHONE_FLAVORS.filter((f) => f !== "paper")];

const CHOICE_LABEL: Record<PhoneChoice, string> = {
  paper: "Paper (plugins, recomendado)",
  bedrock: "Java + Bedrock (Paper con Geyser)",
  vanilla: "Vanilla (oficial de Mojang)",
  purpur: "Purpur (plugins)",
  fabric: "Fabric (mods)",
};

const STATE_LABEL: Record<LocalStatus["state"], string> = {
  stopped: "Apagado",
  starting: "Encendiendo…",
  online: "En línea",
  stopping: "Apagando…",
};

interface Props {
  phone: PhoneApi;
  api: MercApi;
  onError: (msg: string) => void;
}

export function PhoneHost({ phone, api, onError }: Props) {
  const [info, setInfo] = useState<PhoneInfo | null>(null);
  const [servers, setServers] = useState<LocalServer[] | null>(null);
  const [creating, setCreating] = useState(false);
  const [open, setOpen] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [i, s] = await Promise.all([phone.info(), phone.servers()]);
      setInfo(i);
      setServers(s);
    } catch (e) {
      onError(errorMessage(e));
      setServers([]);
    }
  }, [phone, onError]);

  useEffect(() => {
    load();
  }, [load]);

  const current = servers?.find((s) => s.id === open) ?? null;
  if (current && info)
    return (
      <PhoneServerView
        key={current.id}
        phone={phone}
        api={api}
        info={info}
        server={current}
        onBack={() => {
          setOpen(null);
          load();
        }}
        onChanged={(s) => setServers((list) => list?.map((x) => (x.id === s.id ? s : x)) ?? null)}
        onRemoved={() => {
          setOpen(null);
          load();
        }}
        onError={onError}
      />
    );

  return (
    <section aria-label="En este celular" className="mobile-page">
      <div className="files-bar">
        <h1 className="panel-title">En este celular</h1>
        <button type="button" className="btn btn-sm" onClick={load}>
          <RefreshCw size={14} aria-hidden="true" /> Actualizar
        </button>
      </div>
      <div className="card props-card">
        <p>
          Puedes encender un servidor de Minecraft dentro de este celular para jugar con pocos amigos, de <strong>Java</strong> o
          también de <strong>Bedrock</strong>.
        </p>
        <p className="field-help">
          Para Bedrock se crea un servidor Java con <strong>Geyser y Floodgate</strong>: entran jugadores de Java y de Bedrock
          (celular, consola, Windows) al mismo mundo. El servidor oficial de Bedrock solo existe para PC (Windows y Linux x86),
          así que aquí no hay addons .mcpack; para eso usa «Mis PCs». Forge y NeoForge tampoco: necesitan ejecutar su
          instalador de Java, que Android no deja.
        </p>
      </div>

      {info === null ? (
        <p className="wiz-loading">Cargando…</p>
      ) : !info.supported ? (
        <div className="card props-card" role="note">
          <p className="warn-text">
            <AlertTriangle size={14} aria-hidden="true" /> {info.reason}
          </p>
          <p className="field-help">Puedes seguir usando «Mis PCs» para manejar los servidores de tu PC.</p>
        </div>
      ) : (
        <>
          <p className="field-help">
            Memoria del celular: {info.total_ram_mb} MB · para el servidor como mucho {info.heap?.max_mb} MB.
            {info.playit_available ? " Playit incluido." : " Esta versión de la app no trae Playit."}
          </p>
          <button type="button" className="btn btn-go" onClick={() => setCreating(true)}>
            <Plus size={15} aria-hidden="true" /> Crear servidor en este celular
          </button>
          {servers === null ? null : servers.length === 0 ? (
            <p className="field-help">Todavía no hay servidores en este celular.</p>
          ) : (
            <ul className="mobile-servers">
              {servers.map((s) => (
                <li key={s.id}>
                  <button type="button" className="mobile-server" onClick={() => setOpen(s.id)}>
                    <Play size={18} aria-hidden="true" />
                    <span>
                      <strong translate="no">{s.name}</strong>
                      <span className="field-help">
                        {s.flavor} {s.mc_version}
                        {s.bedrock ? " + Bedrock" : ""} · {s.heap_mb} MB
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      {creating && info?.supported && info.heap && (
        <PhoneCreate
          phone={phone}
          api={api}
          info={info}
          onClose={() => setCreating(false)}
          onCreated={(s) => {
            setCreating(false);
            setServers((list) => [...(list ?? []), s]);
            setOpen(s.id);
          }}
        />
      )}
    </section>
  );
}

function PhoneCreate({
  phone,
  api,
  info,
  onClose,
  onCreated,
}: {
  phone: PhoneApi;
  api: MercApi;
  info: PhoneInfo;
  onClose: () => void;
  onCreated: (s: LocalServer) => void;
}) {
  const heap = info.heap!;
  const [step, setStep] = useState<"warnings" | "form">("warnings");
  const [accepted, setAccepted] = useState<Set<PhoneWarning>>(new Set());
  const [choice, setChoice] = useState<PhoneChoice>("paper");
  const bedrock = choice === "bedrock";
  const flavor: PhoneFlavor = bedrock ? "paper" : choice;
  const [versions, setVersions] = useState<CatalogEntry[] | null>(null);
  const [version, setVersion] = useState("");
  const [name, setName] = useState("Mi servidor");
  const [ram, setRam] = useState(heap.suggested_mb);
  const [eula, setEula] = useState(false);
  const [progress, setProgress] = useState<CreateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const all = PHONE_WARNINGS.every((w) => accepted.has(w));

  useEffect(() => {
    if (step !== "form") return;
    let alive = true;
    setVersions(null);
    setError(null);
    phone.versions(flavor).then(
      (v) => {
        if (!alive) return;
        setVersions(v);
        setVersion(v.find((x) => x.latest)?.mc_version ?? v[0]?.mc_version ?? "");
      },
      (e) => alive && (setVersions([]), setError(errorMessage(e))),
    );
    return () => {
      alive = false;
    };
  }, [phone, flavor, step]);

  useEffect(() => {
    if (!progress?.running) return;
    const t = setInterval(() => phone.progress().then(setProgress, () => undefined), 700);
    return () => clearInterval(t);
  }, [phone, progress?.running]);

  async function create() {
    setError(null);
    setProgress({ running: true, step: "Comprobando", done: 0, total: null, error: null });
    try {
      const s = await phone.create({
        name: name.trim(),
        flavor,
        mc_version: version,
        heap_mb: ram,
        accept_eula: eula,
        warnings_accepted: [...accepted],
        bedrock,
      });
      onCreated(s);
    } catch (e) {
      setError(errorMessage(e));
      setProgress(null);
    }
  }

  const busy = progress?.running === true;
  const pct = progress?.total ? Math.round((progress.done / progress.total) * 100) : null;

  return (
    <Dialog
      title="Crear un servidor en este celular"
      lead={step === "warnings" ? "Lee y marca cada aviso. Sin aceptarlos todos no se puede seguir." : undefined}
      onClose={busy ? () => undefined : onClose}
      footer={
        step === "warnings" ? (
          <>
            <button type="button" className="btn btn-quiet" onClick={onClose}>
              Cancelar
            </button>
            <button type="button" className="btn btn-go" disabled={!all} onClick={() => setStep("form")}>
              Entiendo los riesgos, seguir
            </button>
          </>
        ) : busy ? (
          <button type="button" className="btn btn-quiet" onClick={() => phone.cancel()}>
            Cancelar descarga
          </button>
        ) : (
          <>
            <button type="button" className="btn btn-quiet" onClick={() => setStep("warnings")}>
              Atrás
            </button>
            <button
              type="button"
              className="btn btn-go"
              disabled={!eula || !version || !name.trim() || versions === null}
              onClick={create}
            >
              Crear servidor
            </button>
          </>
        )
      }
    >
      {step === "warnings" ? (
        <div className="wiz-page" role="group" aria-label="Avisos de riesgo">
          {PHONE_WARNINGS.map((w) => (
            <label key={w} className="switch-row">
              <input
                type="checkbox"
                checked={accepted.has(w)}
                onChange={(e) =>
                  setAccepted((prev) => {
                    const next = new Set(prev);
                    if (e.target.checked) next.add(w);
                    else next.delete(w);
                    return next;
                  })
                }
              />
              <span>
                <strong>{WARNING_TEXT[w].title}</strong>
                <span className="field-help">{WARNING_TEXT[w].text}</span>
              </span>
            </label>
          ))}
        </div>
      ) : (
        <div className="wiz-page">
          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}
          <label className="field">
            <span className="field-title">Nombre</span>
            <input type="text" value={name} maxLength={40} onChange={(e) => setName(e.target.value)} disabled={busy} />
          </label>
          <label className="field">
            <span className="field-title">Tipo</span>
            <select value={choice} onChange={(e) => setChoice(e.target.value as PhoneChoice)} disabled={busy}>
              {CHOICES.map((f) => (
                <option key={f} value={f}>
                  {CHOICE_LABEL[f]}
                </option>
              ))}
            </select>
            {bedrock && (
              <span className="field-help">
                Se instalan Geyser y Floodgate (de GeyserMC, gratis y de código abierto) desde Modrinth. Entran jugadores de Java y de
                Bedrock, y los de Bedrock no necesitan cuenta de Java. Antes de descargar se comprueba que existan para la versión
                elegida.
              </span>
            )}
          </label>
          <label className="field">
            <span className="field-title">Versión de Minecraft</span>
            {versions === null ? (
              <span className="wiz-loading">Cargando versiones…</span>
            ) : (
              <select value={version} onChange={(e) => setVersion(e.target.value)} disabled={busy}>
                {versions.map((v) => (
                  <option key={v.mc_version} value={v.mc_version}>
                    {v.mc_version}
                    {v.latest ? " (la más nueva)" : ""}
                  </option>
                ))}
              </select>
            )}
            <span className="field-help">En el celular se ofrecen de la 1.18 en adelante (Java 17 y 21 para Android).</span>
          </label>
          <label className="field">
            <span className="field-title">Memoria para el servidor: {ram} MB</span>
            <input
              type="range"
              min={1024}
              max={heap.max_mb}
              step={256}
              value={ram}
              onChange={(e) => setRam(Number(e.target.value))}
              disabled={busy}
            />
            <span className="field-help">
              Recomendado {heap.suggested_mb} MB. Como mucho {heap.max_mb} MB: el resto lo necesita Android.
            </span>
          </label>
          <div className="detect-box">
            <p className="field-help">
              Se descargará el Java para Android (unos 28 MB, comprobado con su huella) y el servidor. Usa wifi si puedes.
            </p>
            <button type="button" className="btn btn-sm" onClick={() => api.openUrl(EULA_URL)}>
              <ExternalLink size={14} aria-hidden="true" /> Leer la EULA (aka.ms/MinecraftEULA)
            </button>
          </div>
          <label className="switch-row">
            <input type="checkbox" checked={eula} onChange={(e) => setEula(e.target.checked)} disabled={busy} />
            <span>
              <strong>He leído y acepto la EULA de Minecraft</strong>
            </span>
          </label>
          {busy && (
            <p className="field-help" role="status">
              {progress?.step}
              {pct !== null ? ` · ${pct}%` : "…"}
            </p>
          )}
        </div>
      )}
    </Dialog>
  );
}

function PhoneServerView({
  phone,
  api,
  info,
  server,
  onBack,
  onChanged,
  onRemoved,
  onError,
}: {
  phone: PhoneApi;
  api: MercApi;
  info: PhoneInfo;
  server: LocalServer;
  onBack: () => void;
  onChanged: (s: LocalServer) => void;
  onRemoved: () => void;
  onError: (msg: string) => void;
}) {
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [lines, setLines] = useState<string[]>([]);
  const [playit, setPlayit] = useState<PhonePlayit | null>(null);
  const [cmd, setCmd] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<"remove" | "kill" | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [st, con, pl] = await Promise.all([phone.status(server.id), phone.console(server.id), phone.playit(server.id)]);
      setStatus(st);
      setLines(con.slice(-300));
      setPlayit(pl);
    } catch (e) {
      onError(errorMessage(e));
    }
  }, [phone, server.id, onError]);

  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 2000);
    return () => clearInterval(t);
  }, [refresh]);

  async function act(key: string, fn: () => Promise<unknown>) {
    setBusy(key);
    try {
      await fn();
      await refresh();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  const on = status !== null && status.state !== "stopped";
  const heap = info.heap;

  return (
    <section aria-label={server.name} className="mobile-page">
      <div className="files-bar">
        <button type="button" className="btn btn-sm btn-quiet" onClick={onBack}>
          ← En este celular
        </button>
      </div>
      <div className="card props-card">
        <div className="companion-head">
          <strong translate="no">{server.name}</strong>
          <span className="field-help">
            {server.flavor} {server.mc_version} · Java {server.java_major} · {server.heap_mb} MB
          </span>
        </div>
        <p role="status">
          <span className={status?.state === "online" ? "badge" : "badge badge-muted"}>{status ? STATE_LABEL[status.state] : "…"}</span>
          {status?.port ? <span className="field-help"> · Java: puerto {status.port} (wifi de tu casa)</span> : null}
        </p>
        {status?.bedrock_port ? (
          <p className="field-help">
            Jugadores de Bedrock: agrega un servidor con la IP de este celular en tu wifi y el puerto{" "}
            <strong>{status.bedrock_port}</strong> (UDP).
          </p>
        ) : null}
        {status?.problem && <p className="warn-text">{status.problem}</p>}
        {status && !status.eula_accepted && (
          <div className="detect-box">
            <p className="field-help">Para encenderlo tienes que aceptar la EULA de Minecraft.</p>
            <button type="button" className="btn btn-sm" onClick={() => api.openUrl(EULA_URL)}>
              <ExternalLink size={14} aria-hidden="true" /> Leer la EULA
            </button>
            <button type="button" className="btn btn-sm" disabled={busy !== null} onClick={() => act("eula", () => phone.acceptEula(server.id))}>
              He leído y acepto la EULA
            </button>
          </div>
        )}
        <div className="row-actions">
          {!on ? (
            <button
              type="button"
              className="btn btn-go"
              disabled={busy !== null || !status?.eula_accepted}
              onClick={() =>
                act("start", async () => {
                  await phone.askNotifications();
                  await phone.start(server.id);
                })
              }
            >
              <Power size={15} aria-hidden="true" /> {busy === "start" ? "Encendiendo…" : "Encender"}
            </button>
          ) : (
            <>
              <button type="button" className="btn btn-stop" disabled={busy !== null || status?.state === "stopping"} onClick={() => act("stop", () => phone.stop(server.id))}>
                <Square size={14} aria-hidden="true" /> Detener (guarda el mundo)
              </button>
              <button type="button" className="btn btn-sm btn-quiet" disabled={busy !== null} onClick={() => setConfirm("kill")}>
                Forzar cierre
              </button>
            </>
          )}
        </div>
        <p className="field-help">
          Mientras está encendido verás una notificación con el botón «Detener». Si Android lo cierra, no se vuelve a encender solo.
        </p>
      </div>

      {!on && heap && (
        <div className="card props-card">
          <label className="field">
            <span className="field-title">Memoria: {server.heap_mb} MB</span>
            <input
              type="range"
              min={1024}
              max={heap.max_mb}
              step={256}
              value={server.heap_mb}
              onChange={(e) => phone.setHeap(server.id, Number(e.target.value)).then(onChanged, (err) => onError(errorMessage(err)))}
            />
          </label>
        </div>
      )}

      <div className="card props-card" aria-label="Amigos por Internet" role="region">
        <h2 className="panel-title">Amigos por Internet (Playit)</h2>
        {!playit?.available ? (
          <p className="field-help">
            Esta versión de la app no trae el agente de Playit. Tus amigos solo pueden entrar si están en tu mismo wifi (con la IP
            del celular y el puerto {status?.port ?? 25565}).
          </p>
        ) : (
          <>
            <label className="switch-row">
              <input
                type="checkbox"
                checked={server.playit}
                disabled={on || busy !== null}
                onChange={(e) =>
                  act("playit", async () => {
                    onChanged(await phone.setPlayit(server.id, e.target.checked));
                  })
                }
              />
              <span>
                <strong>Encender Playit junto con el servidor</strong>
                <span className="field-help">Con datos móviles, cada amigo gasta tu plan.</span>
              </span>
            </label>
            {playit.running ? (
              playit.linked ? (
                playit.address ? (
                  <p className="ok-text">
                    Dirección para tus amigos: <code translate="no">{playit.address}</code>
                  </p>
                ) : (
                  <p className="field-help">
                    Playit está vinculado. Crea un túnel «Minecraft Java» en playit.gg hacia el puerto {status?.port ?? 25565} de este
                    agente; aparecerá aquí.
                  </p>
                )
              ) : (
                <div className="detect-box">
                  <p className="field-help">Vincula Playit con TU cuenta de playit.gg (gratis). La clave la guarda el agente en este celular.</p>
                  {playit.claim && !playit.claim.done ? (
                    <>
                      <button type="button" className="btn btn-sm" onClick={() => api.openUrl(playit.claim!.url)}>
                        <ExternalLink size={14} aria-hidden="true" /> Abrir playit.gg y aprobar
                      </button>
                      {playit.claim.error && <p className="warn-text">{playit.claim.error}</p>}
                    </>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={busy !== null}
                      onClick={() =>
                        act("link", async () => {
                          const c = await phone.playitLink();
                          if (c?.url) await api.openUrl(c.url);
                        })
                      }
                    >
                      <Link2 size={14} aria-hidden="true" /> Vincular con playit.gg
                    </button>
                  )}
                </div>
              )
            ) : (
              <p className="field-help">
                {server.playit ? "Playit arranca cuando enciendes el servidor." : "Playit está apagado para este servidor."}
              </p>
            )}
            {playit.running && playit.linked && status?.bedrock_port ? (
              playit.bedrock_address ? (
                <p className="ok-text">
                  Dirección para Bedrock: <code translate="no">{playit.bedrock_address}</code>
                </p>
              ) : (
                <p className="field-help">
                  Para Bedrock crea un segundo túnel en playit.gg, de tipo «Minecraft Bedrock» (UDP), hacia el puerto{" "}
                  {status.bedrock_port}; aparecerá aquí. Pruébalo primero en tu wifi: hay reportes de túneles UDP de Playit que fallan
                  con Bedrock.
                </p>
              )
            ) : null}
            {playit.running && playit.linked && (
              <button type="button" className="btn btn-sm btn-quiet" disabled={busy !== null} onClick={() => act("unlink", () => phone.playitUnlink())}>
                Desvincular Playit
              </button>
            )}
          </>
        )}
      </div>

      <PhoneContent phone={phone} api={api} id={server.id} running={on} onError={onError} />

      <div className="card props-card" aria-label="Consola" role="region">
        <h2 className="panel-title">Consola</h2>
        <div className="console-lines" translate="no" style={{ maxHeight: 260 }}>
          {lines.length === 0 ? <p className="console-empty">Sin mensajes todavía.</p> : lines.map((l, i) => <div key={i}>{l}</div>)}
        </div>
        {on && (
          <form
            className="join-form"
            onSubmit={(e) => {
              e.preventDefault();
              const t = cmd.trim();
              if (!t) return;
              act("cmd", async () => {
                await phone.command(server.id, t);
                setCmd("");
              });
            }}
          >
            <label className="field">
              <span className="field-title">Comando</span>
              <input type="text" value={cmd} onChange={(e) => setCmd(e.target.value)} placeholder="say hola" autoCapitalize="off" />
            </label>
            <button type="submit" className="btn btn-sm" disabled={!cmd.trim() || busy !== null}>
              <Send size={14} aria-hidden="true" /> Enviar
            </button>
          </form>
        )}
      </div>

      {!on && (
        <div className="card props-card">
          <div className="row-actions">
            <button type="button" className="btn btn-sm" disabled={busy !== null} onClick={() => act("backup", () => phone.backup(server.id))}>
              <HardDriveDownload size={14} aria-hidden="true" /> {busy === "backup" ? "Copiando…" : "Hacer copia de seguridad"}
            </button>
            <button type="button" className="btn btn-sm btn-quiet" disabled={busy !== null} onClick={() => setConfirm("remove")}>
              <Trash2 size={14} aria-hidden="true" /> Quitar
            </button>
          </div>
          <p className="field-help">Las copias se guardan dentro de la app (se conservan las 5 últimas).</p>
        </div>
      )}

      {confirm === "remove" && (
        <Dialog
          title={`¿Quitar ${server.name}?`}
          lead="Antes se hace una copia de seguridad. La carpeta no se borra: pasa a la papelera de la app, dentro del celular."
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
                  setConfirm(null);
                  act("remove", async () => {
                    await phone.remove(server.id);
                    onRemoved();
                  });
                }}
              >
                Quitar
              </button>
            </>
          }
        />
      )}
      {confirm === "kill" && (
        <Dialog
          title="¿Forzar el cierre?"
          lead="Se cierra sin guardar: puedes perder lo último que pasó en el mundo. Úsalo solo si «Detener» no responde."
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
                  setConfirm(null);
                  act("kill", () => phone.forceStop(server.id));
                }}
              >
                Forzar cierre
              </button>
            </>
          }
        />
      )}
    </section>
  );
}
