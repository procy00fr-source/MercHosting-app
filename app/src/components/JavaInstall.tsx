// Botón «Descargar Java»: baja Eclipse Temurin (JRE) de la API oficial de Adoptium, comprueba su
// SHA-256 y lo deja en la carpeta de MercHosting (no instala nada en el sistema).

import { Download } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { JobSnapshot } from "../api/types";
import { formatSize } from "../format";

/** Versiones LTS de Temurin: se usa la primera que cumple lo pedido. */
const LTS = [8, 11, 17, 21, 25];

export function temurinFor(minMajor: number): number {
  return LTS.find((v) => v >= minMajor) ?? minMajor;
}

interface Props {
  sup: Supervisor;
  minMajor: number;
  onInstalled: () => void;
  pollMs?: number;
}

export function JavaInstall({ sup, minMajor, onInstalled, pollMs = 700 }: Props) {
  const major = temurinFor(minMajor);
  const [job, setJob] = useState<JobSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  useEffect(
    () => () => {
      alive.current = false;
    },
    [],
  );

  async function start() {
    setError(null);
    try {
      let j = await sup.installJava(major);
      setJob(j);
      while (alive.current && j.state === "running") {
        await new Promise((r) => setTimeout(r, pollMs));
        j = await sup.job(j.job_id, 0);
        if (alive.current) setJob(j);
      }
      if (j.state === "succeeded") onInstalled();
      else if (j.state === "failed") setError(j.error ?? "No se pudo instalar Java.");
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const running = job?.state === "running";
  return (
    <div className="java-install">
      <button type="button" className="btn btn-sm btn-go" disabled={running} onClick={start}>
        <Download size={14} aria-hidden="true" /> {running ? "Descargando…" : `Descargar Java ${major} (Eclipse Temurin)`}
      </button>
      {running && job && (
        <p className="field-help" role="status">
          {job.message}
          {job.download ? ` · ${formatSize(job.download.done)}${job.download.total ? ` de ${formatSize(job.download.total)}` : ""}` : ""}
        </p>
      )}
      {job?.state === "succeeded" && (
        <p className="ok-text" role="status">
          {job.message}. Ya aparece en la lista.
        </p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <p className="field-help">
        Gratis, de la fuente oficial (Adoptium). Se comprueba su huella SHA-256 y queda en la carpeta de MercHosting: no se instala
        nada en el sistema.
      </p>
    </div>
  );
}
