// Comentarios para el desarrollador: ideas, errores o lo que sea. Se abre el formulario oficial
// ya rellenado; nada se envía hasta que la persona pulsa «Enviar» en él.

import { ExternalLink, MessageSquare } from "lucide-react";
import { useState } from "react";
import type { MercApi } from "../api/client";
import { errorMessage } from "../api/client";
import { kindLabel, MAX_MESSAGE, prefilledUrl, type FeedbackForm } from "../feedback";

export function FeedbackPage({ api, form, version }: { api: MercApi; form: FeedbackForm; version: string | null }) {
  const [kind, setKind] = useState(form.kinds[0]);
  const [message, setMessage] = useState("");
  const [contact, setContact] = useState("");
  const [attach, setAttach] = useState(true);
  const [opened, setOpened] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function open() {
    setError(null);
    try {
      await api.openUrl(prefilledUrl(form, { kind, message: message.trim(), contact, version: attach ? version : null }));
      setOpened(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <div className="admin-panel companions-page" role="region" aria-label="Comentarios">
      <header className="page-head">
        <div>
          <p className="page-eyebrow">MercHosting</p>
          <h1 className="page-title">Comentarios</h1>
          <p className="page-sub">Cuéntale al desarrollador una idea, un error o lo que quieras. Lo lee él mismo.</p>
        </div>
      </header>
      <section className="props-card" aria-label="Escribir un comentario">
        <h2>
          <MessageSquare size={18} aria-hidden="true" /> Tu comentario
        </h2>
        <div className="field">
          <label htmlFor="fb-kind">Tipo</label>
          <select id="fb-kind" value={kind} onChange={(e) => setKind(e.target.value)}>
            {form.kinds.map((k) => (
              <option key={k} value={k}>
                {kindLabel(k)}
              </option>
            ))}
          </select>
        </div>
        <div className="field">
          <label htmlFor="fb-msg">Mensaje</label>
          <textarea id="fb-msg" rows={6} maxLength={MAX_MESSAGE} value={message} onChange={(e) => setMessage(e.target.value)} />
          <span className="field-help">
            {message.length}/{MAX_MESSAGE}. Si es un error, cuenta qué hacías y qué pasó. No escribas contraseñas ni claves.
          </span>
        </div>
        <div className="field">
          <label htmlFor="fb-contact">Cómo responderte (opcional)</label>
          <input id="fb-contact" type="text" maxLength={120} placeholder="Correo, Discord…" value={contact} onChange={(e) => setContact(e.target.value)} />
        </div>
        <label className="switch-row">
          <input type="checkbox" checked={attach} onChange={(e) => setAttach(e.target.checked)} />
          <span>Incluir la versión de MercHosting{version ? ` (${version})` : ""}</span>
        </label>
        <p className="field-help">
          Se abrirá el formulario oficial de Google en tu navegador con esto ya escrito. Revísalo y pulsa «Enviar» allí: hasta entonces no
          se manda nada. MercHosting no adjunta nada más (ni registros, ni direcciones, ni nombres de servidores).
        </p>
        <button type="button" className="btn btn-go" disabled={!message.trim()} onClick={open}>
          <ExternalLink size={14} aria-hidden="true" /> Abrir el formulario para enviar
        </button>
        {opened && (
          <p className="field-help ok-text" role="status">
            Formulario abierto en tu navegador. ¡Gracias! Recuerda pulsar «Enviar» allí.
          </p>
        )}
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
      </section>
    </div>
  );
}
