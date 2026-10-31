// Pestaña Actividad: lo que se hizo con este servidor, en orden cronológico (auditoría local).

import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { ActivityEntry, InstanceId } from "../api/types";
import { uiLocale } from "../i18n/lang";

const ACTION_TEXT: Record<string, string> = {
  "server.create": "Creación iniciada",
  "server.created": "Servidor creado",
  "server.create.cancel": "Creación cancelada",
  "server.start": "Encendido",
  "server.stop": "Detenido",
  "server.restart": "Reiniciado",
  "server.restart.notice": "Reinicio con aviso a los jugadores",
  "server.force_kill": "Cierre forzado",
  "server.command": "Comando en la consola",
  "server.eula.accept": "EULA aceptada",
  "server.properties.set": "Ajustes cambiados",
  "player.action": "Acción sobre un jugador",
  "file.write": "Archivo guardado",
  "file.mkdir": "Carpeta creada",
  "file.rename": "Renombrado",
  "file.trash": "Movido a la papelera",
  "file.upload": "Archivo subido",
  "file.download": "Copia descargada",
  "file.restore": "Restaurado",
  "file.purge": "Borrado definitivamente",
  "world.duplicate": "Mundo duplicado",
  "world.export": "Mundo exportado",
  "world.import": "Mundo importado",
  "world.activate": "Mundo activo cambiado",
  "world.trash": "Mundo a la papelera",
  "world.gamerule": "Regla del juego cambiada",
  "supervisor.recover": "Recuperado tras reiniciar MercHosting",
};

function who(actor: string): string {
  if (actor === "local:merchosting-app") return "Tú, desde la app";
  if (actor === "local:merc-cli") return "Herramienta merc";
  if (actor === "supervisor") return "MercHosting";
  return actor.replace(/^local:/, "");
}

export function ActivityPanel({ sup, id }: { sup: Supervisor; id: InstanceId }) {
  const [items, setItems] = useState<ActivityEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setItems(await sup.activity(id, 300));
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [sup, id]);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="admin-panel" role="tabpanel" aria-label="Actividad">
      <div className="files-bar">
        <h2 className="panel-title">Actividad</h2>
        <button type="button" className="btn btn-sm" onClick={load}>
          <RefreshCw size={14} aria-hidden="true" /> Actualizar
        </button>
      </div>
      <p className="field-help">Todo lo que se hace con este servidor queda anotado aquí, en tu PC.</p>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {items === null ? (
        <p className="wiz-loading" role="status">
          Cargando…
        </p>
      ) : items.length === 0 ? (
        <p className="field-help">Todavía no hay actividad.</p>
      ) : (
        <ol className="card activity-list">
          {items.map((a, i) => (
            <li key={`${a.at_unix}-${i}`}>
              <time dateTime={new Date(a.at_unix * 1000).toISOString()}>
                {new Date(a.at_unix * 1000).toLocaleString(uiLocale(), { dateStyle: "short", timeStyle: "medium" })}
              </time>
              <span className="activity-what">
                <strong>{ACTION_TEXT[a.action] ?? a.action}</strong>
                {a.detail && <span className="activity-detail">{a.detail}</span>}
              </span>
              <span className="activity-who">{who(a.actor)}</span>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}
