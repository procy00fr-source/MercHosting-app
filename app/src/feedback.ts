// Comentarios: se envían con un formulario de Google del desarrollador. La app solo abre el
// formulario ya rellenado en el navegador; la persona lo revisa y pulsa «Enviar» allí. No hay
// claves ni servidores propios, y no se manda nada sin que la persona lo confirme.

export interface FeedbackForm {
  /** `https://docs.google.com/forms/d/e/<id>/viewform` */
  viewUrl: string;
  /** Identificadores `entry.<número>` de cada pregunta del formulario. */
  fields: { kind: string; message: string; contact: string; version: string };
  /** Opciones de la pregunta de tipo, escritas EXACTAMENTE como en el formulario. */
  kinds: string[];
}

/** El formulario oficial. `null` mientras no esté creado: la sección no se muestra. */
export const FEEDBACK_FORM: FeedbackForm | null = {
  viewUrl: "https://docs.google.com/forms/d/e/1FAIpQLSddxR6CPmFU8Qz1RelNxHqY8oqpQ2hIugNgPwygz4JbFB__JQ/viewform",
  fields: { kind: "entry.1692104274", message: "entry.75650578", contact: "entry.1707701594", version: "entry.1351136142" },
  // Tal cual están escritas en el formulario (en minúsculas); en la app se muestran con mayúscula.
  kinds: ["idea", "error", "otro"],
};

export const MAX_MESSAGE = 1500;

const VIEW = /^https:\/\/docs\.google\.com\/forms\/d\/e\/[A-Za-z0-9_-]+\/viewform$/;
const ENTRY = /^entry\.\d+$/;

export function validForm(f: FeedbackForm | null): f is FeedbackForm {
  return (
    !!f &&
    VIEW.test(f.viewUrl) &&
    Object.values(f.fields).every((e) => ENTRY.test(e)) &&
    f.kinds.length > 0
  );
}

/** Enlace del formulario con las respuestas ya escritas (la persona pulsa «Enviar» en Google). */
export function prefilledUrl(
  f: FeedbackForm,
  answers: { kind: string; message: string; contact: string; version: string | null },
): string {
  const q = new URLSearchParams({ usp: "pp_url" });
  q.set(f.fields.kind, answers.kind);
  q.set(f.fields.message, answers.message.slice(0, MAX_MESSAGE));
  if (answers.contact.trim()) q.set(f.fields.contact, answers.contact.trim());
  if (answers.version) q.set(f.fields.version, answers.version);
  return `${f.viewUrl}?${q.toString()}`;
}

/** Cómo se muestra una opción en la app («idea» → «Idea»). */
export function kindLabel(k: string): string {
  return k.charAt(0).toUpperCase() + k.slice(1);
}
