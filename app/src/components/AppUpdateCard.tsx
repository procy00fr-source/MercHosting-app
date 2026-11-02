// Actualizar MercHosting: busca en las releases oficiales (GitHub), muestra las novedades y, si
// la persona lo pide, detiene los servidores guardando el mundo y abre el instalador firmado.

import { ArrowUpCircle, RefreshCw } from "lucide-react";
import { useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { AppUpdateCheck } from "../api/types";
import { hasProcess } from "../format";
import { Dialog } from "./Dialog";
import { RELEASES_URL } from "../about/links";

export function AppUpdateCard({ api, sup, initial }: { api: MercApi; sup: Supervisor; initial?: AppUpdateCheck | null }) {
  const [check, setCheck] = useState<AppUpdateCheck | null>(initial ?? null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<string[] | null>(null);
  const [progress, setProgress] = useState<number | null>(null);

  async function look() {
    setBusy("Buscando…");
    setError(null);
    try {
      setCheck(await api.appUpdateCheck());
    } catch (e) {
      setError(errorMessage(e));
    }
    setBusy(null);
  }

  async function running(): Promise<{ id: string; name: string }[]> {
    return (await sup.list()).filter((s) => hasProcess(s.status.state)).map((s) => ({ id: s.instance.id, name: s.instance.name }));
  }

  async function begin() {
    setError(null);
    try {
      const r = await running();
      setConfirm(r.map((s) => s.name));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function install() {
    setConfirm(null);
    setError(null);
    try {
      const list = await running();
      for (const s of list) {
        setBusy(`Deteniendo «${s.name}» (guardando el mundo)…`);
        await sup.stop(s.id);
      }
      const deadline = Date.now() + 120_000;
      while ((await running()).length > 0) {
        if (Date.now() > deadline) throw new Error("Un servidor no terminó de detenerse. Revísalo y vuelve a intentarlo.");
        await new Promise((r) => setTimeout(r, 500));
      }
      setBusy("Descargando y comprobando la firma…");
      setProgress(0);
      await api.appUpdateInstall((d, t) => setProgress(t ? Math.round((d / t) * 100) : null));
      setBusy("Abriendo el instalador. MercHosting se cerrará; ábrelo de nuevo si no se abre solo al terminar.");
    } catch (e) {
      setError(errorMessage(e));
      setBusy(null);
      setProgress(null);
    }
  }

  return (
    <section className="props-card" aria-label="Actualizar MercHosting">
      <h2>
        <ArrowUpCircle size={18} aria-hidden="true" /> Actualizar MercHosting
      </h2>
      <p className="field-help">
        Las versiones nuevas salen de la página oficial de releases en GitHub y van firmadas: si la firma no coincide, no se instala.
        Tus servidores, mundos y ajustes no se tocan.
      </p>
      {check && <p>Tienes la versión {check.current}.</p>}
      {check && !check.enabled && (
        <p className="field-help">
          Esta es una compilación de desarrollo: se actualiza compilando de nuevo. La versión instalada se actualiza desde aquí.
        </p>
      )}
      {check?.enabled && !check.available && <p className="field-help ok-text">Ya tienes la versión más nueva.</p>}
      {check?.available && (
        <div className="app-update-new">
          <p>
            <strong>MercHosting {check.version}</strong> está disponible.
          </p>
          {check.notes && <pre className="app-update-notes">{check.notes}</pre>}
          {check.self_install === false ? (
            <>
              <p className="field-help">
                Instalaste MercHosting con el paquete .deb, que no se actualiza solo: descarga el .deb nuevo de la página de
                releases e instálalo encima. Tus servidores y ajustes se conservan.
              </p>
              <button type="button" className="btn btn-go" onClick={() => api.openUrl(RELEASES_URL)}>
                Abrir la página de releases
              </button>
            </>
          ) : (
            <button type="button" className="btn btn-go" disabled={!!busy} onClick={begin}>
              Instalar la versión {check.version}
            </button>
          )}
        </div>
      )}
      {(!check || check.enabled) && (
        <button type="button" className="btn btn-sm btn-quiet" disabled={!!busy} onClick={look}>
          <RefreshCw size={14} aria-hidden="true" /> Buscar actualizaciones
        </button>
      )}
      {busy && (
        <p className="field-help" role="status">
          {busy}
          {progress !== null ? ` ${progress} %` : ""}
        </p>
      )}
      {progress !== null && <progress max={100} value={progress} aria-label="Descarga de la actualización" />}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {confirm && (
        <Dialog
          title={`Instalar MercHosting ${check?.version ?? ""}`}
          lead={
            confirm.length
              ? "Para cambiar los archivos de la app hay que detener los servidores. Se detienen guardando el mundo, como con el botón Detener."
              : "Se descarga, se comprueba la firma y se abre el instalador. MercHosting se cerrará mientras se instala."
          }
          onClose={() => setConfirm(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirm(null)}>
                Cancelar
              </button>
              <button type="button" className="btn btn-go" onClick={install}>
                {confirm.length ? "Detener y actualizar" : "Actualizar"}
              </button>
            </>
          }
        >
          {confirm.length > 0 && (
            <p>
              Se detendrán: <strong>{confirm.join(", ")}</strong>.
            </p>
          )}
        </Dialog>
      )}
    </section>
  );
}
