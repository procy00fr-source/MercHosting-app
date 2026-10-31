// Pestaña Invitar jugadores: un mensaje listo para mandar con la dirección (probada), la
// versión, el loader y los mods que los jugadores necesitan en su juego (con su página
// oficial). Invitar a jugar NO da acceso a administrar el servidor.

import { Check, Copy, ExternalLink, PackageOpen, Send, UserPlus } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { ConnectionInfo, FriendPackPlan, InstalledItem, Instance, Perm, Platform } from "../api/types";
import { KIND_LABEL } from "../format";

/** Servidores en los que el jugador necesita instalar el loader en su juego. */
const CLIENT_LOADER: Partial<Record<Instance["kind"], string>> = {
  fabric: "Fabric Loader",
  quilt: "Quilt Loader",
  forge: "Forge",
  neo_forge: "NeoForge",
  mohist: "Forge",
};

interface Props {
  api: MercApi;
  sup: Supervisor;
  inst: Instance;
  platforms: Platform[];
  can?: (p: Perm) => boolean;
  /** Desde la app de un compañero no se guardan archivos en la PC del dueño. */
  remote?: boolean;
}

type Address = { value: string; kind: "public" | "untested" | "lan" };

export function InvitePanel({ api, sup, inst, platforms, can = () => true, remote = false }: Props) {
  const [conn, setConn] = useState<ConnectionInfo | null>(null);
  const [mods, setMods] = useState<InstalledItem[] | null>(null);
  const [modsNote, setModsNote] = useState<string | null>(null);
  const [whitelist, setWhitelist] = useState<boolean | null>(null);
  const [address, setAddress] = useState<string | null>(null);
  const [player, setPlayer] = useState("");
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const hasMods = platforms.some((p) => p.kind === "mod");

  useEffect(() => {
    let alive = true;
    if (can("network")) sup.connection(inst.id).then((c) => alive && setConn(c), (e) => alive && setError(errorMessage(e)));
    if (hasMods)
      sup.contentInstalled(inst.id, "mod").then(
        (l) => {
          if (!alive) return;
          setMods(l.items.filter((i) => i.enabled));
          if (!l.online) setModsNote(l.offline_reason ?? "Sin conexión con Modrinth: no se sabe qué mods necesita el juego.");
        },
        (e) => alive && setModsNote(errorMessage(e)),
      );
    if (can("settings"))
      sup.properties(inst.id).then(
        (p) => alive && setWhitelist(p.entries.find((e) => e.key === "white-list")?.value === "true"),
        () => undefined,
      );
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sup, inst.id, hasMods]);

  const addresses: Address[] = useMemo(() => {
    if (!conn) return [];
    const ok = (a: string) => !!conn.tests.find((t) => t.address === a)?.result;
    const tunnels = conn.tunnels.filter((t) => !t.is_disabled).map((t) => t.display_address);
    return [
      ...tunnels.filter(ok).map((value) => ({ value, kind: "public" as const })),
      ...tunnels.filter((t) => !ok(t)).map((value) => ({ value, kind: "untested" as const })),
      ...conn.lan.map((value) => ({ value, kind: "lan" as const })),
    ];
  }, [conn]);

  useEffect(() => {
    if (address === null && addresses.length > 0) setAddress(addresses[0].value);
  }, [addresses, address]);

  const required = (mods ?? []).filter((m) => m.client_side === "required");
  const optional = (mods ?? []).filter((m) => m.client_side === "optional");
  const unknown = (mods ?? []).filter((m) => !m.client_side || m.client_side === "unknown");
  const voice = conn?.voice?.installed && conn.voice.enabled;
  const loader = CLIENT_LOADER[inst.kind];
  const picked = addresses.find((a) => a.value === address) ?? null;

  const name = (m: InstalledItem) => `${m.title ?? m.file_name}${m.version_number ? ` ${m.version_number}` : ""}`;
  const line = (m: InstalledItem) => `- ${name(m)}${m.page_url ? ` — ${m.page_url}` : ""}`;

  const message = [
    `¡Te invito a mi servidor de Minecraft «${inst.name}»!`,
    picked ? `Dirección: ${picked.value}${picked.kind === "lan" ? " (solo desde mi misma red)" : ""}` : null,
    `Versión: Minecraft Java ${inst.mc_version ?? "?"}${loader ? ` con ${loader}${inst.loader_version ? ` ${inst.loader_version}` : ""}` : ""}`,
    required.length > 0 ? "Mods que necesitas en tu juego:" : null,
    ...required.map(line),
    optional.length > 0 ? "Opcionales (si quieres):" : null,
    ...optional.map(line),
    voice ? "Para hablar por voz: instala Simple Voice Chat en tu juego (opcional)." : null,
    whitelist ? "Pásame tu nombre de Minecraft para añadirte a la lista blanca." : null,
  ]
    .filter((x): x is string => x !== null)
    .join("\n");

  async function copy() {
    try {
      await navigator.clipboard.writeText(message);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setError("No se pudo copiar. Selecciona el texto y cópialo a mano.");
    }
  }

  async function addToWhitelist() {
    setError(null);
    setNote(null);
    try {
      await sup.playerAction(inst.id, "whitelist_add", player.trim());
      setNote(`${player.trim()} añadido a la lista blanca.`);
      setPlayer("");
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <div className="admin-panel invite-panel" role="tabpanel" aria-label="Invitar jugadores">
      <div className="content-head">
        <h2 className="panel-title">
          <Send size={18} aria-hidden="true" /> Invitar jugadores
        </h2>
        <p className="field-help">
          Un mensaje listo para mandar a tus amigos. Solo sirve para jugar: no da acceso a administrar el servidor (eso son los
          compañeros).
        </p>
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      {can("network") && (
        <section className="card props-card" aria-label="Dirección">
          <h2>Dirección</h2>
          {conn === null ? (
            <p className="wiz-loading">Buscando…</p>
          ) : addresses.length === 0 ? (
            <p className="field-help">No hay ninguna dirección. Prepárala en la pestaña Conexión.</p>
          ) : (
            <fieldset className="java-list">
              <legend className="sr-only">Dirección</legend>
              {addresses.map((a) => (
                <label key={a.value} className="check">
                  <input type="radio" name="addr" checked={address === a.value} onChange={() => setAddress(a.value)} />
                  <span>
                    <code>{a.value}</code>{" "}
                    <span className="field-help">
                      {a.kind === "public"
                        ? "· por Internet, probada"
                        : a.kind === "untested"
                          ? "· por Internet, sin probar: pruébala en Conexión antes de mandarla"
                          : "· solo para quien está en tu misma red"}
                    </span>
                  </span>
                </label>
              ))}
            </fieldset>
          )}
        </section>
      )}

      {hasMods && (
        <section className="card props-card" aria-label="Mods para el juego">
          <h2>Mods que necesitan en su juego</h2>
          {mods === null && !modsNote ? (
            <p className="wiz-loading">Revisando tus mods…</p>
          ) : (
            <>
              {loader && (
                <p>
                  Primero, <strong>{loader}</strong> {inst.loader_version ?? ""} para Minecraft {inst.mc_version}.
                </p>
              )}
              {required.length > 0 && (
                <ul className="row-list" aria-label="Mods necesarios">
                  {required.map((m) => (
                    <li key={m.file_name}>
                      <span>{name(m)}</span>
                      {m.page_url && (
                        <button type="button" className="btn btn-sm" onClick={() => api.openUrl(m.page_url!)}>
                          <ExternalLink size={13} aria-hidden="true" /> Página oficial
                        </button>
                      )}
                    </li>
                  ))}
                </ul>
              )}
              {optional.length > 0 && (
                <p className="field-help">Opcionales en el juego: {optional.map(name).join(", ")}.</p>
              )}
              {unknown.length > 0 && (
                <p className="field-help warn-text">
                  Sin datos de si hacen falta en el juego (no están en Modrinth o no hubo conexión): {unknown.map(name).join(", ")}.
                  Revísalos tú antes de mandar la lista.
                </p>
              )}
              {modsNote && <p className="field-help">{modsNote}</p>}
              <p className="field-help">
                Se comparten enlaces a las páginas oficiales, no los archivos: así cada jugador los descarga de su autor.
              </p>
            </>
          )}
        </section>
      )}

      {hasMods && !remote && <FriendPackCard api={api} sup={sup} inst={inst} />}

      {whitelist && can("whitelist") && (
        <section className="card props-card" aria-label="Lista blanca">
          <h2>
            <UserPlus size={16} aria-hidden="true" /> Lista blanca activada
          </h2>
          <p className="field-help">Solo entran los de la lista. Añade el nombre de Minecraft de tu amigo:</p>
          <form
            className="launch-row"
            onSubmit={(e) => {
              e.preventDefault();
              addToWhitelist();
            }}
          >
            <label className="field">
              <span className="field-title">Nombre de Minecraft</span>
              <input type="text" value={player} onChange={(e) => setPlayer(e.target.value)} maxLength={16} />
            </label>
            <button type="submit" className="btn btn-sm btn-go" disabled={!/^[A-Za-z0-9_]{3,16}$/.test(player.trim())}>
              Añadir
            </button>
          </form>
          {note && (
            <p className="ok-text" role="status">
              {note}
            </p>
          )}
        </section>
      )}

      <section className="card props-card" aria-label="Mensaje">
        <h2>Mensaje</h2>
        <pre className="evidence invite-message" aria-label="Mensaje de invitación">
          {message}
        </pre>
        <div className="notice-actions">
          <button type="button" className="btn btn-go" onClick={copy} disabled={picked?.kind === "untested"}>
            {copied ? <Check size={15} aria-hidden="true" /> : <Copy size={15} aria-hidden="true" />} {copied ? "Copiado" : "Copiar mensaje"}
          </button>
          {picked?.kind === "untested" && <span className="field-help">Prueba antes la dirección en Conexión.</span>}
        </div>
        <p className="field-help">Servidor {KIND_LABEL[inst.kind]}: la versión de Minecraft del juego tiene que ser exactamente la misma.</p>
      </section>
    </div>
  );
}

const LOADER_NAME: Record<string, string> = {
  "fabric-loader": "Fabric Loader",
  "quilt-loader": "Quilt Loader",
  forge: "Forge",
  neoforge: "NeoForge",
};

/** Pack para amigos: un .mrpack solo con enlaces oficiales de Modrinth y sus huellas. */
function FriendPackCard({ api, sup, inst }: { api: MercApi; sup: Supervisor; inst: Instance }) {
  const [plan, setPlan] = useState<FriendPackPlan | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  async function prepare() {
    setBusy(true);
    setError(null);
    setSaved(null);
    try {
      setPlan(await sup.friendPack(inst.id));
    } catch (e) {
      setError(errorMessage(e));
    }
    setBusy(false);
  }

  async function save() {
    const dest = await api.pickSavePath(`${inst.name.replace(/[<>:"/\\|?*]/g, "-")} (para jugar).mrpack`);
    if (!dest) return;
    setBusy(true);
    setError(null);
    try {
      setSaved(await sup.exportFriendPack(inst.id, dest));
    } catch (e) {
      setError(errorMessage(e));
    }
    setBusy(false);
  }

  return (
    <section className="card props-card" aria-label="Pack para amigos">
      <h2>
        <PackageOpen size={16} aria-hidden="true" /> Pack para amigos
      </h2>
      <p className="field-help">
        Un archivo .mrpack que tus amigos abren en un launcher que importe modpacks de Modrinth (por ejemplo, Modrinth App o Prism
        Launcher) para tener la versión, el loader y los mods justos. Solo lleva enlaces oficiales de Modrinth con sus huellas: no
        se copia ningún mod dentro.
      </p>
      {!plan && (
        <button type="button" className="btn" onClick={prepare} disabled={busy}>
          {busy ? "Revisando tus mods…" : "Preparar el pack"}
        </button>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {plan && (
        <>
          {plan.mc_version && (
            <p>
              Minecraft {plan.mc_version}
              {plan.loader ? ` con ${LOADER_NAME[plan.loader[0]] ?? plan.loader[0]} ${plan.loader[1]}` : ""}.
            </p>
          )}
          {plan.included.length > 0 && (
            <>
              <h3>Van en el pack ({plan.included.length})</h3>
              <ul className="row-list" aria-label="Mods en el pack">
                {plan.included.map((m) => (
                  <li key={m.file_name}>
                    <span>
                      {m.title}
                      {m.optional ? <span className="field-help"> (opcional)</span> : null}
                    </span>
                  </li>
                ))}
              </ul>
            </>
          )}
          {plan.manual.length > 0 && (
            <>
              <h3>A mano ({plan.manual.length})</h3>
              <p className="field-help warn-text">Estos no pueden ir en el pack. Diles a tus amigos que los bajen de su página.</p>
              <ul className="row-list" aria-label="Mods a mano">
                {plan.manual.map((m) => (
                  <li key={m.file_name}>
                    <span>
                      {m.title}
                      <span className="field-help"> — {m.reason}</span>
                    </span>
                    {m.page_url && (
                      <button type="button" className="btn btn-sm" onClick={() => api.openUrl(m.page_url!)}>
                        <ExternalLink size={13} aria-hidden="true" /> Página oficial
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </>
          )}
          {plan.server_only.length > 0 && (
            <p className="field-help">Solo del servidor (no hacen falta en el juego): {plan.server_only.join(", ")}.</p>
          )}
          {plan.blocked ? (
            <p className="field-help warn-text" role="status">
              {plan.blocked}
            </p>
          ) : (
            <div className="launch-row">
              <button type="button" className="btn btn-go" onClick={save} disabled={busy}>
                Guardar el pack (.mrpack)
              </button>
              <button type="button" className="btn btn-quiet" onClick={prepare} disabled={busy}>
                Revisar de nuevo
              </button>
            </div>
          )}
          {saved && (
            <p className="field-help" role="status">
              Guardado en {saved}. Mándaselo a tus amigos junto con la invitación.
            </p>
          )}
        </>
      )}
    </section>
  );
}
