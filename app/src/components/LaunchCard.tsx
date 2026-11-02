// Ajustes › Java y memoria: cambiar la memoria máxima y el Java del servidor (detenido).

import { Coffee } from "lucide-react";
import { useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId, JavaCandidate, LaunchProfile } from "../api/types";
import { formatMemory } from "../format";
import { Dialog } from "./Dialog";
import { JavaInstall } from "./JavaInstall";

const STEPS_GB = [1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32];

interface Props {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  launch: LaunchProfile;
  mcVersion: string | null;
  running: boolean;
  /** Servidor de un amigo: el Java es cosa del dueño. */
  remote?: boolean;
  onChanged: () => void;
}

export function LaunchCard({ api, sup, id, launch, mcVersion, running, remote = false, onChanged }: Props) {
  const [mem, setMem] = useState(launch.max_memory_mb);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [picker, setPicker] = useState<{ list: JavaCandidate[] | null; pick: string | null; need: number } | null>(null);
  const options = [...new Set([...STEPS_GB.map((g) => g * 1024), launch.max_memory_mb])].sort((a, b) => a - b);

  async function save(change: { java?: string; max_memory_mb?: number }, msg: string) {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      await sup.setLaunch(id, change);
      setNote(msg);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function openPicker() {
    setPicker({ list: null, pick: null, need: 0 });
    try {
      const c = await api.javaFor(mcVersion);
      const ok = c.candidates.filter((x) => x.major !== null && x.major >= c.recommended_major);
      setPicker({ list: ok, pick: c.recommended ?? ok[0]?.path ?? null, need: c.recommended_major });
    } catch (e) {
      setPicker(null);
      setError(errorMessage(e));
    }
  }

  return (
    <section className="card props-card" aria-label="Java y memoria">
      <h2>
        <Coffee size={16} aria-hidden="true" /> Java y memoria
      </h2>
      {running && <p className="field-help warn-text">Detén el servidor para cambiar el Java o la memoria.</p>}
      <div className="launch-row">
        <label className="field">
          <span className="field-title">Memoria máxima</span>
          <select value={mem} onChange={(e) => setMem(Number(e.target.value))} disabled={running || busy}>
            {options.map((mb) => (
              <option key={mb} value={mb}>
                {formatMemory(mb)}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="btn btn-sm btn-go"
          disabled={running || busy || mem === launch.max_memory_mb}
          onClick={() => save({ max_memory_mb: mem }, `Memoria máxima: ${formatMemory(mem)}. Se aplica al encender.`)}
        >
          Guardar memoria
        </button>
      </div>
      <p className="field-help">
        Más memoria ayuda con muchos mods o jugadores, pero deja suficiente para el sistema (y para el juego, si juegas en esta PC).
      </p>
      {!remote && (
        <div className="launch-row">
          <p className="launch-java">
            <span className="field-title">Java</span> <code>{launch.java}</code>
          </p>
          <button type="button" className="btn btn-sm" disabled={running || busy} onClick={openPicker}>
            Cambiar Java
          </button>
        </div>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {note && (
        <p className="ok-text" role="status">
          {note}
        </p>
      )}
      {picker && (
        <Dialog
          title="Elegir Java"
          lead={picker.need ? `Este servidor necesita Java ${picker.need} o más nuevo. Estos son los que hay en esta PC.` : "Buscando…"}
          onClose={() => setPicker(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setPicker(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                disabled={!picker.pick || picker.pick === launch.java}
                onClick={() => {
                  const pick = picker.pick!;
                  setPicker(null);
                  save({ java: pick }, "Java cambiado. Se usa desde el próximo arranque.");
                }}
              >
                Usar este Java
              </button>
            </>
          }
        >
          {picker.list === null ? (
            <p className="wiz-loading">Buscando Java en esta PC…</p>
          ) : picker.list.length === 0 ? (
            <>
              <p>No hay un Java compatible en esta PC.</p>
              <JavaInstall sup={sup} minMajor={picker.need} onInstalled={openPicker} />
            </>
          ) : (
            <fieldset className="java-list">
              <legend className="sr-only">Java</legend>
              {picker.list.map((c) => (
                <label key={c.path} className="check">
                  <input type="radio" name="java-launch" checked={picker.pick === c.path} onChange={() => setPicker({ ...picker, pick: c.path })} />
                  <span>
                    <strong>Java {c.major}</strong> <span className="field-help">{c.path}</span>
                    {c.path === launch.java && <span className="badge badge-muted"> en uso</span>}
                  </span>
                </label>
              ))}
            </fieldset>
          )}
        </Dialog>
      )}
    </section>
  );
}
