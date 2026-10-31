// Pestaña Mundos: mundos del servidor (activar, duplicar, exportar, importar, papelera) y
// reglas del juego (con el servidor encendido, mediante /gamerule).

import { Copy, Download, Globe, PackageOpen, RefreshCw, Star, Trash2, TriangleAlert, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId, RuleValue, WorldInfo, WorldOp } from "../api/types";
import { MercError } from "../api/types";
import { formatSize, safeFolderName } from "../format";
import { Dialog } from "./Dialog";
import { uiLocale } from "../i18n/lang";

type RuleGroup = "Jugadores" | "Mundo" | "Criaturas" | "Chat";
const RULE_GROUPS: RuleGroup[] = ["Jugadores", "Mundo", "Criaturas", "Chat"];

const RULE_TEXT: Record<string, { label: string; help?: string; group: RuleGroup }> = {
  keep_inventory: { group: "Jugadores", label: "Conservar el inventario al morir", help: "Los jugadores no pierden objetos ni experiencia." },
  pvp: { group: "Jugadores", label: "Combate entre jugadores (PvP)" },
  natural_health_regeneration: { group: "Jugadores", label: "Regeneración natural de vida" },
  immediate_respawn: { group: "Jugadores", label: "Reaparecer sin pantalla de muerte" },
  fall_damage: { group: "Jugadores", label: "Daño por caída" },
  fire_damage: { group: "Jugadores", label: "Daño por fuego" },
  drowning_damage: { group: "Jugadores", label: "Daño por ahogarse" },
  freeze_damage: { group: "Jugadores", label: "Daño por congelarse", help: "En nieve polvo." },
  players_sleeping_percentage: { group: "Jugadores", label: "Porcentaje que debe dormir para pasar la noche", help: "De 0 a 100. Por defecto 100 (todos)." },
  advance_time: { group: "Mundo", label: "Ciclo de día y noche", help: "Si lo apagas, la hora se queda quieta." },
  advance_weather: { group: "Mundo", label: "El clima cambia solo", help: "Si lo apagas, el clima se queda como está." },
  block_drops: { group: "Mundo", label: "Los bloques rotos sueltan objetos" },
  random_tick_speed: { group: "Mundo", label: "Velocidad de crecimiento (random tick)", help: "Por defecto 3. Más alto = cultivos más rápidos y más CPU." },
  mob_griefing: { group: "Criaturas", label: "Los mobs modifican bloques", help: "Creepers que rompen, endermans que roban bloques…" },
  spawn_mobs: { group: "Criaturas", label: "Aparecen criaturas de forma natural" },
  spawn_phantoms: { group: "Criaturas", label: "Aparecen phantoms", help: "Los que llegan si no duermes en varios días." },
  spawn_patrols: { group: "Criaturas", label: "Aparecen patrullas de saqueadores" },
  spawn_wandering_traders: { group: "Criaturas", label: "Aparece el comerciante errante" },
  mob_drops: { group: "Criaturas", label: "Las criaturas sueltan objetos al morir" },
  show_advancement_messages: { group: "Chat", label: "Anunciar logros en el chat" },
  show_death_messages: { group: "Chat", label: "Mostrar mensajes de muerte en el chat" },
};

const OP_VERB: Record<WorldOp["kind"], string> = {
  duplicate: "Duplicando",
  export: "Exportando",
  import: "Importando",
  install: "Instalando",
  backup: "Copiando",
  verify: "Verificando",
  restore: "Restaurando",
};

interface Props {
  /** Servidor de un amigo: importar un .zip todavía no se puede desde otra PC. */
  remote?: boolean;
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  running: boolean;
  online: boolean;
}

type Ask = { kind: "duplicate" | "import"; value: string; source?: string } | { kind: "trash"; world: WorldInfo };

function formatDate(ms: number | null): string {
  return ms ? new Date(ms).toLocaleString(uiLocale(), { dateStyle: "short", timeStyle: "short" }) : "";
}

export function WorldsPanel({ api, sup, id, running, online, remote = false }: Props) {
  const [worlds, setWorlds] = useState<WorldInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [ask, setAsk] = useState<Ask | null>(null);
  const [rules, setRules] = useState<RuleValue[] | null>(null);
  const [rulesError, setRulesError] = useState<string | null>(null);
  const [rulesLoading, setRulesLoading] = useState(false);
  const [op, setOp] = useState<WorldOp | null>(null);

  const load = useCallback(async () => {
    try {
      setWorlds((await sup.worlds(id)).worlds);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  const loadRules = useCallback(async () => {
    setRulesLoading(true);
    setRulesError(null);
    try {
      setRules((await sup.gameRules(id)).rules);
    } catch (e) {
      setRulesError(errorMessage(e));
    } finally {
      setRulesLoading(false);
    }
  }, [sup, id]);

  useEffect(() => {
    load();
    // Si al abrir la pestaña ya hay una operación en marcha (empezada antes), se muestra.
    sup.adminState(id).then(
      (st) => setOp(st.world_op),
      () => undefined,
    );
  }, [load, sup, id]);

  // Mientras dura una operación de mundos, se consulta su progreso.
  const watching = busy || op !== null;
  useEffect(() => {
    if (!watching) return;
    let alive = true;
    const t = setInterval(async () => {
      try {
        const st = await sup.adminState(id);
        if (!alive) return;
        setOp(st.world_op);
        if (!st.world_op && !busy) load();
      } catch {
        /* se reintenta en el siguiente ciclo */
      }
    }, 500);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [watching, busy, sup, id, load]);

  useEffect(() => {
    if (online) loadRules();
    else setRules(null);
  }, [online, loadRules]);

  async function run(fn: () => Promise<string | void>) {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const msg = await fn();
      if (msg) setNote(msg);
      await load();
    } catch (e) {
      if (e instanceof MercError && e.code === "conflict" && /cancelada/.test(e.message)) {
        setNote("Cancelado. Se deshizo lo que se había hecho a medias.");
      } else {
        setError(errorMessage(e));
      }
    } finally {
      setBusy(false);
      setOp(null);
    }
  }

  async function cancelOp() {
    try {
      await sup.cancelWorldOp(id);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function exportWorld(w: WorldInfo) {
    const dest = await api.pickSavePath(`${w.name}.zip`);
    if (!dest) return;
    await run(async () => {
      await sup.exportWorld(id, w.name, dest);
      return `«${w.name}» exportado a ${dest}`;
    });
  }

  async function startImport() {
    const source = await api.pickFile({ name: "Mundo de Minecraft (.zip)", extensions: ["zip"] });
    if (!source) return;
    const base = source.split(/[\\/]/).pop()!.replace(/\.zip$/i, "");
    setAsk({ kind: "import", value: safeFolderName(base), source });
  }

  async function setRule(r: RuleValue, value: string) {
    setRulesError(null);
    try {
      await sup.setGameRule(id, r.key, value);
      setRules((cur) => cur?.map((x) => (x.key === r.key ? { ...x, value } : x)) ?? null);
    } catch (e) {
      setRulesError(errorMessage(e));
    }
  }

  function renderRule(r: RuleValue) {
    const text = RULE_TEXT[r.key] ?? { label: r.name };
    if (r.kind.type === "bool") {
      return (
        <label key={r.key} className="switch-row rule-row">
          <input type="checkbox" checked={r.value === "true"} onChange={(e) => setRule(r, String(e.target.checked))} />
          <span>
            <strong>{text.label}</strong>
            {text.help && <span className="field-help">{text.help}</span>}
          </span>
        </label>
      );
    }
    const kind = r.kind;
    return (
      <div key={r.key} className="field rule-row">
        <label htmlFor={`rule-${r.key}`}>{text.label}</label>
        <input
          id={`rule-${r.key}`}
          type="text"
          inputMode="numeric"
          defaultValue={r.value ?? ""}
          onBlur={(e) => {
            const v = e.target.value.trim();
            const n = Number(v);
            if (v !== r.value && Number.isInteger(n) && n >= kind.min && n <= kind.max) setRule(r, v);
          }}
        />
        {text.help && <p className="field-help">{text.help}</p>}
      </div>
    );
  }

  const locked = running || busy || op !== null;
  const shownRules = rules?.filter((r) => r.value !== null) ?? [];
  const missingRules = rules ? rules.length - shownRules.length : 0;

  return (
    <div className="admin-panel" role="tabpanel" aria-label="Mundos">
      {running && (
        <p className="field-help warn-text">
          Para duplicar, exportar, importar o cambiar de mundo, detén el servidor: así el mundo queda guardado del todo.
        </p>
      )}
      <div className="files-bar">
        <h2 className="panel-title">Mundos de este servidor</h2>
        <button type="button" className="btn btn-sm" onClick={load} disabled={busy}>
          <RefreshCw size={14} aria-hidden="true" /> Actualizar
        </button>
        {!remote && (
          <button type="button" className="btn btn-sm btn-go" onClick={startImport} disabled={locked}>
            <PackageOpen size={14} aria-hidden="true" /> Importar .zip
          </button>
        )}
      </div>
      {op && (
        <div className="card world-op" role="status">
          <div className="dl">
            <div className="dl-head">
              <span>
                {OP_VERB[op.kind]} «{op.world}»…
              </span>
              <span>
                {formatSize(op.done_bytes)}
                {op.total_bytes ? ` de ${formatSize(op.total_bytes)}` : ""}
              </span>
            </div>
            <div
              className="dl-bar"
              role="progressbar"
              aria-label="Progreso"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={op.total_bytes ? Math.min(100, Math.round((op.done_bytes / op.total_bytes) * 100)) : undefined}
            >
              <span
                style={{ width: op.total_bytes ? `${Math.min(100, (op.done_bytes / op.total_bytes) * 100)}%` : "35%" }}
                data-indeterminate={!op.total_bytes}
              />
            </div>
          </div>
          <button type="button" className="btn btn-sm" onClick={cancelOp}>
            <X size={14} aria-hidden="true" /> Cancelar
          </button>
        </div>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {note && !error && (
        <p className="field-help ok-text" role="status">
          {note}
        </p>
      )}

      {worlds === null ? (
        <p className="wiz-loading" role="status">
          Buscando mundos…
        </p>
      ) : worlds.length === 0 ? (
        <div className="card empty-small">
          <h2>Todavía no hay mundos</h2>
          <p>Minecraft crea el mundo la primera vez que enciendes el servidor. También puedes importar uno desde un .zip.</p>
        </div>
      ) : (
        <ul className="world-list">
          {worlds.map((w) => (
            <li key={w.name} className="card world-card" data-active={w.active}>
              <span className="world-icon">
                <Globe size={20} aria-hidden="true" />
              </span>
              <span className="world-main">
                <span className="world-name">
                  <span translate="no">{w.name}</span>
                  {w.active && (
                    <span className="badge">
                      <Star size={10} aria-hidden="true" /> Activo
                    </span>
                  )}
                </span>
                <span className="field-help">
                  {formatSize(w.size_bytes)}
                  {w.modified_unix_ms ? ` · guardado ${formatDate(w.modified_unix_ms)}` : ""}
                  {w.companions.length > 0 ? ` · con ${w.companions.join(" y ")}` : ""}
                </span>
              </span>
              <span className="world-actions">
                {!w.active && (
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={locked}
                    onClick={() => run(async () => (await sup.setActiveWorld(id, w.name), `Ahora el servidor cargará «${w.name}»`))}
                  >
                    Usar este mundo
                  </button>
                )}
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={`Duplicar ${w.name}`}
                  title="Duplicar"
                  disabled={locked}
                  onClick={() => setAsk({ kind: "duplicate", value: `${w.name}-copia` , source: w.name })}
                >
                  <Copy size={15} />
                </button>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={`Exportar ${w.name}`}
                  title="Exportar a .zip"
                  disabled={locked}
                  onClick={() => exportWorld(w)}
                >
                  <Download size={15} />
                </button>
                <button
                  type="button"
                  className="icon-btn icon-danger"
                  aria-label={`Mover ${w.name} a la papelera`}
                  title={w.active ? "Es el mundo activo" : "Mover a la papelera"}
                  disabled={locked || w.active}
                  onClick={() => setAsk({ kind: "trash", world: w })}
                >
                  <Trash2 size={15} />
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}

      <section className="card props-card">
        <div className="files-bar">
          <h2 className="panel-title">Reglas del juego</h2>
          {online && (
            <button type="button" className="btn btn-sm" onClick={loadRules} disabled={rulesLoading}>
              <RefreshCw size={14} aria-hidden="true" /> Volver a leer
            </button>
          )}
        </div>
        {!online ? (
          <p className="field-help">
            Enciende el servidor para ver y cambiar las reglas: se usan los comandos oficiales de Minecraft (/gamerule) y se
            guardan en el mundo activo.
          </p>
        ) : rulesLoading && !rules ? (
          <p className="wiz-loading" role="status">
            Preguntando al servidor…
          </p>
        ) : (
          rules && (
            <>
              {RULE_GROUPS.map((g) => {
                // Primero los interruptores y después los números, para que la cuadrícula quede pareja.
                const list = shownRules
                  .filter((r) => (RULE_TEXT[r.key]?.group ?? "Mundo") === g)
                  .sort((a, b) => Number(a.kind.type !== "bool") - Number(b.kind.type !== "bool"));
                if (list.length === 0) return null;
                return (
                  <div key={g} className="rule-group">
                    <h3 className="rule-group-title">{g}</h3>
                    <div className="rules-grid">{list.map(renderRule)}</div>
                  </div>
                );
              })}
              {missingRules > 0 && (
                <p className="field-help">
                  {missingRules === 1 ? "1 regla no respondió" : `${missingRules} reglas no respondieron`}: no existen en esta
                  versión de Minecraft (o el servidor tardó; prueba «Volver a leer»).
                </p>
              )}
            </>
          )
        )}
        {rulesError && (
          <p className="form-error" role="alert">
            {rulesError}
          </p>
        )}
      </section>

      {ask && ask.kind !== "trash" && (
        <Dialog
          title={ask.kind === "duplicate" ? `Duplicar «${ask.source}»` : "Importar un mundo"}
          lead={
            ask.kind === "duplicate"
              ? "Se crea una copia con otro nombre (incluye el Nether y el End si están aparte)."
              : `Se extrae el mundo de ${ask.source} a una carpeta nueva. No se toca ningún mundo existente.`
          }
          onClose={() => setAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                disabled={!ask.value.trim()}
                onClick={() => {
                  const a = ask;
                  setAsk(null);
                  if (a.kind === "duplicate")
                    run(async () => (await sup.duplicateWorld(id, a.source!, a.value.trim()), `Copia creada: «${a.value.trim()}»`));
                  else
                    run(async () => (await sup.importWorld(id, a.source!, a.value.trim()), `Mundo importado: «${a.value.trim()}»`));
                }}
              >
                {ask.kind === "duplicate" ? "Duplicar" : "Importar"}
              </button>
            </>
          }
        >
          <div className="field">
            <label htmlFor="world-name-input">Nombre de la carpeta del mundo</label>
            <input id="world-name-input" type="text" maxLength={64} value={ask.value} onChange={(e) => setAsk({ ...ask, value: e.target.value })} />
          </div>
        </Dialog>
      )}

      {ask?.kind === "trash" && (
        <Dialog
          title={`Mover «${ask.world.name}» a la papelera`}
          lead="El mundo sale del servidor y se guarda en la papelera de MercHosting. Puedes restaurarlo desde Archivos › Papelera y copias."
          onClose={() => setAsk(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setAsk(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                onClick={() => {
                  const w = ask.world;
                  setAsk(null);
                  run(async () => (await sup.trashWorld(id, w.name), `«${w.name}» está en la papelera`));
                }}
              >
                Mover a la papelera
              </button>
            </>
          }
        >
          <p className="field-help">
            <TriangleAlert size={13} aria-hidden="true" /> Tamaño: {formatSize(ask.world.size_bytes)}
          </p>
        </Dialog>
      )}
    </div>
  );
}
