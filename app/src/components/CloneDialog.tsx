// Clonar un servidor: copia completa (mundo, mods, ajustes) a una carpeta nueva junto a la
// original, con otro puerto para poder tener los dos encendidos. Los registros no se copian.

import { Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { Instance, JobSnapshot } from "../api/types";
import { formatSize } from "../format";
import { Dialog } from "./Dialog";

interface Props {
  sup: Supervisor;
  inst: Instance;
  onClose: () => void;
  onCloned: (id: string) => void;
  pollMs?: number;
}

export function CloneDialog({ sup, inst, onClose, onCloned, pollMs = 500 }: Props) {
  const [name, setName] = useState(`${inst.name} (copia)`);
  const [job, setJob] = useState<JobSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  useEffect(
    () => () => {
      alive.current = false;
    },
    [],
  );
  const running = job?.state === "running";

  async function start() {
    setError(null);
    try {
      let j = await sup.cloneServer(inst.id, name.trim());
      setJob(j);
      while (alive.current && j.state === "running") {
        await new Promise((r) => setTimeout(r, pollMs));
        j = await sup.job(j.job_id, 0);
        if (alive.current) setJob(j);
      }
      if (j.state === "succeeded" && j.instance_id) onCloned(j.instance_id);
      else if (j.state !== "succeeded") setError(j.error ?? j.message);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <Dialog
      title={`Clonar «${inst.name}»`}
      lead="Se copia todo (mundo, mods, plugins y ajustes) a una carpeta nueva junto a la original. La copia usa otro puerto para que puedan estar encendidos a la vez. Los registros no se copian."
      onClose={() => !running && onClose()}
      footer={
        <>
          <button type="button" className="btn btn-quiet" disabled={running} onClick={onClose}>
            Cancelar
          </button>
          {running ? (
            <button type="button" className="btn" onClick={() => job && sup.cancelJob(job.job_id)}>
              Detener la copia
            </button>
          ) : (
            <button type="button" className="btn btn-go" disabled={!name.trim()} onClick={start}>
              <Copy size={15} aria-hidden="true" /> Clonar
            </button>
          )}
        </>
      }
    >
      <div className="field">
        <label htmlFor="clone-name">Nombre de la copia</label>
        <input id="clone-name" type="text" maxLength={64} value={name} disabled={running} onChange={(e) => setName(e.target.value)} />
      </div>
      {running && job && (
        <p className="field-help" role="status">
          {job.message}
          {job.download ? ` · ${formatSize(job.download.done)}${job.download.total ? ` de ${formatSize(job.download.total)}` : ""}` : ""}
        </p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
