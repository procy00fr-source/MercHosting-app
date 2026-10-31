// «Apoyar MercHosting»: Supporter Edition, US$8 de pago único y solo cosmético. La compra se
// hace en la página oficial de Gumroad; aquí solo se comprueba la clave de licencia que llega por
// correo. MercHosting nunca ve ni guarda datos de pago.

import { BadgeCheck, Check, ExternalLink, Heart, KeyRound, Minus, RefreshCw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { MercApi, Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { SupporterInfo } from "../api/types";
import credits from "../supporterCredits.json";

/** Redes oficiales del creador (enlaces reales dados por el propietario). */
export const SOCIALS = [
  { name: "YouTube", handle: "@Amercson", url: "https://www.youtube.com/@Amercson" },
  { name: "TikTok", handle: "@amercfr", url: "https://www.tiktok.com/@amercfr" },
];

const ROWS: { what: string; free: string | boolean; supporter: string | boolean }[] = [
  { what: "Servidores, mods, plugins, modpacks y datapacks", free: "Todo", supporter: "Todo" },
  { what: "Copias, tareas, diagnóstico, seguridad y actualizaciones", free: true, supporter: true },
  { what: "Playit, chat de voz y compañeros", free: true, supporter: true },
  { what: "Más RAM, servidores, compañeros o velocidad", free: false, supporter: false },
  { what: "Temas exclusivos (Esmeralda, Nether, Amatista, Glaciar)", free: false, supporter: true },
  { what: "Fondos extra (Aurora y Bloques)", free: false, supporter: true },
  { what: "Insignia «Colaborador» en la barra", free: false, supporter: true },
  { what: "Animación al encender un servidor (se puede apagar)", free: false, supporter: true },
];

function Cell({ v }: { v: string | boolean }) {
  if (typeof v === "string") return <>{v}</>;
  return v ? <Check size={16} aria-label="Sí" /> : <Minus size={16} aria-label="No" />;
}

interface Props {
  api: MercApi;
  sup: Supervisor;
  onChanged: (s: SupporterInfo) => void;
}

export function SupporterPage({ api, sup, onChanged }: Props) {
  const [info, setInfo] = useState<SupporterInfo | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const update = useCallback(
    (s: SupporterInfo) => {
      setInfo(s);
      onChanged(s);
    },
    [onChanged],
  );

  useEffect(() => {
    sup.supporter().then(update, (e) => setError(errorMessage(e)));
  }, [sup, update]);

  async function run(f: () => Promise<SupporterInfo>, ok?: (s: SupporterInfo) => string | null) {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const s = await f();
      update(s);
      setNote(ok?.(s) ?? null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const canBuy = !!info?.available && !!info.checkout_url;

  return (
    <div className="admin-panel companions-page supporter-page" role="region" aria-label="Apoyar MercHosting">
      <header className="page-head">
        <div>
          <p className="page-eyebrow">Supporter Edition</p>
          <h1 className="page-title">Apoyar MercHosting</h1>
          <p className="page-sub">
          MercHosting es gratuito porque creemos que todos deberían poder crear su propio servidor de Minecraft. Si disfrutas la
          aplicación y quieres apoyar su desarrollo, puedes hacerlo con un único pago de US$8. Sin suscripciones, sin funciones
          esenciales bloqueadas y sin obligaciones.
          </p>
        </div>
      </header>

      {info?.active && (
        <section className="props-card supporter-status" aria-label="Tu Supporter">
          <h2>
            <BadgeCheck size={18} aria-hidden="true" /> Eres Supporter. ¡Gracias!
          </h2>
          <p className="field-help">
            Compra {info.purchased ? `del ${info.purchased}` : ""}
            {info.email_hint ? ` con ${info.email_hint}` : ""} · clave {info.key_hint}
            {info.checked_at_unix_ms ? ` · comprobada el ${new Date(info.checked_at_unix_ms).toLocaleDateString()}` : ""}
          </p>
          {info.last_error && <p className="warn-text">No se pudo comprobar la última vez ({info.last_error}). Tus extras siguen activos.</p>}
          <p className="field-help">Los extras están en Apariencia. En otra PC usa la misma clave.</p>
          <div className="row-actions">
            <button type="button" className="btn btn-sm btn-quiet" disabled={busy} onClick={() => run(() => sup.refreshSupporter(), (s) => (s.active ? "Clave comprobada: todo en orden." : null))}>
              <RefreshCw size={14} aria-hidden="true" /> Comprobar ahora
            </button>
            <button type="button" className="btn btn-sm btn-quiet" disabled={busy} onClick={() => run(() => sup.forgetSupporter(), () => "Clave quitada de esta PC. Puedes volver a ponerla cuando quieras.")}>
              <Trash2 size={14} aria-hidden="true" /> Quitar de esta PC
            </button>
          </div>
        </section>
      )}

      {info && !info.active && info.revoked_reason && (
        <p className="warn-text" role="status">
          Los extras se desactivaron: {info.revoked_reason}
        </p>
      )}

      <section className="props-card" aria-label="Gratis y Supporter">
        <h2>Gratis y Supporter</h2>
        <table className="compare-table">
          <thead>
            <tr>
              <th scope="col">Qué incluye</th>
              <th scope="col">Gratis</th>
              <th scope="col">Supporter</th>
            </tr>
          </thead>
          <tbody>
            {ROWS.map((r) => (
              <tr key={r.what}>
                <th scope="row">{r.what}</th>
                <td>
                  <Cell v={r.free} />
                </td>
                <td>
                  <Cell v={r.supporter} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {!info?.active && (
          <div className="buy-row">
            <button type="button" className="btn btn-lg btn-go" disabled={!canBuy} onClick={() => info?.checkout_url && api.openUrl(info.checkout_url)}>
              <Heart size={16} aria-hidden="true" /> {canBuy ? "Apoyar por US$8" : "Próximamente"}
            </button>
            <span className="field-help">
              {canBuy ? "Pago único. Se abre la página oficial de Gumroad en tu navegador." : "La tienda todavía no está abierta. MercHosting sigue completo y gratis."}
            </span>
          </div>
        )}
      </section>

      {!info?.active && (
        <section className="props-card" aria-label="Activar con tu clave">
          <h2>
            <KeyRound size={18} aria-hidden="true" /> ¿Ya compraste?
          </h2>
          <p className="field-help">Pega la clave de licencia que te llegó en el correo de Gumroad. Sirve también para restaurar la compra en otra PC.</p>
          <form
            className="key-row"
            onSubmit={(e) => {
              e.preventDefault();
              run(() => sup.activateSupporter(key), () => "¡Listo! Supporter activado. Mira los extras en Apariencia.").then(() => setKey(""));
            }}
          >
            <label htmlFor="supporter-key" className="sr-only">
              Clave de licencia
            </label>
            <input
              id="supporter-key"
              className="mono"
              placeholder="XXXXXXXX-XXXXXXXX-XXXXXXXX-XXXXXXXX"
              value={key}
              autoComplete="off"
              spellCheck={false}
              disabled={busy || !info?.available}
              onChange={(e) => setKey(e.target.value)}
            />
            <button type="submit" className="btn btn-go" disabled={busy || !info?.available || key.trim().length < 8}>
              {busy ? "Comprobando…" : "Activar"}
            </button>
          </form>
        </section>
      )}

      {note && (
        <p className="ok-text" role="status">
          {note}
        </p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <section className="props-card" aria-label="Cómo funciona el pago">
        <h2>Cómo funciona</h2>
        <ul className="plain-list">
          <li>El pago lo procesa Gumroad, que es el vendedor oficial. MercHosting nunca ve ni guarda tu tarjeta.</li>
          <li>No hay cuenta de MercHosting: tu compra es la clave de licencia de tu correo.</li>
          <li>Reembolsos y comprobantes se piden a Gumroad. Si una compra se reembolsa, los extras se quitan en la siguiente comprobación.</li>
          <li>Sin internet tus extras siguen. La clave se vuelve a comprobar cada cierto tiempo cuando hay conexión.</li>
          <li>Comprar no da más RAM, servidores, compañeros ni mejores túneles: todo lo importante ya es gratis.</li>
        </ul>
        <h3>A dónde va tu apoyo</h3>
        <p className="field-help">
          Al desarrollo de MercHosting, a la firma digital del instalador cuando sea viable, al mantenimiento, la documentación y la
          distribución. Un pago único no puede financiar para siempre servicios de terceros, así que no lo prometemos.
        </p>
      </section>

      <section className="props-card" aria-label="Gracias a">
        <h2>Gracias a</h2>
        {credits.names.length > 0 ? (
          <ul className="credit-names">
            {credits.names.map((n: string) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        ) : (
          <p className="field-help">Todavía no hay nombres aquí.</p>
        )}
        <p className="field-help">
          Solo aparece quien lo pide: al comprar en Gumroad puedes escribir, si quieres, el nombre público con el que salir aquí. Si
          lo dejas vacío, no se publica nada.
        </p>
      </section>

      <section className="props-card" aria-label="Redes del creador">
        <h2>Sigue el proyecto</h2>
        <div className="row-actions">
          {SOCIALS.map((s) => (
            <button key={s.name} type="button" className="btn btn-sm btn-quiet" onClick={() => api.openUrl(s.url)}>
              <ExternalLink size={14} aria-hidden="true" /> {s.name} {s.handle}
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}
