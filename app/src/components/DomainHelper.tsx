// «Usar mi dominio»: arma el registro SRV que la persona tiene que crear en el panel DNS de su
// dominio para que sus amigos entren con un nombre propio (play.midominio.com) que lleva al
// túnel de Playit. MercHosting no toca el DNS de nadie: solo da los valores exactos y luego
// prueba el dominio de verdad, igual que lo haría el juego.

import { CheckCircle2, Copy, Globe, TriangleAlert } from "lucide-react";
import { useState } from "react";
import type { AddressTest } from "../api/types";
import { customDomain, setCustomDomain } from "../domainPref";

interface Props {
  instanceId: string;
  /** Dirección pública de Playit (la que se muestra a los amigos). */
  tunnel: string;
  /** Última prueba de esa dirección: de ahí sale el host y el puerto reales. */
  tunnelTest: AddressTest | undefined;
  /** Última prueba del dominio, si se hizo. */
  domainTest: (domain: string) => AddressTest | undefined;
  online: boolean;
  busy: boolean;
  onTest: (domain: string) => void;
  onCopy: (text: string) => void;
  copied: string | null;
}

/** Nombre de dominio con al menos un punto y sin puerto, esquema ni ruta. */
export function validDomain(d: string): boolean {
  const s = d.trim().toLowerCase();
  if (s.length < 4 || s.length > 253 || !s.includes(".")) return false;
  return s.split(".").every((l) => /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/.test(l));
}

/** Quita lo que suele pegarse de más: «https://», una ruta o un puerto. */
export function cleanDomain(d: string): string {
  return d
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//, "")
    .replace(/[/?#].*$/, "")
    .replace(/:\d+$/, "")
    .replace(/\.$/, "");
}

/** Host y puerto reales del túnel: de la prueba (tras resolver su SRV) o de la dirección si lleva puerto. */
export function srvTarget(tunnel: string, test: AddressTest | undefined): { host: string; port: number } | null {
  const from = test?.result?.tested ?? (/:\d+$/.test(tunnel) ? tunnel : null);
  if (!from) return null;
  const i = from.lastIndexOf(":");
  const port = Number(from.slice(i + 1));
  const host = from.slice(0, i);
  if (!host || !Number.isInteger(port) || port <= 0 || port > 65535) return null;
  return { host, port };
}

const isIp = (h: string) => /^\d{1,3}(\.\d{1,3}){3}$/.test(h) || h.includes(":");

export function DomainHelper({ instanceId, tunnel, tunnelTest, domainTest, online, busy, onTest, onCopy, copied }: Props) {
  const [open, setOpen] = useState(() => customDomain(instanceId) !== null);
  const [value, setValue] = useState(() => customDomain(instanceId) ?? "");
  const domain = cleanDomain(value);
  const valid = validDomain(domain);
  const target = srvTarget(tunnel, tunnelTest);
  const name = valid ? `_minecraft._tcp.${domain}` : "";
  const t = valid ? domainTest(domain) : undefined;
  // Si el dominio responde y lleva al mismo destino que el túnel, quedó bien hecho.
  const sameTarget = !!t?.result && !!target && t.result.tested === `${target.host}:${target.port}`;

  function save(d: string) {
    setValue(d);
    const c = cleanDomain(d);
    setCustomDomain(instanceId, validDomain(c) ? c : null);
  }

  if (!open) {
    return (
      <button type="button" className="btn btn-sm btn-quiet" onClick={() => setOpen(true)}>
        <Globe size={14} aria-hidden="true" /> Usar mi dominio
      </button>
    );
  }

  const rows: [string, string][] = target
    ? [
        ["Tipo", "SRV"],
        ["Nombre", name || "_minecraft._tcp.<tu dominio>"],
        ["Prioridad", "0"],
        ["Peso", "5"],
        ["Puerto", String(target.port)],
        ["Destino", target.host],
      ]
    : [];

  return (
    <div className="domain-box" aria-label="Usar mi dominio">
      <h3>
        <Globe size={15} aria-hidden="true" /> Usar mi dominio
      </h3>
      <p className="field-help">
        Si tienes un dominio propio, tus amigos pueden entrar con él en vez de la dirección de Playit. Tu IP sigue sin mostrarse:
        el dominio solo apunta al túnel.
      </p>
      <label className="field">
        <span>Tu dominio o subdominio</span>
        <input
          type="text"
          value={value}
          placeholder="play.midominio.com"
          spellCheck={false}
          autoComplete="off"
          onChange={(e) => save(e.target.value)}
          translate="no"
        />
      </label>
      {value.trim() !== "" && !valid && <p className="warn-text">Escribe solo el nombre, por ejemplo play.midominio.com.</p>}

      {!target ? (
        <p className="field-help">Primero prueba la dirección de Playit de arriba: así se sabe a qué servidor y puerto debe apuntar tu dominio.</p>
      ) : (
        <>
          <p>
            En el panel DNS de donde compraste el dominio, crea este registro:
          </p>
          <table className="srv-table">
            <tbody>
              {rows.map(([k, v]) => (
                <tr key={k}>
                  <th scope="row">{k}</th>
                  <td>
                    <code translate="no">{v}</code>
                  </td>
                  <td>
                    {k !== "Tipo" && (k !== "Nombre" || valid) && (
                      <button type="button" className="btn btn-xs" onClick={() => onCopy(v)} aria-label={`Copiar ${k}: ${v}`}>
                        <Copy size={12} aria-hidden="true" /> {copied === v ? "Copiado" : "Copiar"}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {isIp(target.host) && (
            <p className="warn-text">
              <TriangleAlert size={13} aria-hidden="true" /> El destino de un SRV tiene que ser un nombre, no una IP. Usa la
              dirección con nombre que te da Playit.
            </p>
          )}
          <p className="field-help">
            En muchos paneles el nombre se escribe sin tu dominio al final (por ejemplo <code translate="no">_minecraft._tcp.play</code>
            ). Los cambios de DNS pueden tardar desde minutos hasta horas en verse.
          </p>
          <div className="address-row">
            <button
              type="button"
              className="btn btn-sm btn-go"
              disabled={!valid || !online || busy}
              onClick={() => onTest(domain)}
            >
              {busy ? "Probando…" : "Probar mi dominio"}
            </button>
            {!online && <span className="field-help">Enciende el servidor para probarlo.</span>}
          </div>
          {t &&
            (t.result ? (
              sameTarget ? (
                <p className="ok-text">
                  <CheckCircle2 size={13} aria-hidden="true" /> Listo: <strong translate="no">{domain}</strong> llega a tu
                  servidor en {t.result.latency_ms} ms. Ya puedes compartirlo.
                </p>
              ) : (
                <p className="warn-text">
                  <TriangleAlert size={13} aria-hidden="true" /> Tu dominio responde, pero lleva a{" "}
                  <code translate="no">{t.result.tested}</code> en vez de a tu túnel. Revisa el registro.
                </p>
              )
            ) : (
              <p className="warn-text">
                <TriangleAlert size={13} aria-hidden="true" /> Todavía no responde: {t.error}. Si acabas de crear el registro, espera
                un rato y vuelve a probar.
              </p>
            ))}
        </>
      )}
    </div>
  );
}
