// Pestaña Salud y rendimiento: lo que mide el supervisor (CPU y memoria de la PC, espacio en los
// discos, memoria y CPU del servidor), el TPS que informa el propio servidor cuando tiene ese
// comando, consejos para jugar y hostear en la misma PC, modos de rendimiento con vista previa y
// deshacer, y las alertas de este servidor.

import { Activity, Cpu, Gauge, HardDrive, MemoryStick, RotateCcw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { Alert, HealthReport, InstanceId, TpsReading } from "../api/types";
import { formatSize } from "../format";
import { uiLocale } from "../i18n/lang";


/** Modos de rendimiento: solo opciones de server.properties que Minecraft documenta. */
const PROFILES: { id: string; label: string; help: string; values: Record<string, string> }[] = [
  {
    id: "light",
    label: "Ligero",
    help: "Para PCs justas o si juegas en la misma PC: se ve y se simula menos lejos.",
    values: { "view-distance": "6", "simulation-distance": "4", "entity-broadcast-range-percentage": "75" },
  },
  {
    id: "balanced",
    label: "Equilibrado",
    help: "Buen punto medio para jugar con amigos.",
    values: { "view-distance": "8", "simulation-distance": "6", "entity-broadcast-range-percentage": "100" },
  },
  {
    id: "vanilla",
    label: "Como Minecraft",
    help: "Los valores por defecto de Minecraft.",
    values: { "view-distance": "10", "simulation-distance": "10", "entity-broadcast-range-percentage": "100" },
  },
];

const PROP_LABEL: Record<string, string> = {
  "view-distance": "Distancia de visión (chunks)",
  "simulation-distance": "Distancia de simulación (chunks)",
  "entity-broadcast-range-percentage": "Distancia a la que se ven entidades (%)",
};

function Bar({ value, max, label }: { value: number; max: number; label: string }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div
      className="health-bar"
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(pct)}
      data-level={pct >= 92 ? "bad" : pct >= 80 ? "warn" : "ok"}
    >
      <span style={{ width: `${pct}%` }} />
    </div>
  );
}

function tpsLevel(v: number): "ok" | "warn" | "bad" {
  return v >= 19 ? "ok" : v >= 15 ? "warn" : "bad";
}

interface Props {
  sup: Supervisor;
  id: InstanceId;
  /** Estado del servidor (`ONLINE`, `STOPPED`…). */
  state: string;
  alerts: Alert[];
}

export function HealthPanel({ sup, id, state, alerts }: Props) {
  const [h, setH] = useState<HealthReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [proc, setProc] = useState<{ cpu: number; mem: number } | null>(null);
  const [tps, setTps] = useState<TpsReading | null>(null);
  const [tpsBusy, setTpsBusy] = useState(false);
  const [tpsError, setTpsError] = useState<string | null>(null);
  const [props, setProps] = useState<{ values: Record<string, string>; modified: number | null } | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [undo, setUndo] = useState<Record<string, string> | null>(null);
  const [profileNote, setProfileNote] = useState<string | null>(null);
  const [profileError, setProfileError] = useState<string | null>(null);

  const online = state === "ONLINE";

  const load = useCallback(async () => {
    try {
      const [r, m] = await Promise.all([sup.health(id), sup.metrics(id)]);
      setH(r);
      const last = m.samples.at(-1);
      setProc(last ? { cpu: last.cpu_percent, mem: last.memory_bytes } : null);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  const loadProps = useCallback(async () => {
    try {
      const p = await sup.properties(id);
      setProps(p.exists ? { values: Object.fromEntries(p.entries.map((e) => [e.key, e.value])), modified: p.modified_unix_ms } : null);
    } catch {
      setProps(null);
    }
  }, [sup, id]);

  useEffect(() => {
    load();
    loadProps();
    const t = setInterval(load, 2000);
    return () => clearInterval(t);
  }, [load, loadProps]);

  async function measure() {
    setTpsBusy(true);
    setTpsError(null);
    try {
      setTps(await sup.measureTps(id));
    } catch (e) {
      setTpsError(errorMessage(e));
    } finally {
      setTpsBusy(false);
    }
  }

  async function applyValues(values: Record<string, string>, msg: string, keepUndo: boolean) {
    if (!props) return;
    setProfileError(null);
    const before = Object.fromEntries(Object.keys(values).map((k) => [k, props.values[k] ?? ""]));
    try {
      const r = await sup.setProperties(id, values, props.modified);
      setProps({ values: Object.fromEntries(r.entries.map((e) => [e.key, e.value])), modified: r.modified_unix_ms });
      setUndo(keepUndo ? before : null);
      setPreview(null);
      setProfileNote(msg + (online ? " Se aplica al reiniciar el servidor." : ""));
    } catch (e) {
      setProfileError(errorMessage(e));
    }
  }

  const memTotal = h?.pc_memory_total_bytes ?? null;
  const memUsed = h?.pc_memory_used_bytes ?? null;
  const xmx = (h?.max_memory_mb ?? 0) * 1024 * 1024;
  const shareWarn = memTotal !== null && xmx > memTotal * 0.5;
  const previewProfile = PROFILES.find((p) => p.id === preview) ?? null;
  const serverAlerts = alerts.filter((a) => a.instance_id === id).slice(0, 5);

  return (
    <div className="admin-panel health-panel" role="tabpanel" aria-label="Salud y rendimiento">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">Salud y rendimiento</h2>
          <p className="field-help">Todo lo que ves aquí está medido en esta PC; lo que no se puede medir se dice.</p>
        </div>
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <div className="health-grid">
        <section className="card health-card" aria-label="Esta PC">
          <h3>
            <Cpu size={16} aria-hidden="true" /> Esta PC
          </h3>
          {h === null ? (
            <p className="wiz-loading">Midiendo…</p>
          ) : (
            <>
              <p className="health-line">
                <span>Procesador</span>
                <strong>{h.pc_cpu_percent === null ? "midiendo…" : `${Math.round(h.pc_cpu_percent)} %`}</strong>
              </p>
              {h.pc_cpu_percent !== null && <Bar value={h.pc_cpu_percent} max={100} label="Uso del procesador" />}
              <p className="health-line">
                <span>Memoria</span>
                <strong>
                  {memUsed !== null && memTotal !== null
                    ? `${formatSize(memUsed)} de ${formatSize(memTotal)}`
                    : memTotal !== null
                      ? `${formatSize(memTotal)} en total`
                      : "no disponible"}
                </strong>
              </p>
              {memUsed !== null && memTotal !== null && <Bar value={memUsed} max={memTotal} label="Memoria usada de la PC" />}
              {h.disks.map((d) => (
                <div key={d.what}>
                  <p className="health-line">
                    <span>
                      <HardDrive size={13} aria-hidden="true" /> Disco {d.what === "server" ? "del servidor" : "de las copias"}
                    </span>
                    <strong>{formatSize(d.free_bytes)} libres</strong>
                  </p>
                  <Bar
                    value={d.total_bytes - d.free_bytes}
                    max={d.total_bytes}
                    label={`Disco ${d.what === "server" ? "del servidor" : "de las copias"} usado`}
                  />
                </div>
              ))}
            </>
          )}
        </section>

        <section className="card health-card" aria-label="Servidor">
          <h3>
            <MemoryStick size={16} aria-hidden="true" /> Servidor
          </h3>
          {proc && online ? (
            <>
              <p className="health-line">
                <span>Memoria del proceso</span>
                <strong>
                  {formatSize(proc.mem)} de {formatSize(xmx)} asignados
                </strong>
              </p>
              <Bar value={proc.mem} max={xmx || proc.mem} label="Memoria del servidor" />
              <p className="health-line">
                <span>Procesador (de toda la PC)</span>
                <strong>{Math.round(proc.cpu)} %</strong>
              </p>
              <p className="field-help">
                Java reserva memoria por adelantado: verla cerca del máximo no siempre significa que falte.
              </p>
            </>
          ) : (
            <p className="field-help">Enciende el servidor para ver su memoria y procesador. Tiene {formatSize(xmx)} asignados.</p>
          )}
        </section>

        <section className="card health-card" aria-label="TPS">
          <h3>
            <Gauge size={16} aria-hidden="true" /> TPS (ticks por segundo)
          </h3>
          <p className="field-help">20 es lo normal. Si baja, el juego va «a tirones» para todos.</p>
          {h && h.tps_command === null ? (
            <p className="field-help">
              Este tipo de servidor no informa su TPS (Vanilla y Fabric no tienen ese comando). No se muestra un número inventado.
            </p>
          ) : (
            <>
              <button type="button" className="btn btn-sm" disabled={!online || tpsBusy} onClick={measure}>
                <Activity size={14} aria-hidden="true" /> {tpsBusy ? "Midiendo…" : "Medir TPS"}
              </button>
              {!online && <p className="field-help">Enciende el servidor para medirlo.</p>}
              {tpsError && (
                <p className="form-error" role="alert">
                  {tpsError}
                </p>
              )}
              {tps && (
                <div className="tps-row" aria-label="Resultado del TPS">
                  {tps.tps.map(([period, v]) => (
                    <span key={period} className="tps-chip" data-level={tpsLevel(v)}>
                      <strong>{v.toFixed(1)}</strong> {period === "media" ? "media" : `últ. ${period}`}
                    </span>
                  ))}
                  {tps.mspt !== null && <span className="tps-chip">{tps.mspt.toFixed(1)} ms por tick</span>}
                  <p className="field-help">Medido con el comando «{tps.command}» del propio servidor.</p>
                </div>
              )}
            </>
          )}
        </section>
      </div>

      <section className="card props-card" aria-label="Jugar y hostear en la misma PC">
        <h2>Jugar y hostear en la misma PC</h2>
        {shareWarn ? (
          <p className="warn-text">
            <TriangleAlert size={14} aria-hidden="true" /> El servidor puede usar hasta {formatSize(xmx)} de los{" "}
            {formatSize(memTotal ?? 0)} de esta PC. Si además juegas aquí, el juego y el sistema pueden quedarse sin memoria: baja la
            memoria del servidor o usa el modo «Ligero».
          </p>
        ) : (
          <p className="field-help">
            El servidor y el juego compiten por el mismo procesador y la misma memoria. Si notas tirones al jugar aquí, prueba el
            modo «Ligero» y cierra programas que no uses.
          </p>
        )}
      </section>

      <section className="card props-card" aria-label="Modo de rendimiento">
        <h2>Modo de rendimiento</h2>
        <p className="field-help">
          Cambia cuánto ve y simula el servidor alrededor de cada jugador. Revisa los cambios antes de aplicarlos; se guarda una
          copia de server.properties y puedes deshacerlo.
        </p>
        {!props ? (
          <p className="field-help">Todavía no hay server.properties: enciende el servidor una vez.</p>
        ) : (
          <>
            <div className="seg" role="group" aria-label="Modos">
              {PROFILES.map((p) => (
                <button key={p.id} type="button" className="seg-btn" aria-pressed={preview === p.id} onClick={() => setPreview(p.id)}>
                  {p.label}
                </button>
              ))}
            </div>
            {previewProfile && (
              <div className="profile-preview" aria-label="Vista previa">
                <p className="field-help">{previewProfile.help}</p>
                <table className="diff-table">
                  <thead>
                    <tr>
                      <th>Opción</th>
                      <th>Ahora</th>
                      <th>Con «{previewProfile.label}»</th>
                    </tr>
                  </thead>
                  <tbody>
                    {Object.entries(previewProfile.values).map(([k, v]) => (
                      <tr key={k} data-changed={(props.values[k] ?? "") !== v}>
                        <td>{PROP_LABEL[k] ?? k}</td>
                        <td>{props.values[k] ?? "—"}</td>
                        <td>{v}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <div className="files-bar">
                  <button type="button" className="btn btn-quiet btn-sm" onClick={() => setPreview(null)}>
                    Cancelar
                  </button>
                  <button
                    type="button"
                    className="btn btn-go btn-sm"
                    disabled={Object.entries(previewProfile.values).every(([k, v]) => props.values[k] === v)}
                    onClick={() => applyValues(previewProfile.values, `Modo «${previewProfile.label}» aplicado.`, true)}
                  >
                    Aplicar «{previewProfile.label}»
                  </button>
                </div>
              </div>
            )}
            {undo && (
              <button
                type="button"
                className="btn btn-sm"
                onClick={() =>
                  applyValues(
                    Object.fromEntries(Object.entries(undo).filter(([, v]) => v !== "")),
                    "Se volvió a los valores anteriores.",
                    false,
                  )
                }
              >
                <RotateCcw size={14} aria-hidden="true" /> Deshacer el cambio
              </button>
            )}
            {profileError && (
              <p className="form-error" role="alert">
                {profileError}
              </p>
            )}
            {profileNote && !profileError && (
              <p className="field-help ok-text" role="status">
                {profileNote}
              </p>
            )}
          </>
        )}
      </section>

      <section className="card props-card" aria-label="Alertas de este servidor">
        <h2>Alertas de este servidor</h2>
        {serverAlerts.length === 0 ? (
          <p className="field-help">Sin alertas. Si el servidor se cae o falla una tarea, aparecerá aquí y en la campana.</p>
        ) : (
          <ul className="alert-list">
            {serverAlerts.map((a) => (
              <li key={a.id} data-level={a.level}>
                <strong>{a.title}</strong>
                <span className="field-help">
                  {new Date(a.at_unix_ms).toLocaleString(uiLocale(), { dateStyle: "medium", timeStyle: "short" })} · {a.message}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
