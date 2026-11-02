// Alertas de todos los servidores (caídas, poco disco, memoria llena, tareas o copias que fallan,
// túnel caído, actualizaciones) y sus ajustes, incluidas las notificaciones del sistema.

import { BellOff, CheckCheck } from "lucide-react";
import { useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { AlertList, AlertSettings } from "../api/types";
import { Dialog } from "./Dialog";
import { uiLocale } from "../i18n/lang";

const LEVEL: Record<string, string> = { info: "Info", warning: "Aviso", critical: "Grave" };

interface Props {
  sup: Supervisor;
  list: AlertList;
  onChange: (l: AlertList) => void;
  onClose: () => void;
}

export function AlertsDialog({ sup, list, onChange, onClose }: Props) {
  const [settings, setSettings] = useState<AlertSettings>(list.settings);
  const [gb, setGb] = useState(String(list.settings.disk_min_gb));
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  async function run(fn: () => Promise<AlertList>, msg?: string) {
    setError(null);
    setNote(null);
    try {
      const l = await fn();
      onChange(l);
      setSettings(l.settings);
      setGb(String(l.settings.disk_min_gb));
      if (msg) setNote(msg);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const gbNum = Number(gb);
  const gbValid = Number.isInteger(gbNum) && gbNum >= 1 && gbNum <= 1000;
  const dirty = JSON.stringify(settings) !== JSON.stringify(list.settings) || gbNum !== list.settings.disk_min_gb;
  const toggle = (k: keyof Omit<AlertSettings, "disk_min_gb">, label: string) => (
    <label className="switch-row">
      <input type="checkbox" checked={settings[k]} onChange={(e) => setSettings({ ...settings, [k]: e.target.checked })} />
      <span>{label}</span>
    </label>
  );

  return (
    <Dialog
      title="Alertas"
      lead="Lo vigila el supervisor aunque cierres la ventana. Las notificaciones del sistema salen mientras MercHosting está abierto (también minimizado)."
      onClose={onClose}
      wide
      footer={
        <button type="button" className="btn btn-quiet" onClick={onClose}>
          Cerrar
        </button>
      }
    >
      <div className="files-bar">
        <h3 className="panel-title">{list.unread > 0 ? `${list.unread} sin leer` : "Todo leído"}</h3>
        <button type="button" className="btn btn-sm" disabled={list.unread === 0} onClick={() => run(() => sup.markAlertsRead([]))}>
          <CheckCheck size={14} aria-hidden="true" /> Marcar todo como leído
        </button>
      </div>
      {list.items.length === 0 ? (
        <p className="field-help">
          <BellOff size={14} aria-hidden="true" /> Sin alertas por ahora.
        </p>
      ) : (
        <ul className="alert-list" aria-label="Lista de alertas">
          {list.items.slice(0, 50).map((a) => (
            <li key={a.id} data-level={a.level} data-read={a.read}>
              <strong>
                <span className="badge badge-muted">{LEVEL[a.level]}</span> {a.title}
              </strong>
              <span className="field-help">
                {new Date(a.at_unix_ms).toLocaleString(uiLocale(), { dateStyle: "medium", timeStyle: "short" })}
                {a.server_name ? ` · ${a.server_name}` : ""} · {a.message}
              </span>
            </li>
          ))}
        </ul>
      )}

      <section className="card props-card" aria-label="Ajustes de alertas">
        <h2>Qué vigilar</h2>
        {toggle("crash", "Un servidor se cierra inesperadamente")}
        {toggle("tasks", "Falla una tarea programada")}
        {toggle("backups", "Falla una copia de seguridad automática")}
        {toggle("tunnel", "El túnel de Playit deja de funcionar con un servidor encendido")}
        {toggle("updates", "Hay una actualización del software de un servidor")}
        {toggle("resources", "La memoria de la PC está casi llena")}
        {toggle("disk", "Queda poco espacio en el disco")}
        <div className="field prop-row">
          <label htmlFor="al-gb">Avisar si quedan menos de (GB)</label>
          <input id="al-gb" type="text" inputMode="numeric" value={gb} onChange={(e) => setGb(e.target.value)} aria-invalid={!gbValid} />
          {!gbValid && <p className="form-error">Un número entre 1 y 1000.</p>}
        </div>
        {toggle("notify", "Mostrar notificaciones del sistema")}
        <div className="files-bar">
          <button
            type="button"
            className="btn btn-go btn-sm"
            disabled={!dirty || !gbValid}
            onClick={() => run(() => sup.setAlertSettings({ ...settings, disk_min_gb: gbNum }), "Ajustes guardados.")}
          >
            Guardar ajustes
          </button>
        </div>
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
      </section>
    </Dialog>
  );
}
