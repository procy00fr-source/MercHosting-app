// MercHosting en el celular (Android).
// - «Mis PCs»: la app es un compañero que administra los servidores de tu PC o de tu servidor
//   casero con un código de Compañeros. Cada acción la decide esa PC según los permisos que te
//   dio; con «control total» puedes crear servidores Java y Bedrock allí y activar Playit.
// - «En este celular» (fase 2): un servidor Java dentro del propio celular (PhoneHost), con
//   avisos de riesgo obligatorios. Bedrock no se puede dentro del celular.

import { HardDrive, Info, LogIn, Plus, RefreshCw, Server, Smartphone, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { MercApi } from "../api/client";
import { Supervisor, errorMessage, remoteApi } from "../api/client";
import type { InstanceId, RemoteHost } from "../api/types";
import { ROLE_LABEL, roleOf } from "../companions";
import { uiLocale } from "../i18n/lang";
import { CreateWizard } from "./CreateWizard";
import { Dialog } from "./Dialog";
import type { PhoneApi } from "../api/phone";
import { Logo } from "./Logo";
import { PhoneHost } from "./PhoneHost";
import { RemoteServerView } from "./RemoteServerView";
import type { SidebarCommon } from "./Sidebar";

const NAME_KEY = "merchosting.mobileName";

/** Licencia del agente de Playit (va dentro del APK): la BSD-2-Clause pide mostrarla. */
const PLAYIT_LICENSE = `Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.`;

function when(ms: number | null) {
  if (!ms) return "nunca";
  return new Date(ms).toLocaleString(uiLocale(), { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

function savedName(): string {
  try {
    return localStorage.getItem(NAME_KEY) ?? "";
  } catch {
    return "";
  }
}

interface Props {
  api: MercApi;
  /** Huella de este celular (la ve la PC en su lista de compañeros). */
  identity?: () => Promise<string>;
  /** Servidores dentro de este celular (solo en Android). */
  phone?: PhoneApi;
}

type Tab = "pcs" | "here" | "join" | "about";

export function MobileApp({ api, identity, phone }: Props) {
  const sup = useMemo(() => new Supervisor(api), [api]);
  const [tab, setTab] = useState<Tab>("pcs");
  const [hosts, setHosts] = useState<RemoteHost[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [myName, setMyName] = useState(savedName);
  const [fingerprint, setFingerprint] = useState<string | null>(null);
  const [open, setOpen] = useState<{ host: RemoteHost; id: InstanceId } | null>(null);
  const [creating, setCreating] = useState<RemoteHost | null>(null);
  const [forgetting, setForgetting] = useState<RemoteHost | null>(null);

  const load = useCallback(async () => {
    try {
      const list = await sup.remoteHosts();
      setHosts(list);
      if (list.length === 0) setTab((t) => (t === "pcs" ? "join" : t));
    } catch (e) {
      setError(errorMessage(e));
      setHosts([]);
    }
  }, [sup]);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    identity?.().then(setFingerprint, () => setFingerprint(null));
  }, [identity]);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 9000);
    return () => clearTimeout(t);
  }, [toast]);

  async function run(key: string, fn: () => Promise<RemoteHost[]>) {
    setBusy(key);
    setError(null);
    try {
      setHosts(await fn());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  const sidebar: SidebarCommon = {
    brandTone: "brand",
    brandSub: "En tu celular",
    introSound: false,
    onToggleSound: () => undefined,
    side: "left",
    onToggleSide: () => undefined,
    supervisorOk: true,
    supervisorText: "Conectado por Compañeros",
    alertsUnread: 0,
    onOpenAlerts: () => undefined,
  };

  if (open)
    return (
      <div className="mobile-root">
        <div className="shell" data-side="left">
          <RemoteServerView
            key={`${open.host.host}-${open.id}`}
            api={api}
            host={open.host}
            id={open.id}
            sidebar={sidebar}
            onBack={() => {
              setOpen(null);
              load();
            }}
            onError={setToast}
          />
        </div>
        {toast && (
          <div className="toast" role="alert">
            <p>{toast}</p>
            <button type="button" className="btn btn-quiet btn-sm" onClick={() => setToast(null)}>
              Cerrar
            </button>
          </div>
        )}
      </div>
    );

  return (
    <div className="mobile-root">
      <header className="mobile-head">
        <Logo size={34} tone="brand" />
        <div>
          <p className="brand-name">MercHosting</p>
          <p className="brand-sub">{tab === "here" ? "Un servidor Java dentro de este celular" : "Administra los servidores de tu PC desde el celular"}</p>
        </div>
      </header>

      <main className="mobile-main">
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}

        {tab === "pcs" && (
          <section aria-label="Mis PCs" className="mobile-page">
            <div className="files-bar">
              <h1 className="panel-title">Mis PCs</h1>
              <button type="button" className="btn btn-sm" disabled={busy !== null} onClick={() => run("all", () => sup.remoteHosts())}>
                <RefreshCw size={14} aria-hidden="true" /> Actualizar
              </button>
            </div>
            {hosts === null ? (
              <p className="wiz-loading">Cargando…</p>
            ) : hosts.length === 0 ? (
              <div className="card props-card">
                <p>Todavía no conectaste ninguna PC.</p>
                <button type="button" className="btn btn-go" onClick={() => setTab("join")}>
                  <LogIn size={15} aria-hidden="true" /> Conectar con un código
                </button>
              </div>
            ) : (
              hosts.map((h) => (
                <article key={h.host} className="card props-card mobile-host" aria-label={h.name || "PC"}>
                  <div className="companion-head">
                    <strong translate="no">{h.name || "PC sin nombre"}</strong>
                    <span className="field-help">
                      PC {h.fingerprint}
                      {h.last_ok_unix_ms ? ` · respondió: ${when(h.last_ok_unix_ms)}` : ""}
                    </span>
                  </div>
                  <div className="row-actions">
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={busy !== null}
                      onClick={() => run(`ref-${h.host}`, () => sup.refreshHost(h.host))}
                      aria-label={`Actualizar ${h.name}`}
                    >
                      <RefreshCw size={14} aria-hidden="true" /> {busy === `ref-${h.host}` ? "Conectando…" : "Actualizar"}
                    </button>
                    <button
                      type="button"
                      className="btn btn-sm btn-quiet"
                      disabled={busy !== null}
                      onClick={() => setForgetting(h)}
                      aria-label={`Olvidar ${h.name}`}
                    >
                      <Trash2 size={14} aria-hidden="true" /> Olvidar
                    </button>
                  </div>
                  {h.last_error && <p className="warn-text">{h.last_error}</p>}
                  {h.host_admin && (
                    <div className="mobile-admin">
                      <span className="ok-text">
                        <HardDrive size={14} aria-hidden="true" /> Control total de esa PC
                      </span>
                      <button type="button" className="btn btn-go" onClick={() => setCreating(h)}>
                        <Plus size={15} aria-hidden="true" /> Crear servidor (Java o Bedrock)
                      </button>
                    </div>
                  )}
                  {h.servers.length === 0 ? (
                    <p className="field-help">
                      {h.host_admin ? "Todavía no hay servidores en esa PC." : "No tienes acceso a ningún servidor de esta PC."}
                    </p>
                  ) : (
                    <ul className="mobile-servers">
                      {h.servers.map((s) => (
                        <li key={s.id}>
                          <button type="button" className="mobile-server" onClick={() => setOpen({ host: h, id: s.id })}>
                            <Server size={18} aria-hidden="true" />
                            <span>
                              <strong translate="no">{s.name}</strong>
                              <span className="field-help">{h.host_admin ? "Control total" : ROLE_LABEL[roleOf(s.perms)]}</span>
                            </span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </article>
              ))
            )}
          </section>
        )}

        {tab === "here" && phone && <PhoneHost phone={phone} api={api} onError={setToast} />}

        {tab === "join" && (
          <section aria-label="Conectar" className="mobile-page">
            <h1 className="panel-title">Conectar una PC</h1>
            <div className="card props-card">
              <p>
                En tu PC, abre MercHosting › <strong>Compañeros</strong> y crea una invitación. Para manejarla entera (crear servidores,
                Playit…) elige <strong>Control total (servidor casero)</strong>. En una PC sin pantalla usa{" "}
                <code>merc compañeros invitar --control-total</code>.
              </p>
              <form
                className="join-form"
                onSubmit={(e) => {
                  e.preventDefault();
                  const name = myName.trim();
                  try {
                    localStorage.setItem(NAME_KEY, name);
                  } catch {
                    /* sin almacenamiento */
                  }
                  run("join", async () => {
                    const list = await sup.joinWithCode(code.trim(), name);
                    setCode("");
                    setTab("pcs");
                    return list;
                  });
                }}
              >
                <label className="field">
                  <span className="field-title">Código de invitación</span>
                  <input
                    type="text"
                    value={code}
                    onChange={(e) => setCode(e.target.value)}
                    placeholder="MH1-…"
                    spellCheck={false}
                    autoCapitalize="off"
                    autoCorrect="off"
                  />
                </label>
                <label className="field">
                  <span className="field-title">Nombre de este celular (así lo verá la PC)</span>
                  <input type="text" value={myName} onChange={(e) => setMyName(e.target.value)} maxLength={32} />
                </label>
                <button type="submit" className="btn btn-go btn-lg" disabled={busy !== null || !code.trim() || !myName.trim()}>
                  {busy === "join" ? "Conectando…" : "Conectar"}
                </button>
              </form>
              <p className="field-help">
                Pega el código que te muestra la PC (cópialo en la PC y pásalo a tu celular por un mensaje a ti mismo). Cada código sirve
                una sola vez y caduca.
              </p>
            </div>
          </section>
        )}

        {tab === "about" && (
          <section aria-label="Acerca de" className="mobile-page">
            <h1 className="panel-title">Cómo funciona</h1>
            <div className="card props-card">
              <ul className="plain-list">
                <li>
                  En «Mis PCs» los servidores corren en tu PC o en tu servidor casero. La PC tiene que estar encendida, con MercHosting
                  abierto o en segundo plano, y conectada a Internet.
                </li>
                {phone && (
                  <li>
                    En «En este celular» puedes encender un servidor Java dentro del propio celular (gasta batería, se calienta y Android
                    puede cerrarlo). Bedrock solo se puede en una PC.
                  </li>
                )}
                <li>
                  El celular se conecta con el canal cifrado de Compañeros (sin cuentas ni puertos abiertos). La PC decide cada acción
                  según los permisos que le dio a este celular, y todo queda en su Actividad.
                </li>
                <li>Para que tus amigos jueguen desde fuera, activa Playit en la pestaña Conexión del servidor (con control total).</li>
                <li>Descargar o subir archivos se hace desde MercHosting en una PC.</li>
              </ul>
              {phone && (
                <details className="field-help">
                  <summary>Componentes de terceros del servidor dentro del celular</summary>
                  <p>
                    Java para Android: OpenJDK compilado por AngelAuraMC (github.com/AngelAuraMC/angelauramc-openjdk-build), GPLv2 con
                    Classpath Exception. No va dentro de la app: se descarga al crear el primer servidor y se comprueba su huella SHA-256.
                  </p>
                  <p>
                    Agente de Playit (playitd v1.2.0, github.com/playit-cloud/playit-agent), compilado para Android desde su código
                    oficial y, si la compilación salió bien, incluido en esta app. Licencia BSD-2-Clause:
                  </p>
                  <pre className="license-text" translate="no">{PLAYIT_LICENSE}</pre>
                </details>
              )}
              {fingerprint && (
                <p className="field-help">
                  <Smartphone size={13} aria-hidden="true" /> Huella de este celular: <code translate="no">{fingerprint}</code> (la verás
                  en la lista de compañeros de tu PC).
                </p>
              )}
            </div>
          </section>
        )}
      </main>

      <nav className="mobile-nav" aria-label="Secciones">
        <button type="button" aria-current={tab === "pcs" ? "page" : undefined} onClick={() => setTab("pcs")}>
          <HardDrive size={20} aria-hidden="true" />
          <span>Mis PCs</span>
        </button>
        {phone && (
          <button type="button" aria-current={tab === "here" ? "page" : undefined} onClick={() => setTab("here")}>
            <Smartphone size={20} aria-hidden="true" />
            <span>En este celular</span>
          </button>
        )}
        <button type="button" aria-current={tab === "join" ? "page" : undefined} onClick={() => setTab("join")}>
          <LogIn size={20} aria-hidden="true" />
          <span>Conectar</span>
        </button>
        <button type="button" aria-current={tab === "about" ? "page" : undefined} onClick={() => setTab("about")}>
          <Info size={20} aria-hidden="true" />
          <span>Ayuda</span>
        </button>
      </nav>

      {creating && (
        <CreateWizard
          api={remoteApi(api, creating.host)}
          system={null}
          usedPorts={[]}
          remote={{ hostName: creating.name || "tu servidor casero" }}
          onClose={() => setCreating(null)}
          onCreated={async (id) => {
            const h = creating;
            setCreating(null);
            const list = await sup.refreshHost(h.host).catch(() => null);
            if (list) setHosts(list);
            setOpen({ host: list?.find((x) => x.host === h.host) ?? h, id });
          }}
        />
      )}

      {forgetting && (
        <Dialog
          title={`¿Olvidar ${forgetting.name || "esta PC"}?`}
          lead="Se quita de este celular. En esa PC seguirás apareciendo como compañero hasta que te quiten allí; para volver a conectar necesitarás un código nuevo."
          onClose={() => setForgetting(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setForgetting(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  const h = forgetting;
                  setForgetting(null);
                  run(`forget-${h.host}`, () => sup.forgetHost(h.host));
                }}
              >
                Olvidar
              </button>
            </>
          }
        />
      )}

      {toast && (
        <div className="toast" role="alert">
          <p>{toast}</p>
          <button type="button" className="btn btn-quiet btn-sm" onClick={() => setToast(null)}>
            Cerrar
          </button>
        </div>
      )}
    </div>
  );
}
