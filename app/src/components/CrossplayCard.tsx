// Jugadores de Bedrock (celular, consola, Windows) en un servidor Java, con Geyser y Floodgate
// de GeyserMC (código abierto, MIT). Se instalan desde Modrinth con el mismo diálogo de siempre
// (ver la versión, las dependencias y la licencia antes de instalar). La conexión se prueba de
// verdad con el ping de Bedrock por UDP, igual que hace el juego.

import { CheckCircle2, Copy, ExternalLink, Gamepad2, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { AddressTest, BedrockInfo, ContentList, InstanceId, Platform } from "../api/types";
import { ProjectDialog } from "./ContentPanel";

/** Plataformas donde GeyserMC publica Geyser y Floodgate. */
const GEYSER_LOADERS: Record<string, string[]> = {
  plugin: ["paper", "spigot"],
  mod: ["fabric", "neoforge"],
};

export function geyserPlatform(platforms: Platform[]): Platform | null {
  return (
    platforms.find((p) => (GEYSER_LOADERS[p.kind] ?? []).some((l) => p.loaders.includes(l)) && p.sources.includes("modrinth")) ?? null
  );
}

interface Props {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  bedrock: BedrockInfo | null;
  online: boolean;
  running: boolean;
  remote: boolean;
  /** Recarga la información de conexión. */
  onChanged: () => void;
  onOpenPlayit: () => void;
}

function BedrockTest({ t }: { t: AddressTest | undefined }) {
  if (!t) return <span className="badge badge-muted">Sin probar</span>;
  if (t.result)
    return (
      <span className="ok-text">
        <CheckCircle2 size={13} aria-hidden="true" /> Funciona: responde en {t.result.latency_ms} ms · {t.result.version} ·{" "}
        {t.result.players_online}/{t.result.players_max} jugadores
      </span>
    );
  return (
    <span className="warn-text">
      <TriangleAlert size={13} aria-hidden="true" /> No responde: {t.error}
    </span>
  );
}

export function CrossplayCard({ api, sup, id, bedrock, online, running, remote, onChanged, onOpenPlayit }: Props) {
  const [platforms, setPlatforms] = useState<Platform[] | null>(null);
  const [picking, setPicking] = useState<"geyser" | "floodgate" | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    sup.contentPlatforms(id).then(
      (p) => alive && setPlatforms(p),
      () => alive && setPlatforms([]),
    );
    return () => {
      alive = false;
    };
  }, [sup, id]);

  const platform = platforms ? geyserPlatform(platforms) : null;
  const testOf = (a: string) => bedrock?.tests.find((t) => t.address === a);

  async function test(address: string) {
    setBusy(address);
    setError(null);
    try {
      await sup.testBedrockAddress(id, address);
      onChanged();
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

  function installed(_l: ContentList, msg: string) {
    setPicking(null);
    setNote(msg);
    onChanged();
  }

  const canInstall = !remote && !!platform;

  return (
    <section className="card props-card" aria-label="Jugadores de Bedrock">
      <h2>
        <Gamepad2 size={16} aria-hidden="true" /> Jugadores de Bedrock (celular, consola, Windows)
      </h2>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {note && <p className="ok-text">{note}</p>}

      {!bedrock ? (
        <>
          <p>
            Con <strong>Geyser</strong> los jugadores de Minecraft Bedrock entran a este servidor de Java, y con{" "}
            <strong>Floodgate</strong> pueden hacerlo sin tener una cuenta de Java. Los dos son de GeyserMC, gratis y de código
            abierto.
          </p>
          {platforms === null ? (
            <p className="wiz-loading">Comprobando el tipo de servidor…</p>
          ) : !platform ? (
            <p className="field-help">
              Geyser funciona en servidores Paper, Purpur, Spigot, Fabric o NeoForge. Este tipo de servidor no es compatible.
            </p>
          ) : remote ? (
            <p className="field-help">El dueño del servidor puede instalarlo desde su PC.</p>
          ) : (
            <>
              <button type="button" className="btn btn-go" disabled={running} onClick={() => setPicking("geyser")}>
                Instalar Geyser
              </button>
              {running && <p className="field-help">Detén el servidor para instalarlo.</p>}
              <p className="field-help">Verás la versión, lo que descarga y la licencia antes de instalar.</p>
            </>
          )}
        </>
      ) : (
        <>
          <ul className="plain-list">
            <li>
              Geyser: <code translate="no">{bedrock.geyser}</code>
              {!bedrock.geyser_enabled && <span className="warn-text"> · desactivado</span>}
            </li>
            <li>
              Floodgate:{" "}
              {bedrock.floodgate ? (
                <>
                  <code translate="no">{bedrock.floodgate}</code>
                  {!bedrock.floodgate_enabled && <span className="warn-text"> · desactivado</span>}
                </>
              ) : (
                <>
                  no instalado: los jugadores de Bedrock necesitarán una cuenta de Java.{" "}
                  {canInstall && (
                    <button
                      type="button"
                      className="btn btn-xs"
                      disabled={running}
                      title={running ? "Detén el servidor para instalarlo." : undefined}
                      onClick={() => setPicking("floodgate")}
                    >
                      Instalar Floodgate
                    </button>
                  )}
                </>
              )}
            </li>
            <li>
              Puerto de Bedrock (UDP): <strong>{bedrock.port}</strong>
              {!bedrock.port_from_config && <span className="field-help"> · el de fábrica; se confirma al encender por primera vez</span>}
            </li>
          </ul>

          {bedrock.lan.map((a) => (
            <div key={a} className="tunnel-box">
              <p className="field-help">En tu misma red (Bedrock)</p>
              <div className="address-row">
                <code className="address">{a}</code>
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={!online || busy !== null}
                  onClick={() => test(a)}
                  aria-label={`Probar Bedrock ${a}`}
                >
                  {busy === a ? "Probando…" : "Probar"}
                </button>
              </div>
              <p>
                <BedrockTest t={testOf(a)} />
              </p>
            </div>
          ))}

          {bedrock.tunnels.length > 0 ? (
            bedrock.tunnels.map((t) => {
              const tested = testOf(t.display_address);
              const ok = !!tested?.result;
              return (
                <div key={t.display_address} className="tunnel-box">
                  <p className="field-help">Dirección para tus amigos de Bedrock (servidor y puerto)</p>
                  <div className="address-row">
                    <code className="address address-big">{t.display_address}</code>
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={!ok}
                      title={ok ? undefined : "Pruébala antes de compartirla"}
                      onClick={() => copy(t.display_address)}
                      aria-label={`Copiar Bedrock ${t.display_address}`}
                    >
                      <Copy size={14} aria-hidden="true" /> {copied === t.display_address ? "Copiada" : "Copiar"}
                    </button>
                    <button
                      type="button"
                      className="btn btn-sm btn-go"
                      disabled={!online || busy !== null || t.is_disabled}
                      onClick={() => test(t.display_address)}
                      aria-label={`Probar Bedrock ${t.display_address}`}
                    >
                      {busy === t.display_address ? "Probando…" : "Probar conexión"}
                    </button>
                  </div>
                  <p>
                    <BedrockTest t={tested} />
                  </p>
                </div>
              );
            })
          ) : remote ? (
            <p className="field-help">Para jugar por Internet, el dueño tiene que crear un túnel de Bedrock en Playit.</p>
          ) : (
            <div className="claim-box crossplay-guide" aria-label="Túnel de Bedrock">
              <p>Para que entren por Internet, crea un segundo túnel en Playit:</p>
              <ol>
                <li>Abre tu cuenta de playit.gg y elige «Create Tunnel».</li>
                <li>
                  Tipo: <strong>Minecraft Bedrock</strong>. Puerto local: <strong>{bedrock.port}</strong>.
                </li>
                <li>Vuelve aquí: el túnel aparece solo y lo puedes probar.</li>
              </ol>
              <button type="button" className="btn btn-sm" onClick={onOpenPlayit}>
                <ExternalLink size={14} aria-hidden="true" /> Abrir mi cuenta de playit.gg
              </button>
            </div>
          )}
          {!online && <p className="field-help">Enciende el servidor para probar las direcciones: Geyser arranca con él.</p>}
          <p className="field-help">
            En Bedrock se agrega como servidor escribiendo la dirección y el puerto por separado. Si por Internet no logran entrar,
            prueben primero en tu misma red: hay reportes de túneles UDP de Playit que fallan con Bedrock.
          </p>
        </>
      )}

      {picking && platform && (
        <ProjectDialog
          api={api}
          sup={sup}
          id={id}
          platform={platform}
          running={running}
          picked={{ project: picking, source: "modrinth" }}
          onClose={() => setPicking(null)}
          onInstalled={installed}
        />
      )}
    </section>
  );
}
