// Pestaña Jugadores de los servidores Bedrock: lista blanca (allowlist.json) y permisos
// (permissions.json). Los permisos van por XUID (el identificador de Xbox): MercHosting lo anota
// cuando el jugador entra al servidor; no se inventa ni se busca en servicios de terceros.
// Con el servidor encendido el cambio se aplica al momento (el servidor relee el archivo).

import { RefreshCw, ShieldCheck, TriangleAlert, UserPlus, Users } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { BedrockLevel, BedrockPlayersInfo, InstanceId } from "../api/types";
import { uiLocale } from "../i18n/lang";

interface Props {
  sup: Supervisor;
  id: InstanceId;
  /** Puede cambiar la lista blanca. */
  canAllow: boolean;
  /** Puede cambiar permisos. */
  canOp: boolean;
}

const LEVEL_LABEL: Record<BedrockLevel, string> = {
  operator: "Operador",
  member: "Miembro",
  visitor: "Visitante",
};

/** Mismo criterio que el supervisor: letras, números, espacios, _ - # . (1 a 32). */
export function validGamertag(name: string): boolean {
  const n = name.trim();
  return n.length > 0 && [...n].length <= 32 && /^[\p{L}\p{N} _#.-]+$/u.test(n);
}

function when(unix: number) {
  if (!unix) return null;
  return new Date(unix * 1000).toLocaleString(uiLocale(), { dateStyle: "short", timeStyle: "short" });
}

export function BedrockPlayersPanel({ sup, id, canAllow, canOp }: Props) {
  const [info, setInfo] = useState<BedrockPlayersInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [ignoreLimit, setIgnoreLimit] = useState(false);

  const load = useCallback(async () => {
    try {
      setInfo(await sup.bedrockPlayers(id));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  useEffect(() => {
    load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, [load]);

  async function run(f: () => Promise<BedrockPlayersInfo>) {
    setBusy(true);
    setError(null);
    try {
      setInfo(await f());
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function add() {
    const n = name.trim();
    if (!validGamertag(n)) return;
    if (await run(() => sup.bedrockAllowlist(id, { type: "add", name: n, ignores_player_limit: ignoreLimit }))) {
      setName("");
      setIgnoreLimit(false);
    }
  }

  if (!info)
    return (
      <div className="admin-panel" role="tabpanel" aria-label="Jugadores">
        {error ? <p className="form-error">{error}</p> : <p className="wiz-loading">Cargando…</p>}
      </div>
    );

  const levelOf = (xuid: string): BedrockLevel | null => info.permissions.find((p) => p.xuid === xuid)?.level ?? null;
  const defaultLabel = LEVEL_LABEL[info.default_permission as BedrockLevel] ?? info.default_permission;
  const unknownPerms = info.permissions.filter((p) => !info.known.some((k) => k.xuid === p.xuid));
  const applyNote = info.running
    ? "El servidor está encendido: los cambios se aplican al momento."
    : "Los cambios se guardan en los archivos y valen al encender el servidor.";

  return (
    <div className="admin-panel bedrock-players" role="tabpanel" aria-label="Jugadores">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">Jugadores</h2>
          <p className="field-help">{applyNote}</p>
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

      <section className="card props-card" aria-label="Conectados ahora">
        <h2>
          <Users size={16} aria-hidden="true" /> Conectados ahora
        </h2>
        {!info.running ? (
          <p className="field-help">Enciende el servidor para ver quién está conectado.</p>
        ) : info.online.length === 0 ? (
          <p className="field-help">No hay nadie conectado.</p>
        ) : (
          <ul className="plain-list" translate="no">
            {info.online.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        )}
      </section>

      <section className="card props-card" aria-label="Lista blanca">
        <h2>
          <UserPlus size={16} aria-hidden="true" /> Lista blanca
        </h2>
        {info.legacy_whitelist && (
          <p className="warn-text">
            <TriangleAlert size={13} aria-hidden="true" /> Hay un archivo whitelist.json: el servidor lo usa en lugar de
            allowlist.json, así que esta lista no tendrá efecto. Bórralo o renómbralo en Archivos.
          </p>
        )}
        {!info.online_mode && (
          <p className="warn-text">
            <TriangleAlert size={13} aria-hidden="true" /> online-mode está desactivado: así el servidor no usa la lista
            blanca. Actívalo en Ajustes.
          </p>
        )}
        <label className="switch-row">
          <input
            type="checkbox"
            checked={info.allow_list_enabled}
            disabled={busy || !canAllow}
            onChange={(e) => run(() => sup.bedrockAllowlistEnabled(id, e.target.checked))}
          />
          <span>
            <strong>Solo pueden entrar los de la lista</strong>
            <span className="field-help">Se guarda en server.properties (allow-list). Los operadores también necesitan estar en la lista.</span>
          </span>
        </label>
        {canAllow && (
          <form
            className="address-row"
            onSubmit={(e) => {
              e.preventDefault();
              void add();
            }}
          >
            <label className="field">
              <span>Gamertag de Xbox</span>
              <input type="text" value={name} placeholder="Steve Craft" autoComplete="off" spellCheck={false} onChange={(e) => setName(e.target.value)} translate="no" />
            </label>
            <label className="check-row">
              <input type="checkbox" checked={ignoreLimit} onChange={(e) => setIgnoreLimit(e.target.checked)} /> Puede entrar aunque el
              servidor esté lleno
            </label>
            <button type="submit" className="btn btn-sm btn-go" disabled={busy || !validGamertag(name)}>
              Añadir
            </button>
          </form>
        )}
        {name.trim() !== "" && !validGamertag(name) && (
          <p className="warn-text">Solo letras, números, espacios y _ - # . (hasta 32 caracteres).</p>
        )}
        {info.allowlist.length === 0 ? (
          <p className="field-help">La lista está vacía.</p>
        ) : (
          <ul className="bp-list">
            {info.allowlist.map((a) => (
              <li key={a.name}>
                <strong translate="no">{a.name}</strong>
                <span className="field-help">{a.xuid ? "ya entró alguna vez" : "todavía no ha entrado"}</span>
                <label className="check-row">
                  <input
                    type="checkbox"
                    checked={a.ignores_player_limit}
                    disabled={busy || !canAllow}
                    aria-label={`Puede entrar aunque esté lleno: ${a.name}`}
                    onChange={(e) => run(() => sup.bedrockAllowlist(id, { type: "set_ignores_limit", name: a.name, value: e.target.checked }))}
                  />{" "}
                  Aunque esté lleno
                </label>
                {canAllow && (
                  <button
                    type="button"
                    className="btn btn-xs"
                    disabled={busy}
                    aria-label={`Quitar de la lista: ${a.name}`}
                    onClick={() => run(() => sup.bedrockAllowlist(id, { type: "remove", name: a.name }))}
                  >
                    Quitar
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="card props-card" aria-label="Permisos">
        <h2>
          <ShieldCheck size={16} aria-hidden="true" /> Permisos
        </h2>
        <p className="field-help">
          Bedrock da los permisos por XUID (el identificador de Xbox), no por nombre. MercHosting lo anota cuando el jugador entra a
          este servidor; si alguien no aparece aquí, pídele que entre una vez. Quien no tiene un permiso propio es {defaultLabel}.
        </p>
        {info.known.length === 0 ? (
          <p className="field-help">Todavía no ha entrado nadie mientras MercHosting miraba la consola.</p>
        ) : (
          <ul className="bp-list">
            {info.known.map((k) => {
              const lv = levelOf(k.xuid);
              const seen = when(k.last_seen_unix);
              return (
                <li key={k.xuid}>
                  <strong translate="no">{k.name}</strong>
                  <span className="field-help">
                    {k.online ? "conectado" : seen ? `visto el ${seen}` : "conocido por la lista blanca"}
                  </span>
                  <label className="field bp-level">
                    <span className="sr-only">Permiso de {k.name}</span>
                    <select
                      value={lv ?? ""}
                      disabled={busy || !canOp}
                      aria-label={`Permiso de ${k.name}`}
                      onChange={(e) => run(() => sup.bedrockPermission(id, k.xuid, (e.target.value || null) as BedrockLevel | null))}
                    >
                      <option value="">Por defecto ({defaultLabel})</option>
                      <option value="operator">Operador</option>
                      <option value="member">Miembro</option>
                      <option value="visitor">Visitante</option>
                    </select>
                  </label>
                </li>
              );
            })}
          </ul>
        )}
        {unknownPerms.length > 0 && (
          <>
            <p className="field-help">En permissions.json también hay XUIDs que MercHosting no ha visto entrar:</p>
            <ul className="bp-list">
              {unknownPerms.map((p) => (
                <li key={p.xuid}>
                  <code translate="no">{p.xuid}</code>
                  <span className="field-help">{LEVEL_LABEL[p.level]}</span>
                  {canOp && (
                    <button
                      type="button"
                      className="btn btn-xs"
                      disabled={busy}
                      aria-label={`Quitar permiso de ${p.xuid}`}
                      onClick={() => run(() => sup.bedrockPermission(id, p.xuid, null))}
                    >
                      Quitar permiso
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
    </div>
  );
}
