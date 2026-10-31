// Ajustes › «Si se cae y al iniciar Windows»: reinicio automático tras una caída (con tope) y
// encender el servidor cuando Windows inicia sesión. Las dos cosas vienen apagadas.

import { Power, RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { AutomationInfo, InstanceId } from "../api/types";

export function AutomationCard({ sup, id }: { sup: Supervisor; id: InstanceId }) {
  const [info, setInfo] = useState<AutomationInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    sup.automation(id).then(setInfo, (e) => setError(errorMessage(e)));
  }, [sup, id]);

  async function run(f: () => Promise<AutomationInfo | void>) {
    setBusy(true);
    setError(null);
    try {
      const r = await f();
      setInfo(r ?? (await sup.automation(id)));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  if (!info) return error ? <p className="form-error">{error}</p> : null;

  return (
    <section className="card props-card automation-card" aria-label="Si se cae y al iniciar Windows">
      <h2>Si se cae y al iniciar Windows</h2>
      <label className="switch-row">
        <input
          type="checkbox"
          checked={info.auto_restart}
          disabled={busy}
          onChange={(e) => run(() => sup.setAutomation(id, { auto_restart: e.target.checked }))}
        />
        <span>
          <strong>
            <RotateCcw size={14} aria-hidden="true" /> Reiniciar solo si se cae
          </strong>
          <span className="field-help">
            Espera 10, 30 y 60 segundos entre intentos. Si se cae {info.max_restarts} veces en 10 minutos, deja de intentarlo para no
            quedarse en bucle: algo está mal y conviene mirar el Diagnóstico.
          </span>
        </span>
      </label>
      {info.gave_up && (
        <p className="warn-text" role="status">
          Se reinició solo {info.max_restarts} veces y volvió a caer: ya no se reinicia solo hasta que lo enciendas tú.
        </p>
      )}

      <label className="switch-row">
        <input
          type="checkbox"
          checked={info.startup_enabled}
          disabled={busy || !info.startup_available}
          onChange={(e) => run(() => sup.setStartup(e.target.checked))}
        />
        <span>
          <strong>
            <Power size={14} aria-hidden="true" /> Arrancar MercHosting con Windows
          </strong>
          <span className="field-help">
            {info.startup_available
              ? "Al iniciar sesión en Windows arranca el supervisor en segundo plano (sin abrir ninguna ventana). Vale para todos tus servidores."
              : "Solo está disponible en Windows."}
          </span>
        </span>
      </label>
      <label className="switch-row">
        <input
          type="checkbox"
          checked={info.autostart}
          disabled={busy}
          onChange={(e) => run(() => sup.setAutomation(id, { autostart: e.target.checked }))}
        />
        <span>
          <strong>Encender este servidor al iniciar Windows</strong>
          <span className="field-help">
            {info.startup_enabled
              ? "Se enciende solo al iniciar sesión (si tiene la EULA aceptada y no está archivado)."
              : "Necesita «Arrancar MercHosting con Windows» activado."}
          </span>
        </span>
      </label>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
