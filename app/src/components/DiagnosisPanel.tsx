// Pestaña Diagnóstico: problemas conocidos en mods, plugins, Java y registros del servidor.
// Cada hallazgo muestra la evidencia real (la línea del registro o el archivo) y su nivel de
// confianza. Los arreglos son reversibles y se confirman antes de aplicarlos.

import { FileWarning, RefreshCw, Stethoscope } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { Diagnosis, FixAction, InstanceId, JavaCandidate, Perm } from "../api/types";
import { formatMemory } from "../format";
import { Dialog } from "./Dialog";
import { JavaInstall } from "./JavaInstall";

const CONFIDENCE_LABEL = { alta: "Confianza alta", media: "Confianza media", baja: "Confianza baja" } as const;
const TAB_LABEL: Record<string, string> = { backups: "Abrir copias de seguridad" };

interface Props {
  api: MercApi;
  sup: Supervisor;
  id: InstanceId;
  running: boolean;
  mcVersion: string | null;
  /** Permisos (en servidores propios, todo). */
  can?: (p: Perm) => boolean;
  /** Servidor de un amigo: elegir Java es cosa del dueño. */
  remote?: boolean;
  onOpenTab: (tab: string, query?: string) => void;
}

function actionLabel(a: FixAction): string {
  switch (a.type) {
    case "disable_content":
      return `Desactivar ${a.file}`;
    case "set_port":
      return `Usar el puerto ${a.port}`;
    case "set_memory":
      return `Subir la memoria a ${formatMemory(a.max_mb)}`;
    case "choose_java":
      return `Elegir Java ${a.min_major} o más nuevo`;
    case "search_content":
      return `Buscar «${a.query}»`;
    case "open_tab":
      return TAB_LABEL[a.tab] ?? "Abrir";
  }
}

export function DiagnosisPanel({ api, sup, id, running, mcVersion, can = () => true, remote = false, onOpenTab }: Props) {
  const [diag, setDiag] = useState<Diagnosis | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<FixAction | null>(null);
  const [java, setJava] = useState<{ min: number; list: JavaCandidate[] | null; pick: string | null } | null>(null);

  const load = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setDiag(await sup.diagnose(id));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }, [sup, id]);

  useEffect(() => {
    load();
  }, [load]);

  /** ¿Se puede ofrecer este arreglo aquí? (permisos y servidor detenido cuando hace falta) */
  function available(a: FixAction): { ok: boolean; why?: string } {
    switch (a.type) {
      case "disable_content":
      case "search_content":
        return can("content") ? (a.type === "disable_content" && running ? { ok: false, why: "Detén el servidor primero" } : { ok: true }) : { ok: false };
      case "set_port":
        return can("settings") ? (running ? { ok: false, why: "Detén el servidor primero" } : { ok: true }) : { ok: false };
      case "set_memory":
        return can("settings") ? (running ? { ok: false, why: "Detén el servidor primero" } : { ok: true }) : { ok: false };
      case "choose_java":
        return remote ? { ok: false } : running ? { ok: false, why: "Detén el servidor primero" } : { ok: true };
      case "open_tab":
        return { ok: can(a.tab === "backups" ? "backups" : "status") };
    }
  }

  async function act(a: FixAction) {
    if (a.type === "search_content") return onOpenTab(a.kind, a.query);
    if (a.type === "open_tab") return onOpenTab(a.tab);
    if (a.type === "choose_java") {
      setJava({ min: a.min_major, list: null, pick: null });
      try {
        const choice = await api.javaFor(mcVersion);
        const ok = choice.candidates.filter((c) => (c.major ?? 0) >= a.min_major);
        setJava({ min: a.min_major, list: ok, pick: ok[0]?.path ?? null });
      } catch (e) {
        setJava(null);
        setError(errorMessage(e));
      }
      return;
    }
    setConfirm(a);
  }

  async function apply(a: FixAction) {
    setConfirm(null);
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      if (a.type === "disable_content") {
        await sup.contentToggle(id, a.kind, a.file, false);
        setNote(`${a.file} desactivado. Puedes volver a activarlo en ${a.kind === "mod" ? "Mods" : "Plugins"}.`);
      } else if (a.type === "set_port") {
        const props = await sup.properties(id);
        await sup.setProperties(id, { "server-port": String(a.port) }, props.modified_unix_ms);
        setNote(`El servidor usará el puerto ${a.port}. Si usas Playit, revisa la Conexión.`);
      } else if (a.type === "set_memory") {
        await sup.setLaunch(id, { max_memory_mb: a.max_mb });
        setNote(`Memoria máxima: ${formatMemory(a.max_mb)}.`);
      }
      setDiag(await sup.diagnose(id));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="admin-panel diagnosis-panel" role="tabpanel" aria-label="Diagnóstico">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">
            <Stethoscope size={18} aria-hidden="true" /> Diagnóstico
          </h2>
          <p className="field-help">
            Busca problemas conocidos en tus mods y plugins, en el Java y en lo que escribió el servidor. Solo muestra lo que tiene
            una prueba.
          </p>
        </div>
        <button type="button" className="btn btn-sm" onClick={load} disabled={busy}>
          <RefreshCw size={14} aria-hidden="true" /> {busy ? "Revisando…" : "Revisar de nuevo"}
        </button>
      </div>
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
      {diag && (
        <>
          <p className="field-help">Se revisó: {diag.sources.length > 0 ? diag.sources.join(" · ") : "nada todavía (no hay mods ni registros)"}.</p>
          {diag.findings.length === 0 ? (
            <section className="card props-card">
              <p className="ok-text">No se encontraron problemas conocidos.</p>
              <p className="field-help">
                Eso no garantiza que todo esté bien: solo se buscan errores conocidos con una prueba clara. Si el servidor falla,
                vuelve a revisar justo después del fallo.
              </p>
            </section>
          ) : (
            <ul className="finding-list" aria-label="Problemas encontrados">
              {diag.findings.map((f, i) => (
                <li key={`${f.code}-${i}`} className="card props-card finding" data-confidence={f.confidence}>
                  <div className="finding-head">
                    <FileWarning size={18} aria-hidden="true" />
                    <h3>{f.title}</h3>
                    <span className="badge finding-badge">{CONFIDENCE_LABEL[f.confidence]}</span>
                  </div>
                  <p>{f.explanation}</p>
                  <details open={f.evidence.length <= 2}>
                    <summary className="field-help">Prueba ({f.source})</summary>
                    <pre className="evidence">{f.evidence.join("\n")}</pre>
                  </details>
                  {f.actions.length > 0 && (
                    <div className="notice-actions">
                      {f.actions.map((a, j) => {
                        const av = available(a);
                        if (!av.ok && !av.why) return null;
                        return (
                          <button
                            key={j}
                            type="button"
                            className={j === 0 ? "btn btn-sm btn-go" : "btn btn-sm"}
                            disabled={busy || !av.ok}
                            title={av.why}
                            onClick={() => act(a)}
                          >
                            {actionLabel(a)}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      {confirm && (
        <Dialog
          title={actionLabel(confirm)}
          lead={
            confirm.type === "disable_content"
              ? `Se renombra a «${confirm.file}.disabled»: no se borra nada y puedes volver a activarlo cuando quieras.`
              : confirm.type === "set_port"
                ? `Se cambia server-port a ${confirm.port} en server.properties (MercHosting guarda una copia del archivo antes).`
                : confirm.type === "set_memory"
                  ? `El servidor podrá usar hasta ${formatMemory(confirm.max_mb)}. Se aplica la próxima vez que lo enciendas; puedes volver a cambiarlo en Ajustes.`
                  : ""
          }
          onClose={() => setConfirm(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirm(null)}>
                Cancelar
              </button>
              <button type="button" className="btn btn-go" onClick={() => apply(confirm)}>
                Aplicar
              </button>
            </>
          }
        />
      )}

      {java && (
        <Dialog
          title={`Elegir Java ${java.min} o más nuevo`}
          lead="Estos son los Java de esta PC que sirven. El servidor usará el que elijas desde el próximo arranque."
          onClose={() => setJava(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setJava(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                disabled={!java.pick || busy}
                onClick={async () => {
                  const pick = java.pick!;
                  setJava(null);
                  setBusy(true);
                  try {
                    await sup.setLaunch(id, { java: pick });
                    setNote("Java cambiado.");
                    setDiag(await sup.diagnose(id));
                  } catch (e) {
                    setError(errorMessage(e));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Usar este Java
              </button>
            </>
          }
        >
          {java.list === null ? (
            <p className="wiz-loading">Buscando Java en esta PC…</p>
          ) : java.list.length === 0 ? (
            <>
              <p>No hay un Java {java.min} o más nuevo en esta PC.</p>
              <JavaInstall
                sup={sup}
                minMajor={java.min}
                onInstalled={() => act({ type: "choose_java", min_major: java.min })}
              />
            </>
          ) : (
            <fieldset className="java-list">
              <legend className="sr-only">Java</legend>
              {java.list.map((c) => (
                <label key={c.path} className="check">
                  <input type="radio" name="java" checked={java.pick === c.path} onChange={() => setJava({ ...java, pick: c.path })} />
                  <span>
                    <strong>Java {c.major}</strong> <span className="field-help">{c.path}</span>
                  </span>
                </label>
              ))}
            </fieldset>
          )}
        </Dialog>
      )}
    </div>
  );
}
