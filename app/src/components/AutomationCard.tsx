// Ajustes › «Si se cae y al iniciar sesión»: reinicio automático tras una caída (con tope),
// encender el servidor al iniciar sesión (Windows o Linux) y no suspender la PC mientras haya un
// servidor encendido. Todo viene apagado; «Activar modo 24/7» enciende las cuatro cosas de una
// vez, con su explicación delante.

import { Moon, Power, RotateCcw, Timer } from "lucide-react";
import { useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { AutomationInfo, InstanceId } from "../api/types";
import { isLinux } from "../platform";

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

  const all247 = info.auto_restart && info.autostart && info.startup_enabled && info.keep_awake;

  async function enable247() {
    await run(async () => {
      if (info!.startup_available && !info!.startup_enabled) await sup.setStartup(true);
      if (!info!.keep_awake) await sup.setKeepAwake(true);
      return sup.setAutomation(id, { auto_restart: true, autostart: true });
    });
  }

  return (
    <section className="card props-card automation-card" aria-label="Si se cae y al iniciar sesión">
      <h2>Si se cae y al iniciar sesión</h2>
      <div className="mode247">
        <p>
          <Timer size={14} aria-hidden="true" /> <strong>Modo 24/7.</strong> El servidor está disponible mientras tu PC esté
          prendida: si se cae vuelve solo, enciende al iniciar sesión y la PC no se suspende sola.{" "}
          {all247 ? <span className="ok-text">Está activado para este servidor.</span> : null}
        </p>
        {!all247 && (
          <button type="button" className="btn btn-sm btn-go" disabled={busy || !info.startup_available} onClick={enable247}>
            Activar modo 24/7
          </button>
        )}
        <p className="field-help">
          No hace milagros: si apagas la PC, se va la luz o se corta el Internet, el servidor también se apaga.
        </p>
      </div>
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
            <Power size={14} aria-hidden="true" /> Arrancar MercHosting al iniciar sesión
          </strong>
          <span className="field-help">
            {info.startup_available
              ? "Al iniciar sesión arranca el supervisor en segundo plano (sin abrir ninguna ventana). Vale para todos tus servidores."
              : "No está disponible en este sistema."}
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
          <strong>Encender este servidor al iniciar sesión</strong>
          <span className="field-help">
            {info.startup_enabled
              ? "Se enciende solo al iniciar sesión (si tiene la EULA aceptada y no está archivado)."
              : "Necesita «Arrancar MercHosting al iniciar sesión» activado."}
          </span>
        </span>
      </label>
      <label className="switch-row">
        <input
          type="checkbox"
          checked={info.keep_awake}
          disabled={busy}
          onChange={(e) => run(() => sup.setKeepAwake(e.target.checked))}
        />
        <span>
          <strong>
            <Moon size={14} aria-hidden="true" /> No suspender la PC mientras haya un servidor encendido
          </strong>
          <span className="field-help">
            {isLinux()
              ? "Le pide al sistema (con systemd-inhibit) que no la suspenda. La pantalla sí se puede apagar; mientras esté activo, el escritorio puede avisarte de que MercHosting impide suspender. Vale para todos tus servidores."
              : "Le pide a Windows que no la duerma por inactividad. La pantalla sí se puede apagar, y apagar o suspender a mano sigue funcionando. Vale para todos tus servidores."}
            {info.keep_awake
              ? info.keep_awake_active
                ? " Ahora mismo está evitando la suspensión."
                : isLinux()
                  ? " Se activa cuando enciendes un servidor (si tu sistema tiene systemd-inhibit)."
                  : " Se activa cuando enciendes un servidor."
              : ""}
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
