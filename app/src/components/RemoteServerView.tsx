// Un servidor de un amigo, abierto desde esta PC. Es la misma vista que la de un servidor propio,
// pero cada petición viaja a la PC del amigo (que decide si está permitida) y solo se muestran
// las secciones que permiten tus permisos.

import { ArrowLeft, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MercApi } from "../api/client";
import { Supervisor, errorMessage, remoteApi } from "../api/client";
import type { InstanceId, InstanceSummary, RemoteHost } from "../api/types";
import { Logo } from "./Logo";
import { ServerView } from "./ServerView";
import { Sidebar, type SidebarCommon } from "./Sidebar";

const POLL_MS = 2000;

interface Props {
  api: MercApi;
  host: RemoteHost;
  id: InstanceId;
  sidebar: SidebarCommon;
  onBack: () => void;
  onError: (message: string) => void;
}

export function RemoteServerView({ api, host, id, sidebar, onBack, onError }: Props) {
  const local = useMemo(() => new Supervisor(api), [api]);
  const rApi = useMemo(() => remoteApi(api, host.host), [api, host.host]);
  const rSup = useMemo(() => new Supervisor(rApi), [rApi]);
  const [summary, setSummary] = useState<InstanceSummary | null>(null);
  const [perms, setPerms] = useState(host.servers.find((s) => s.id === id)?.perms ?? []);
  const [error, setError] = useState<string | null>(null);

  const inFlight = useRef(false);
  const load = useCallback(async () => {
    // Si la PC del amigo tarda, no se acumulan consultas.
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      const list = await rSup.list();
      const s = list.find((x) => x.instance.id === id) ?? null;
      setSummary(s);
      setError(s ? null : "Ya no tienes acceso a este servidor.");
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      inFlight.current = false;
    }
  }, [rSup, id]);

  useEffect(() => {
    // Permisos actuales según la PC del amigo.
    local.refreshHost(host.host).then(
      (hosts) => {
        const s = hosts.find((h) => h.host === host.host)?.servers.find((x) => x.id === id);
        if (s) setPerms(s.perms);
      },
      () => undefined,
    );
  }, [local, host.host, id]);

  useEffect(() => {
    load();
    const t = setInterval(load, POLL_MS);
    return () => clearInterval(t);
  }, [load]);

  if (summary && !error)
    return (
      <ServerView
        key={`${host.host}-${id}`}
        api={rApi}
        summary={summary}
        system={null}
        sidebar={sidebar}
        onServers={onBack}
        onChanged={load}
        onRemoved={onBack}
        onError={onError}
        remote={{ perms, hostName: host.name || "tu amigo", hostAdmin: host.host_admin }}
      />
    );

  return (
    <>
      <Sidebar {...sidebar} current={null} serversActive={false} onServers={onBack} groups={[]} active={null} onSelect={() => undefined} />
      <main className="main">
        <div className="empty empty-center">
          <Logo size={72} tone={error ? "bad" : "busy"} />
          {error ? (
            <>
              <h2>No se pudo abrir el servidor de {host.name || "tu amigo"}</h2>
              <p role="alert">{error}</p>
              <div className="notice-actions">
                <button type="button" className="btn btn-go" onClick={load}>
                  <RefreshCw size={15} aria-hidden="true" /> Reintentar
                </button>
                <button type="button" className="btn btn-quiet" onClick={onBack}>
                  <ArrowLeft size={15} aria-hidden="true" /> Volver
                </button>
              </div>
            </>
          ) : (
            <>
              <h2>Conectando con la PC de {host.name || "tu amigo"}…</h2>
              <p role="status">La primera vez puede tardar unos segundos mientras se abre el camino entre las dos PCs.</p>
            </>
          )}
        </div>
      </main>
    </>
  );
}
