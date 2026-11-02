import { FolderOpen } from "lucide-react";
import { useMemo, useState } from "react";
import type { MercApi } from "../api/client";
import { Supervisor, errorMessage } from "../api/client";
import type { DetectedServer, InstanceId, JavaChoice } from "../api/types";
import { folderName, formatMemory, KIND_LABEL } from "../format";
import { Dialog } from "./Dialog";

interface Props {
  api: MercApi;
  onClose: () => void;
  onImported: (id: InstanceId) => void;
}

const CONFIDENCE_TEXT = {
  verified: "Comprobado con los archivos del instalador",
  inferred: "Deducido por los nombres de los archivos; revisa que sea correcto",
  unknown: "No se pudo determinar",
} as const;

export function ImportDialog({ api, onClose, onImported }: Props) {
  const sup = useMemo(() => new Supervisor(api), [api]);
  const [path, setPath] = useState<string | null>(null);
  const [detected, setDetected] = useState<DetectedServer | null>(null);
  const [java, setJava] = useState<JavaChoice | null>(null);
  const [javaPath, setJavaPath] = useState("");
  const [name, setName] = useState("");
  const [ram, setRam] = useState(4096);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function chooseFolder() {
    setError(null);
    const picked = await api.pickFolder();
    if (!picked) return;
    setPath(picked);
    setDetected(null);
    setJava(null);
    setWorking(true);
    try {
      const det = await sup.detect(picked);
      setDetected(det);
      setName(folderName(picked));
      // Bedrock Dedicated Server es nativo: no necesita Java.
      if (det.launch && det.launch.type !== "native") {
        const choice = await api.javaFor(det.mc_version);
        setJava(choice);
        setJavaPath(choice.recommended ?? "");
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setWorking(false);
    }
  }

  const native = detected?.launch?.type === "native";

  async function submit() {
    if (!path || (!native && !javaPath)) return;
    setWorking(true);
    setError(null);
    try {
      const inst = await sup.importInstance({ name, path, java: native ? "" : javaPath, maxMemoryMb: native ? 0 : ram });
      onImported(inst.id);
    } catch (e) {
      setError(errorMessage(e));
      setWorking(false);
    }
  }

  const usableJava = java?.candidates.filter((c) => c.major !== null) ?? [];
  const canImport = !!detected?.launch && (native || !!javaPath) && name.trim().length > 0 && !working;

  return (
    <Dialog
      title="Importar un servidor"
      lead="Elige la carpeta de un servidor que ya tengas. MercHosting no mueve ni modifica sus archivos al importarlo."
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            Cancelar
          </button>
          <button type="button" className="btn btn-go" disabled={!canImport} onClick={submit}>
            Importar servidor
          </button>
        </>
      }
    >
      <div className="field" role="group" aria-labelledby="import-folder-title">
        <span id="import-folder-title" className="field-title">
          Carpeta del servidor
        </span>
        <div className="folder-pick">
          <button type="button" className="btn" onClick={chooseFolder} disabled={working}>
            <FolderOpen size={16} aria-hidden="true" />
            {path ? "Cambiar carpeta" : "Elegir carpeta"}
          </button>
          <span className="folder-path">{path ?? "Ninguna carpeta elegida"}</span>
        </div>
      </div>

      {working && !detected && (
        <p className="field-help" role="status">
          Analizando la carpeta…
        </p>
      )}

      {detected && (
        <div className="detect-box" aria-live="polite">
          {detected.launch ? (
            <>
              <h3>
                {KIND_LABEL[detected.kind]} {detected.mc_version ?? "(versión de Minecraft desconocida)"}
              </h3>
              <p className="field-help">{CONFIDENCE_TEXT[detected.confidence]}</p>
              <ul>
                {detected.loader_version && <li>Versión del loader: {detected.loader_version}</li>}
                {detected.evidence.map((e) => (
                  <li key={e}>{e}</li>
                ))}
                {detected.warnings.map((w) => (
                  <li key={w} className="warn">
                    {w}
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <>
              <h3>No se encontró un servidor en esta carpeta</h3>
              <ul>
                {detected.warnings.map((w) => (
                  <li key={w} className="warn">
                    {w}
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>
      )}

      {detected?.launch && (
        <>
          <div className="field">
            <label htmlFor="import-name">Nombre en MercHosting</label>
            <input id="import-name" type="text" value={name} maxLength={64} onChange={(e) => setName(e.target.value)} />
          </div>

          {native ? (
            <p className="field-help">Bedrock Dedicated Server es un programa de Mojang: no usa Java ni una memoria fija.</p>
          ) : (
          <div className="field">
            <label htmlFor="import-ram">Memoria máxima del servidor</label>
            <div className="ram-row">
              <input
                id="import-ram"
                type="range"
                min={1024}
                max={16384}
                step={512}
                value={ram}
                onChange={(e) => setRam(Number(e.target.value))}
              />
              <span className="ram-value">{formatMemory(ram)}</span>
            </div>
            <p className="field-help">
              Sale de la memoria de tu PC. Si también juegas en esta PC, deja suficiente para el sistema y para Minecraft.
            </p>
          </div>
          )}

          {java && (
            <div className="field">
              <label htmlFor="import-java">Java</label>
              {usableJava.length > 0 ? (
                <select id="import-java" value={javaPath} onChange={(e) => setJavaPath(e.target.value)}>
                  {!java.recommended && <option value="">Elige un Java</option>}
                  {usableJava.map((c) => (
                    <option key={c.path} value={c.path}>
                      Java {c.major}
                      {c.path === java.recommended ? " (recomendado)" : ""} en {c.path}
                    </option>
                  ))}
                </select>
              ) : null}
              {!java.recommended && (
                <p className="field-help" role="alert">
                  No hay un Java compatible instalado. Este servidor necesita Java {java.recommended_major}.
                  Instálalo (por ejemplo, Eclipse Temurin {java.recommended_major}) y vuelve a elegir la carpeta.
                </p>
              )}
            </div>
          )}
        </>
      )}

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
