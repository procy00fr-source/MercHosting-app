// Pestaña Ajustes: editor visual de server.properties.
// Las opciones conocidas tienen controles con nombre claro; el resto está en «Avanzado».
// Guardar crea antes una copia del archivo y detecta si otro programa lo cambió.

import { Image, RotateCcw, Save, Search, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId, PropEntry } from "../api/types";
import { MercError } from "../api/types";
import { ResourcePackBrowser } from "./ResourcePackBrowser";

type Field =
  | { key: string; label: string; help?: string; type: "bool" }
  | { key: string; label: string; help?: string; type: "int"; min: number; max: number }
  | { key: string; label: string; help?: string; type: "choice"; options: [string, string][] }
  | { key: string; label: string; help?: string; type: "text"; max: number };

const GROUPS: { title: string; fields: Field[] }[] = [
  {
    title: "General",
    fields: [
      { key: "max-players", label: "Jugadores máximos", type: "int", min: 1, max: 1000 },
      {
        key: "gamemode",
        label: "Modo de juego",
        type: "choice",
        options: [
          ["survival", "Supervivencia"],
          ["creative", "Creativo"],
          ["adventure", "Aventura"],
          ["spectator", "Espectador"],
        ],
      },
      {
        key: "difficulty",
        label: "Dificultad",
        type: "choice",
        options: [
          ["peaceful", "Pacífica"],
          ["easy", "Fácil"],
          ["normal", "Normal"],
          ["hard", "Difícil"],
        ],
      },
      { key: "force-gamemode", label: "Forzar el modo de juego al entrar", type: "bool" },
      { key: "hardcore", label: "Extremo (hardcore)", help: "Al morir, el jugador pasa a espectador.", type: "bool" },
      { key: "pvp", label: "Combate entre jugadores (PvP)", type: "bool" },
      { key: "allow-flight", label: "Permitir volar", help: "Útil con mods de vuelo; si no, el servidor expulsa a quien vuele.", type: "bool" },
    ],
  },
  {
    title: "Acceso",
    fields: [
      {
        key: "online-mode",
        label: "Comprobar cuentas oficiales (online-mode)",
        help: "Recomendado. Sin esto cualquiera puede entrar con cualquier nombre.",
        type: "bool",
      },
      { key: "white-list", label: "Lista blanca", help: "Solo entran los de la lista (pestaña Jugadores).", type: "bool" },
      { key: "enforce-whitelist", label: "Expulsar a quien no esté en la lista al activarla", type: "bool" },
      { key: "server-port", label: "Puerto", type: "int", min: 1024, max: 65535 },
      { key: "spawn-protection", label: "Protección del punto de aparición (bloques)", help: "0 = sin protección. No afecta a los operadores.", type: "int", min: 0, max: 1000 },
      { key: "player-idle-timeout", label: "Expulsar por inactividad (minutos)", help: "0 = nunca.", type: "int", min: 0, max: 10000 },
    ],
  },
  {
    title: "Mundo",
    fields: [
      { key: "level-seed", label: "Semilla", help: "Solo se usa al generar un mundo nuevo.", type: "text", max: 64 },
      { key: "allow-nether", label: "Permitir el Nether", type: "bool" },
      { key: "spawn-monsters", label: "Aparecen monstruos", type: "bool" },
      { key: "generate-structures", label: "Generar estructuras (aldeas, templos…)", help: "Solo en zonas nuevas del mapa.", type: "bool" },
      { key: "enable-command-block", label: "Bloques de comandos", type: "bool" },
    ],
  },
  {
    title: "Rendimiento",
    fields: [
      { key: "view-distance", label: "Distancia de visión (chunks)", help: "Más alto = más memoria y CPU.", type: "int", min: 3, max: 32 },
      { key: "simulation-distance", label: "Distancia de simulación (chunks)", help: "Hasta dónde se mueven mobs y crecen cultivos.", type: "int", min: 3, max: 32 },
    ],
  },
];

/** server.properties de Bedrock Dedicated Server: otras claves (server-name, allow-list…). */
const BEDROCK_GROUPS: { title: string; fields: Field[] }[] = [
  {
    title: "General",
    fields: [
      { key: "server-name", label: "Nombre en la lista de servidores", type: "text", max: 64 },
      { key: "max-players", label: "Jugadores máximos", type: "int", min: 1, max: 1000 },
      {
        key: "gamemode",
        label: "Modo de juego",
        type: "choice",
        options: [
          ["survival", "Supervivencia"],
          ["creative", "Creativo"],
          ["adventure", "Aventura"],
        ],
      },
      {
        key: "difficulty",
        label: "Dificultad",
        type: "choice",
        options: [
          ["peaceful", "Pacífica"],
          ["easy", "Fácil"],
          ["normal", "Normal"],
          ["hard", "Difícil"],
        ],
      },
      { key: "force-gamemode", label: "Forzar el modo de juego al entrar", type: "bool" },
      { key: "allow-cheats", label: "Permitir trucos (comandos)", help: "Necesario para que funcionen muchos comandos y addons.", type: "bool" },
    ],
  },
  {
    title: "Acceso",
    fields: [
      {
        key: "online-mode",
        label: "Comprobar cuentas de Microsoft (online-mode)",
        help: "Recomendado. Cada jugador entra con su cuenta de Microsoft verificada.",
        type: "bool",
      },
      { key: "allow-list", label: "Lista de permitidos (allow-list)", help: "Solo entran los de allowlist.json (está en Archivos).", type: "bool" },
      { key: "server-port", label: "Puerto (UDP, IPv4)", type: "int", min: 1024, max: 65535 },
      { key: "server-portv6", label: "Puerto (UDP, IPv6)", type: "int", min: 1024, max: 65535 },
      { key: "player-idle-timeout", label: "Expulsar por inactividad (minutos)", help: "0 = nunca.", type: "int", min: 0, max: 10000 },
    ],
  },
  {
    title: "Mundo",
    fields: [{ key: "level-seed", label: "Semilla", help: "Solo se usa al generar un mundo nuevo.", type: "text", max: 64 }],
  },
  {
    title: "Rendimiento",
    fields: [
      { key: "view-distance", label: "Distancia de visión (chunks)", help: "Más alto = más memoria y CPU.", type: "int", min: 5, max: 32 },
      { key: "tick-distance", label: "Distancia de simulación (chunks)", help: "Hasta dónde se mueven mobs y crecen cultivos.", type: "int", min: 4, max: 12 },
    ],
  },
];
const BEDROCK_KNOWN = new Set(BEDROCK_GROUPS.flatMap((g) => g.fields.map((f) => f.key)));

/** Las gestiona la tarjeta «Paquete de recursos» (la huella la calcula el supervisor). */
const PACK_KEYS = ["resource-pack", "resource-pack-sha1", "require-resource-pack"];
// El MOTD tiene su propio editor (pestaña «MOTD e icono»), con colores y dos líneas.
const KNOWN = new Set([...GROUPS.flatMap((g) => g.fields.map((f) => f.key)), ...PACK_KEYS, "motd"]);

function fieldError(f: Field, v: string): string | null {
  if (f.type === "int") {
    if (!/^-?\d+$/.test(v)) return "Escribe un número";
    const n = Number(v);
    if (n < f.min || n > f.max) return `Entre ${f.min} y ${f.max}`;
  }
  if (f.type === "text" && [...v].length > f.max) return `Máximo ${f.max} caracteres`;
  return null;
}

interface Props {
  sup: Supervisor;
  id: InstanceId;
  running: boolean;
  mcVersion?: string | null;
  openUrl?: (url: string) => void;
  /** Servidor Bedrock: claves de Bedrock y sin paquete de recursos de Java. */
  bedrock?: boolean;
}

export function PropertiesPanel({ sup, id, running, mcVersion = null, openUrl, bedrock = false }: Props) {
  const groups = bedrock ? BEDROCK_GROUPS : GROUPS;
  const known = bedrock ? BEDROCK_KNOWN : KNOWN;
  const [loaded, setLoaded] = useState<{ exists: boolean; entries: PropEntry[]; modified_unix_ms: number | null } | null>(null);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    setConflict(false);
    try {
      const r = await sup.properties(id);
      setLoaded(r);
      setDraft({});
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  useEffect(() => {
    load();
  }, [load]);

  const original = useMemo(() => Object.fromEntries((loaded?.entries ?? []).map((e) => [e.key, e.value])), [loaded]);
  const value = (k: string) => draft[k] ?? original[k] ?? "";
  const set = (k: string, v: string) => {
    setSaved(null);
    setDraft((d) => {
      const n = { ...d };
      if (v === original[k]) delete n[k];
      else n[k] = v;
      return n;
    });
  };

  const fields = groups.flatMap((g) => g.fields);
  const invalid = fields.some((f) => f.key in draft && fieldError(f, draft[f.key]) !== null);
  const changes = Object.keys(draft).length;

  async function save() {
    if (!loaded) return;
    setSaving(true);
    setError(null);
    try {
      const r = await sup.setProperties(id, draft, loaded.modified_unix_ms);
      setLoaded(r);
      setDraft({});
      setSaved(
        running
          ? "Guardado. Los cambios se aplican cuando reinicies el servidor."
          : "Guardado. Se guardó una copia del archivo anterior por si quieres volver atrás.",
      );
    } catch (e) {
      if (e instanceof MercError && e.code === "conflict") setConflict(true);
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  }

  if (!loaded) {
    return (
      <div className="admin-panel" role="tabpanel" aria-label="Ajustes">
        {error ? (
          <p className="form-error" role="alert">
            {error}
          </p>
        ) : (
          <p className="wiz-loading" role="status">
            Leyendo server.properties…
          </p>
        )}
      </div>
    );
  }

  if (!loaded.exists) {
    return (
      <div className="admin-panel" role="tabpanel" aria-label="Ajustes">
        <div className="empty-small">
          <h2>Todavía no hay server.properties</h2>
          <p>Minecraft lo crea la primera vez que se enciende el servidor. Enciéndelo una vez y vuelve aquí.</p>
        </div>
      </div>
    );
  }

  const advanced = loaded.entries.filter((e) => !known.has(e.key));

  return (
    <div className="admin-panel" role="tabpanel" aria-label="Ajustes">
      <div className="save-bar" data-dirty={changes > 0}>
        <p>
          {changes > 0
            ? `${changes} cambio${changes === 1 ? "" : "s"} sin guardar`
            : running
              ? "Los cambios se aplicarán al reiniciar el servidor."
              : "Edita y guarda. Antes de guardar se hace una copia del archivo."}
        </p>
        <button type="button" className="btn btn-quiet" disabled={changes === 0 || saving} onClick={() => setDraft({})}>
          <RotateCcw size={15} aria-hidden="true" /> Descartar
        </button>
        <button type="button" className="btn btn-go" disabled={changes === 0 || invalid || saving} onClick={save}>
          <Save size={15} aria-hidden="true" /> {saving ? "Guardando…" : "Guardar cambios"}
        </button>
      </div>
      {error && (
        <div className="form-error" role="alert">
          {error}{" "}
          {conflict && (
            <button type="button" className="btn btn-sm" onClick={load}>
              Volver a cargar
            </button>
          )}
        </div>
      )}
      {saved && (
        <p className="field-help ok-text" role="status">
          {saved}
        </p>
      )}

      <div className="props-groups">
        {groups.map((g) => {
          const present = g.fields.filter((f) => f.key in original);
          if (present.length === 0) return null;
          return (
            <section key={g.title} className="card props-card">
              <h2>{g.title}</h2>
              {present.map((f) => {
                const v = value(f.key);
                const err = fieldError(f, v);
                const fid = `prop-${f.key}`;
                const changed = f.key in draft;
                if (f.type === "bool") {
                  return (
                    <label key={f.key} className="switch-row prop-row" data-changed={changed}>
                      <input type="checkbox" checked={v === "true"} onChange={(e) => set(f.key, String(e.target.checked))} />
                      <span>
                        <strong>{f.label}</strong>
                        {f.help && <span className="field-help">{f.help}</span>}
                        {f.key === "online-mode" && v === "false" && (
                          <span className="field-help warn-text">
                            <TriangleAlert size={12} aria-hidden="true" /> Cualquiera puede entrar usando el nombre de otro.
                          </span>
                        )}
                      </span>
                    </label>
                  );
                }
                return (
                  <div key={f.key} className="field prop-row" data-changed={changed}>
                    <label htmlFor={fid}>{f.label}</label>
                    {f.type === "choice" ? (
                      <select id={fid} value={v} onChange={(e) => set(f.key, e.target.value)}>
                        {!f.options.some(([o]) => o === v) && <option value={v}>{v}</option>}
                        {f.options.map(([o, l]) => (
                          <option key={o} value={o}>
                            {l}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        id={fid}
                        type="text"
                        inputMode={f.type === "int" ? "numeric" : undefined}
                        value={v}
                        onChange={(e) => set(f.key, e.target.value)}
                        aria-invalid={!!err}
                      />
                    )}
                    {err ? <p className="form-error">{err}</p> : f.help && <p className="field-help">{f.help}</p>}
                  </div>
                );
              })}
            </section>
          );
        })}
      </div>

      {!bedrock && (
      <ResourcePackCard
        sup={sup}
        id={id}
        mcVersion={mcVersion}
        openUrl={openUrl}
        url={original["resource-pack"] ?? ""}
        sha1={original["resource-pack-sha1"] ?? ""}
        required={original["require-resource-pack"] === "true"}
        running={running}
        blocked={changes > 0}
        onSaved={(r) => {
          setLoaded(r);
          setDraft({});
        }}
      />
      )}

      {advanced.length > 0 && (
        <section className="card props-card">
          <button type="button" className="advanced-toggle" aria-expanded={showAdvanced} onClick={() => setShowAdvanced(!showAdvanced)}>
            Avanzado: {advanced.length} opciones más
          </button>
          {showAdvanced && (
            <>
              <p className="field-help">
                Opciones menos comunes o de mods, tal como están en el archivo. Cámbialas solo si sabes qué hacen.
              </p>
              <div className="props-advanced">
                {advanced.map((e) => (
                  <div key={e.key} className="field prop-row" data-changed={e.key in draft}>
                    <label htmlFor={`adv-${e.key}`} className="mono">
                      {e.key}
                    </label>
                    <input id={`adv-${e.key}`} type="text" value={value(e.key)} onChange={(ev) => set(e.key, ev.target.value)} />
                  </div>
                ))}
              </div>
            </>
          )}
        </section>
      )}
    </div>
  );
}

function ResourcePackCard(props: {
  sup: Supervisor;
  id: InstanceId;
  mcVersion: string | null;
  openUrl?: (url: string) => void;
  url: string;
  sha1: string;
  required: boolean;
  running: boolean;
  /** Hay cambios sin guardar arriba: se guardan antes para no mezclar. */
  blocked: boolean;
  onSaved: (r: { exists: boolean; entries: PropEntry[]; modified_unix_ms: number | null }) => void;
}) {
  const { sup, id, url, sha1, required, running, blocked, onSaved, mcVersion, openUrl } = props;
  const [browsing, setBrowsing] = useState(false);
  const [draftUrl, setDraftUrl] = useState(url);
  const [draftRequired, setDraftRequired] = useState(required);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);

  useEffect(() => {
    setDraftUrl(url);
    setDraftRequired(required);
  }, [url, required]);

  const trimmed = draftUrl.trim();
  const badUrl = trimmed !== "" && !trimmed.startsWith("https://");
  const dirty = trimmed !== url || (trimmed !== "" && draftRequired !== required);

  async function apply(next: string, label?: string) {
    setBusy(true);
    setError(null);
    setOk(null);
    try {
      onSaved(await sup.setResourcePack(id, next, draftRequired));
      const when = running ? " Se aplica cuando reinicies el servidor." : "";
      setOk(next ? `${label ? `${label}: p` : "P"}aquete comprobado y guardado con su huella SHA-1.${when}` : `Paquete quitado.${when}`);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="card props-card" aria-label="Paquete de recursos">
      <h2>
        <Image size={16} aria-hidden="true" /> Paquete de recursos
      </h2>
      <p className="field-help">
        Texturas o sonidos que Minecraft ofrece a cada jugador al entrar. Pega el enlace de descarga directa de un .zip
        (HTTPS). MercHosting lo descarga una vez para comprobar que es un paquete de recursos y calcular su huella; así el
        juego no lo vuelve a bajar si ya lo tiene. El archivo tiene que seguir en esa dirección para que tus amigos lo
        reciban.
      </p>
      <div className="field">
        <label htmlFor="rp-url">Enlace del paquete (.zip)</label>
        <input
          id="rp-url"
          type="text"
          inputMode="url"
          spellCheck={false}
          placeholder="https://…/paquete.zip"
          value={draftUrl}
          maxLength={2000}
          onChange={(e) => {
            setDraftUrl(e.target.value);
            setOk(null);
          }}
          aria-invalid={badUrl}
        />
        {badUrl && <p className="form-error">Tiene que empezar por https://</p>}
        {sha1 && url && <p className="field-help mono">SHA-1: {sha1}</p>}
      </div>
      <label className="switch-row prop-row">
        <input type="checkbox" checked={draftRequired} onChange={(e) => setDraftRequired(e.target.checked)} />
        <span>
          <strong>Obligatorio</strong>
          <span className="field-help">Quien rechace el paquete no podrá entrar al servidor.</span>
        </span>
      </label>
      {blocked && <p className="field-help warn-text">Guarda o descarta antes los cambios de arriba.</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {ok && (
        <p className="field-help ok-text" role="status">
          {ok}
        </p>
      )}
      <div className="files-bar">
        <button
          type="button"
          className="btn btn-go"
          disabled={busy || blocked || badUrl || trimmed === "" || !dirty}
          onClick={() => apply(trimmed)}
        >
          {busy ? "Comprobando el paquete…" : "Comprobar y guardar"}
        </button>
        <button type="button" className="btn" disabled={busy || blocked} onClick={() => setBrowsing(true)}>
          <Search size={15} aria-hidden="true" /> Buscar en Modrinth
        </button>
        {url && (
          <button type="button" className="btn btn-quiet" disabled={busy || blocked} onClick={() => apply("")}>
            Quitar paquete
          </button>
        )}
      </div>
      {browsing && (
        <ResourcePackBrowser
          sup={sup}
          id={id}
          mcVersion={mcVersion}
          openUrl={openUrl}
          onClose={() => setBrowsing(false)}
          onPick={(u, label) => {
            setBrowsing(false);
            setDraftUrl(u);
            void apply(u, label);
          }}
        />
      )}
    </section>
  );
}
