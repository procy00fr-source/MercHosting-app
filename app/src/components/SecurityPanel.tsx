// Pestaña Seguridad: comprobaciones reales del servidor (cuentas oficiales, exposición a
// Internet, lista blanca, RCON, compañeros, copias y claves), cada una con su explicación y,
// cuando se puede, un arreglo a un clic (confirmado).

import { CheckCircle2, Info, RefreshCw, ShieldAlert, ShieldCheck, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId, SecurityAction, SecurityCheck, SecurityLevel, SecurityReport } from "../api/types";
import { Dialog } from "./Dialog";

const LEVEL: Record<SecurityLevel, { label: string; icon: React.ReactNode }> = {
  peligro: { label: "Peligro", icon: <ShieldAlert size={18} aria-hidden="true" /> },
  aviso: { label: "Revisar", icon: <TriangleAlert size={18} aria-hidden="true" /> },
  info: { label: "Para saber", icon: <Info size={18} aria-hidden="true" /> },
  bien: { label: "Bien", icon: <CheckCircle2 size={18} aria-hidden="true" /> },
};

const TAB_ACTION: Record<string, string> = {
  players: "Abrir Jugadores",
  backups: "Abrir copias de seguridad",
  tasks: "Programar copias",
  activity: "Ver la actividad",
  files: "Abrir archivos",
};

function actionLabel(a: SecurityAction): string {
  switch (a.type) {
    case "set_property":
      return `Cambiar ${a.key} a ${a.value}`;
    case "open_tab":
      return TAB_ACTION[a.tab] ?? "Abrir";
    case "open_companions":
      return "Revisar compañeros";
  }
}

interface Props {
  sup: Supervisor;
  id: InstanceId;
  running: boolean;
  onOpenTab: (tab: string) => void;
  onOpenCompanions?: () => void;
}

export function SecurityPanel({ sup, id, running, onOpenTab, onOpenCompanions }: Props) {
  const [report, setReport] = useState<SecurityReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<SecurityCheck | null>(null);

  const load = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setReport(await sup.securityReport(id));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }, [sup, id]);

  useEffect(() => {
    load();
  }, [load]);

  async function act(c: SecurityCheck) {
    const a = c.action;
    if (!a) return;
    if (a.type === "open_tab") return onOpenTab(a.tab);
    if (a.type === "open_companions") return onOpenCompanions?.();
    setConfirm(c);
  }

  async function apply(a: Extract<SecurityAction, { type: "set_property" }>) {
    setConfirm(null);
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const props = await sup.properties(id);
      await sup.setProperties(id, { [a.key]: a.value }, props.modified_unix_ms);
      setNote(running ? `Guardado. Reinicia el servidor para aplicar ${a.key}.` : `Guardado: ${a.key}=${a.value}.`);
      setReport(await sup.securityReport(id));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const counts = report
    ? (["peligro", "aviso"] as const).map((l) => report.checks.filter((c) => c.level === l).length)
    : [0, 0];

  return (
    <div className="admin-panel security-panel" role="tabpanel" aria-label="Seguridad">
      <div className="files-bar">
        <div className="content-head">
          <h2 className="panel-title">
            <ShieldCheck size={18} aria-hidden="true" /> Seguridad
          </h2>
          <p className="field-help">Lo que se puede comprobar de verdad desde tu PC. No sustituye al sentido común: comparte la dirección solo con gente de confianza.</p>
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
      {report && (
        <>
          <p className="field-help" role="status">
            {counts[0] + counts[1] === 0
              ? "No hay nada urgente."
              : `${counts[0] > 0 ? `${counts[0]} en peligro` : ""}${counts[0] > 0 && counts[1] > 0 ? " · " : ""}${counts[1] > 0 ? `${counts[1]} para revisar` : ""}.`}
          </p>
          <ul className="finding-list" aria-label="Comprobaciones de seguridad">
            {report.checks.map((c, i) => (
              <li key={`${c.code}-${i}`} className="card props-card security-check" data-level={c.level}>
                <div className="finding-head">
                  {LEVEL[c.level].icon}
                  <h3>{c.title}</h3>
                  <span className="badge finding-badge">{LEVEL[c.level].label}</span>
                </div>
                <p>{c.detail}</p>
                {c.action && (c.action.type !== "open_companions" || onOpenCompanions) && (
                  <div className="notice-actions">
                    <button type="button" className={c.level === "peligro" ? "btn btn-sm btn-go" : "btn btn-sm"} disabled={busy} onClick={() => act(c)}>
                      {actionLabel(c.action)}
                    </button>
                  </div>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
      {confirm && confirm.action?.type === "set_property" && (
        <Dialog
          title={actionLabel(confirm.action)}
          lead={`Se cambia ${confirm.action.key} en server.properties (MercHosting guarda antes una copia del archivo).${running ? " Se aplica al reiniciar el servidor." : ""}`}
          onClose={() => setConfirm(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirm(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                onClick={() => apply(confirm.action as Extract<SecurityAction, { type: "set_property" }>)}
              >
                Aplicar
              </button>
            </>
          }
        />
      )}
    </div>
  );
}
