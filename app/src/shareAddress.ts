// La dirección que la persona comparte con sus amigos para entrar por Internet, para mostrarla
// en el Resumen. Solo se ofrece para copiar si se probó de verdad y respondió; si no, se dice
// que falta probarla. Si hay un dominio propio guardado y su prueba responde, va ese.

import { useEffect, useState } from "react";
import type { Supervisor } from "./api/client";
import type { ConnectionInfo } from "./api/types";
import { customDomain } from "./domainPref";

export interface ShareAddress {
  address: string;
  /** Respondió en la última prueba real. */
  tested: boolean;
  /** Es el dominio propio de la persona (no la dirección de Playit). */
  domain: boolean;
}

export function pickShareAddress(conn: ConnectionInfo | null, domain: string | null): ShareAddress | null {
  const tunnel = conn?.tunnels.find((t) => !t.is_disabled);
  if (!conn || !tunnel) return null;
  const ok = (a: string) => !!conn.tests.find((t) => t.address === a)?.result;
  if (domain && ok(domain)) return { address: domain, tested: true, domain: true };
  return { address: tunnel.display_address, tested: ok(tunnel.display_address), domain: false };
}

export function useShareAddress(sup: Supervisor, id: string, enabled: boolean): ShareAddress | null {
  const [conn, setConn] = useState<ConnectionInfo | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    const load = () =>
      sup
        .connection(id)
        .then((c) => alive && setConn(c))
        .catch(() => {
          /* sin datos de conexión: el Resumen no muestra la dirección */
        });
    load();
    const t = setInterval(load, 8000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [sup, id, enabled]);
  return enabled ? pickShareAddress(conn, customDomain(id)) : null;
}
