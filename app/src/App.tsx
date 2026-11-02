import { CircleHelp, FolderInput, Handshake, Heart, Info, MessageSquare, Palette, Plus } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MercApi } from "./api/client";
import { Supervisor, errorMessage, remoteApi } from "./api/client";
import type { AlertList, AppUpdateCheck, InstanceId, InstanceSummary, RemoteHost, SupervisorInfo, SupporterInfo, SystemInfo } from "./api/types";
import { applyMode, applyTheme, effective, isLight, loadPrefs, reducedMotion, savePrefs, type AppearancePrefs } from "./appearance";
import { iconSvg } from "./appIcon";
import { AboutPage } from "./components/AboutPage";
import { AppearancePage } from "./components/AppearancePage";
import { FeedbackPage } from "./components/FeedbackPage";
import { FEEDBACK_FORM, validForm, type FeedbackForm } from "./feedback";
import { Celebration } from "./components/Celebration";
import { SupporterPage } from "./components/SupporterPage";
import { HelpPage } from "./components/HelpPage";
import { TourOverlay } from "./components/TourOverlay";
import { WelcomeDialog } from "./components/WelcomeDialog";
import { tourById, type Tour, type TourNav } from "./tours";
import { loadLevel, markWelcomeSeen, saveLevel, UiLevelContext, welcomeSeen, type UiLevel } from "./uiLevel";
import { AlertsDialog } from "./components/AlertsDialog";
import { CompanionsPage } from "./components/CompanionsPage";
import { CreateWizard } from "./components/CreateWizard";
import { ImportDialog } from "./components/ImportDialog";
import { Logo } from "./components/Logo";
import { Particles } from "./components/Particles";
import { RemoteServerView } from "./components/RemoteServerView";
import { ServerView } from "./components/ServerView";
import { ServersHome } from "./components/ServersHome";
import { Sidebar, type SidebarCommon } from "./components/Sidebar";
import { Splash } from "./components/Splash";
import { introSoundEnabled, setIntroSoundEnabled } from "./sound/introSound";
import { t } from "./i18n/translate";

const LIST_POLL_MS = 1000;
const SIDE_KEY = "merchosting.navSide";
/** Hasta qué momento ya se notificaron alertas (para no repetirlas al reabrir). */
const SEEN_KEY = "merchosting.alertsNotifiedUntil";

function readSeen(): number {
  try {
    const v = Number(localStorage.getItem(SEEN_KEY));
    if (Number.isFinite(v) && v > 0) return v;
    // Primera vez: no se notifican las alertas antiguas.
    const now = Date.now();
    localStorage.setItem(SEEN_KEY, String(now));
    return now;
  } catch {
    return Date.now();
  }
}

export function App({
  api,
  intro = true,
  welcome = intro,
  alertsPollMs = 5000,
  feedbackForm = FEEDBACK_FORM,
}: {
  api: MercApi;
  /** Formulario de comentarios (en pruebas se pasa uno; sin él, la sección no aparece). */
  feedbackForm?: FeedbackForm | null;
  intro?: boolean;
  /** Mostrar la bienvenida del primer inicio (si no se vio antes). */
  welcome?: boolean;
  alertsPollMs?: number;
}) {
  const sup = useMemo(() => new Supervisor(api), [api]);
  // Versión nueva de la app: se mira una vez al abrir (sin molestar si no hay conexión).
  const [appUpdate, setAppUpdate] = useState<AppUpdateCheck | null>(null);
  useEffect(() => {
    let alive = true;
    const t = setTimeout(() => {
      api.appUpdateCheck().then(
        (c) => alive && setAppUpdate(c),
        () => undefined,
      );
    }, 3000);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [api]);
  const [info, setInfo] = useState<SupervisorInfo | null>(null);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [connectError, setConnectError] = useState<string | null>(null);
  const [servers, setServers] = useState<InstanceSummary[] | null>(null);
  const [selected, setSelected] = useState<InstanceId | null>(null);
  const [importing, setImporting] = useState(false);
  const [creating, setCreating] = useState(false);
  /** Crear un servidor en tu servidor casero (otra PC con control total). */
  const [creatingRemote, setCreatingRemote] = useState<RemoteHost | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [introSound, setIntroSound] = useState(introSoundEnabled);
  const [view, setView] = useState<"servers" | "server" | "companions" | "remote" | "appearance" | "supporter" | "help" | "feedback" | "about">("servers");
  const [level, setLevelState] = useState<UiLevel>(loadLevel);
  const [showWelcome, setShowWelcome] = useState(() => welcome && !welcomeSeen());
  const [tour, setTour] = useState<Tour | null>(null);
  const setLevel = (l: UiLevel) => {
    setLevelState(l);
    saveLevel(l);
  };
  const [supporter, setSupporter] = useState<SupporterInfo | null>(null);
  const [prefs, setPrefs] = useState<AppearancePrefs>(loadPrefs);
  const [celebrate, setCelebrate] = useState<string | null>(null);
  const lastStates = useRef<Map<string, string> | null>(null);
  const look = effective(prefs, !!supporter?.active);
  const [remote, setRemote] = useState<{ host: RemoteHost; id: InstanceId } | null>(null);
  const [side, setSide] = useState<"left" | "right">(() => {
    try {
      return localStorage.getItem(SIDE_KEY) === "left" ? "left" : "right";
    } catch {
      return "right";
    }
  });
  const firstList = useRef(true);
  const [alerts, setAlerts] = useState<AlertList | null>(null);
  const [alertsOpen, setAlertsOpen] = useState(false);
  const notifiedUntil = useRef<number | null>(null);

  const loadAlerts = useCallback(async () => {
    let l: AlertList;
    try {
      l = await sup.alerts();
    } catch {
      return;
    }
    setAlerts(l);
    if (notifiedUntil.current === null) notifiedUntil.current = readSeen();
    const fresh = l.items.filter((a) => !a.read && a.at_unix_ms > (notifiedUntil.current ?? 0));
    if (fresh.length === 0) return;
    notifiedUntil.current = Math.max(...fresh.map((a) => a.at_unix_ms));
    try {
      localStorage.setItem(SEEN_KEY, String(notifiedUntil.current));
    } catch {
      /* sin almacenamiento */
    }
    if (!l.settings.notify) return;
    for (const a of fresh.slice(0, 3).reverse()) {
      await api.notify(t(a.title), a.server_name ? `${a.server_name}: ${t(a.message)}` : t(a.message));
    }
  }, [api, sup]);

  const open = useCallback((id: InstanceId) => {
    setSelected(id);
    setView("server");
  }, []);

  const connect = useCallback(async () => {
    setConnectError(null);
    try {
      setInfo(await api.info());
      sup.systemInfo().then(setSystem, () => setSystem(null));
    } catch (e) {
      setConnectError(errorMessage(e));
    }
  }, [api, sup]);

  const refresh = useCallback(async () => {
    try {
      const list = await sup.list();
      setServers(list);
      setConnectError(null);
      setSelected((cur) => (cur && list.some((s) => s.instance.id === cur) ? cur : null));
      // Al abrir la app con un solo servidor se entra directamente en él.
      if (firstList.current) {
        firstList.current = false;
        if (list.length === 1) {
          setSelected(list[0].instance.id);
          setView("server");
        }
      }
    } catch (e) {
      setConnectError(errorMessage(e));
    }
  }, [sup]);

  useEffect(() => {
    connect();
  }, [connect]);

  useEffect(() => {
    if (!info) return;
    refresh();
    const t = setInterval(refresh, LIST_POLL_MS);
    return () => clearInterval(t);
  }, [info, refresh]);

  useEffect(() => {
    applyTheme(look.theme);
  }, [look.theme]);

  // Icono alternativo de la ventana (Supporter). Al volver al oficial se restaura una sola vez.
  const iconSet = useRef(false);
  useEffect(() => {
    if (look.icon === "oficial") {
      if (iconSet.current) api.setWindowIcon(null).catch(() => undefined);
      iconSet.current = false;
      return;
    }
    iconSet.current = true;
    api.setWindowIcon(iconSvg(look.icon)).catch(() => undefined);
  }, [api, look.icon]);

  // Modo claro u oscuro; con «Igual que Windows» cambia en cuanto cambia Windows.
  useEffect(() => {
    applyMode(isLight(look.mode));
    if (look.mode !== "sistema") return;
    const mq = window.matchMedia?.("(prefers-color-scheme: light)");
    const on = () => applyMode(isLight("sistema"));
    mq?.addEventListener?.("change", on);
    return () => mq?.removeEventListener?.("change", on);
  }, [look.mode]);

  // Supporter: se lee lo guardado y, si hace tiempo que no se comprueba, se revisa en segundo plano.
  useEffect(() => {
    if (!info) return;
    sup.supporter().then(
      (s) => {
        setSupporter(s);
        if (s.stale) sup.refreshSupporter().then(setSupporter, () => undefined);
      },
      () => undefined,
    );
  }, [info, sup]);

  // Celebración al terminar de encender un servidor (Supporter, desactivable).
  useEffect(() => {
    if (!servers) return;
    const now = new Map(servers.map((s) => [s.instance.id, s.status.state as string]));
    const before = lastStates.current;
    lastStates.current = now;
    if (!before || !look.celebrations || reducedMotion()) return;
    const up = servers.find((s) => s.status.state === "ONLINE" && before.has(s.instance.id) && before.get(s.instance.id) !== "ONLINE");
    if (up) setCelebrate(`¡${up.instance.name} está en línea!`);
  }, [servers, look.celebrations]);

  useEffect(() => {
    if (!info) return;
    loadAlerts();
    const t = setInterval(loadAlerts, alertsPollMs);
    return () => clearInterval(t);
  }, [info, loadAlerts, alertsPollMs]);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 9000);
    return () => clearTimeout(t);
  }, [toast]);

  // Los tutoriales llevan a la pantalla de cada paso (en un servidor: el abierto o el primero).
  const tourNav = useCallback(
    (n: TourNav) => {
      if ("view" in n) {
        setView(n.view);
        return;
      }
      const id = selected ?? servers?.[0]?.instance.id;
      if (!id) return;
      setSelected(id);
      setView("server");
      setTimeout(() => window.dispatchEvent(new CustomEvent("merc:tab", { detail: n.tab })), 60);
    },
    [selected, servers],
  );

  const current = servers?.find((s) => s.instance.id === selected) ?? null;
  const online = servers?.filter((s) => s.status.state === "ONLINE").length ?? 0;
  const common: SidebarCommon = {
    brandTone: online > 0 ? "ok" : "brand",
    brandSub:
      servers === null
        ? "Cargando…"
        : online > 0
          ? `${online} en línea de ${servers.length}`
          : `${servers.length} servidor${servers.length === 1 ? "" : "es"}`,
    introSound,
    onToggleSound: () => {
      setIntroSoundEnabled(!introSound);
      setIntroSound(!introSound);
    },
    side,
    onToggleSide: () => {
      const next = side === "left" ? "right" : "left";
      setSide(next);
      try {
        localStorage.setItem(SIDE_KEY, next);
      } catch {
        /* sin almacenamiento: solo dura esta sesión */
      }
    },
    alertsUnread: alerts?.unread ?? 0,
    onOpenAlerts: () => setAlertsOpen(true),
    onHelp: () => setView("help"),
    appUpdate: appUpdate?.available ? appUpdate.version : null,
    badge: look.badge,
    supervisorOk: !connectError,
    supervisorText: connectError ? `Sin conexión con el supervisor: ${connectError}` : `Supervisor ${info?.server_version ?? ""} en marcha`,
  };

  return (
    <UiLevelContext.Provider value={level}>
      <div className="backdrop" data-bg={look.background} aria-hidden="true" />
      {look.background === "particulas" && <Particles />}
      {celebrate && <Celebration text={celebrate} onDone={() => setCelebrate(null)} />}
      {intro && <Splash ready={!!info && servers !== null} error={info ? null : connectError} />}

      {!info ? (
        <div className="empty empty-center">
          <Logo size={88} tone={connectError ? "bad" : "busy"} />
          {connectError ? (
            <>
              <h2>No se pudo conectar con el supervisor</h2>
              <p>{connectError}</p>
              <button type="button" className="btn btn-lg btn-go" onClick={connect}>
                Reintentar
              </button>
            </>
          ) : (
            <>
              <h2>Iniciando MercHosting</h2>
              <p role="status">Conectando con el supervisor que mantiene tus servidores en marcha…</p>
            </>
          )}
        </div>
      ) : (
        <div className="shell" data-side={side}>
          {view === "remote" && remote ? (
            <RemoteServerView
              key={`${remote.host.host}-${remote.id}`}
              api={api}
              host={remote.host}
              id={remote.id}
              sidebar={common}
              onBack={() => setView("companions")}
              onError={setToast}
            />
          ) : current && view === "server" ? (
            <ServerView
              key={current.instance.id}
              api={api}
              summary={current}
              system={system}
              alerts={alerts?.items ?? []}
              sidebar={common}
              onServers={() => setView("servers")}
              onChanged={refresh}
              onRemoved={() => {
                setSelected(null);
                setView("servers");
              }}
              onError={setToast}
              onOpenCompanions={() => setView("companions")}
              onOpenServer={(id) => {
                refresh();
                open(id);
              }}
            />
          ) : (
            <>
              <Sidebar
                {...common}
                current={null}
                serversActive={view === "servers" || view === "server"}
                onServers={() => setView("servers")}
                groups={[
                  {
                    title: "Compartir",
                    items: [{ id: "companions", label: "Compañeros", icon: <Handshake size={18} aria-hidden="true" /> }],
                  },
                  {
                    title: "MercHosting",
                    items: [
                      { id: "appearance", label: "Apariencia", icon: <Palette size={18} aria-hidden="true" /> },
                      { id: "supporter", label: "Apoyar MercHosting", icon: <Heart size={18} aria-hidden="true" /> },
                      { id: "help", label: "Ayuda y tutoriales", icon: <CircleHelp size={18} aria-hidden="true" /> },
                      ...(validForm(feedbackForm)
                        ? [{ id: "feedback", label: "Comentarios", icon: <MessageSquare size={18} aria-hidden="true" /> }]
                        : []),
                      { id: "about", label: "Acerca de", icon: <Info size={18} aria-hidden="true" /> },
                    ],
                  },
                ]}
                active={
                  view === "companions" || view === "appearance" || view === "supporter" || view === "help" || view === "feedback" || view === "about"
                    ? view
                    : null
                }
                onSelect={(id) => setView(id as "companions" | "appearance" | "supporter" | "help" | "feedback" | "about")}
                extra={
                  <div className="nav-actions">
                    <button type="button" className="btn btn-import" data-tour="crear-servidor" onClick={() => setCreating(true)}>
                      <Plus size={17} aria-hidden="true" /> Crear servidor
                    </button>
                    <button type="button" className="btn btn-ghost" data-tour="importar-servidor" onClick={() => setImporting(true)}>
                      <FolderInput size={16} aria-hidden="true" /> Importar servidor
                    </button>
                  </div>
                }
              />
              <main className="main">
                {view === "about" ? (
                  <AboutPage
                    api={api}
                    version={info?.server_version ?? null}
                    onOpenSupporter={() => setView("supporter")}
                    onOpenFeedback={validForm(feedbackForm) ? () => setView("feedback") : undefined}
                  />
                ) : view === "feedback" && validForm(feedbackForm) ? (
                  <FeedbackPage api={api} form={feedbackForm} version={info?.server_version ?? null} />
                ) : view === "help" ? (
                  <HelpPage
                    level={level}
                    hasServer={(servers?.length ?? 0) > 0}
                    onLevel={setLevel}
                    onStartTour={(id) => setTour(tourById(id) ?? null)}
                    onWelcome={() => setShowWelcome(true)}
                    api={api}
                    sup={sup}
                    appUpdate={appUpdate}
                  />
                ) : view === "appearance" ? (
                  <AppearancePage
                    prefs={prefs}
                    supporter={!!supporter?.active}
                    onChange={(p) => {
                      setPrefs(p);
                      savePrefs(p);
                    }}
                    onOpenSupporter={() => setView("supporter")}
                  />
                ) : view === "supporter" ? (
                  <SupporterPage api={api} sup={sup} onChanged={setSupporter} />
                ) : view === "companions" ? (
                  <CompanionsPage
                    sup={sup}
                    servers={servers ?? []}
                    onOpenRemote={(host, id) => {
                      setRemote({ host, id });
                      setView("remote");
                    }}
                    onCreateRemote={setCreatingRemote}
                  />
                ) : (
                <ServersHome
                  servers={servers}
                  onOpen={open}
                  onCreate={() => setCreating(true)}
                  onImport={() => setImporting(true)}
                />
                )}
              </main>
            </>
          )}

          {importing && (
            <ImportDialog
              api={api}
              onClose={() => setImporting(false)}
              onImported={(id) => {
                setImporting(false);
                open(id);
                refresh();
              }}
            />
          )}

          {creating && (
            <CreateWizard
              api={api}
              system={system}
              usedPorts={(servers ?? []).flatMap((s) => (s.configured_port ? [s.configured_port] : []))}
              onClose={() => setCreating(false)}
              onCreated={(id) => {
                setCreating(false);
                open(id);
                refresh();
              }}
            />
          )}

          {creatingRemote && (
            <CreateWizard
              api={remoteApi(api, creatingRemote.host)}
              system={null}
              usedPorts={[]}
              remote={{ hostName: creatingRemote.name || "tu servidor casero" }}
              onClose={() => setCreatingRemote(null)}
              onCreated={async (id) => {
                const h = creatingRemote;
                setCreatingRemote(null);
                // Lista de servidores de esa PC al día (ya incluye el nuevo).
                const hosts = await sup.refreshHost(h.host).catch(() => null);
                setRemote({ host: hosts?.find((x) => x.host === h.host) ?? h, id });
                setView("remote");
              }}
            />
          )}

          {alertsOpen && alerts && (
            <AlertsDialog sup={sup} list={alerts} onChange={setAlerts} onClose={() => setAlertsOpen(false)} />
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
      )}
      {showWelcome && info && (
        <WelcomeDialog
          system={system}
          level={level}
          onLevel={setLevel}
          onClose={(startTour) => {
            markWelcomeSeen();
            setShowWelcome(false);
            if (startTour) setTour(tourById("primeros-pasos") ?? null);
          }}
        />
      )}
      {tour && <TourOverlay key={tour.id} tour={tour} onNav={tourNav} onClose={() => setTour(null)} />}
    </UiLevelContext.Provider>
  );
}
