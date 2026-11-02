// «Apariencia»: tema, fondo y extras visuales. El fondo sencillo o con partículas es para todos;
// los temas exclusivos, fondos extra, la insignia y las celebraciones son de Supporter.

import { Languages, Lock, Palette } from "lucide-react";
import { ICONS, iconDataUrl } from "../appIcon";
import { BACKGROUNDS, MODES, THEMES, type AppearancePrefs } from "../appearance";
import { type Lang, setLang, uiLang } from "../i18n/lang";

const LANGS: { id: Lang; name: string }[] = [
  { id: "es", name: "Español" },
  { id: "en", name: "English" },
];

/** Recarga la interfaz (el supervisor y los servidores siguen en marcha). */
function reload() {
  try {
    window.location.reload();
  } catch {
    /* entorno sin recarga (pruebas) */
  }
}

interface Props {
  /** Por defecto recarga la ventana para aplicar el idioma. */
  onLangChange?: (l: Lang) => void;
  prefs: AppearancePrefs;
  supporter: boolean;
  onChange: (p: AppearancePrefs) => void;
  onOpenSupporter: () => void;
}

export function AppearancePage({ prefs, supporter, onChange, onOpenSupporter, onLangChange = reload }: Props) {
  const set = (patch: Partial<AppearancePrefs>) => onChange({ ...prefs, ...patch });

  return (
    <div className="admin-panel companions-page appearance-page" role="region" aria-label="Apariencia">
      <header className="page-head">
        <div>
          <p className="page-eyebrow">
            <Palette size={13} aria-hidden="true" /> MercHosting
          </p>
          <h1 className="page-title">Apariencia</h1>
          <p className="page-sub">Solo cambia cómo se ve MercHosting. No afecta a tus servidores ni a su rendimiento.</p>
        </div>
      </header>

      {!supporter && (
        <p className="field-help supporter-hint">
          <Lock size={13} aria-hidden="true" /> Las opciones con candado son de Supporter Edition (US$8, pago único).{" "}
          <button type="button" className="link-btn" onClick={onOpenSupporter}>
            Ver qué incluye
          </button>
        </p>
      )}

      <section className="props-card" aria-label="Idioma">
        <h2>
          <Languages size={16} aria-hidden="true" /> Idioma · <span translate="no">Language</span>
        </h2>
        <div className="seg-choice-row" role="radiogroup" aria-label="Idioma">
          {LANGS.map((l) => (
            <button
              key={l.id}
              type="button"
              role="radio"
              aria-checked={uiLang() === l.id}
              className="choice-card choice-small"
              data-on={uiLang() === l.id}
              translate="no"
              onClick={() => {
                if (uiLang() === l.id) return;
                setLang(l.id);
                onLangChange?.(l.id);
              }}
            >
              <span className="choice-name">{l.name}</span>
            </button>
          ))}
        </div>
        <p className="field-help">Al cambiar el idioma, la ventana se vuelve a cargar. Tus servidores siguen en marcha.</p>
      </section>

      <section className="props-card" aria-label="Modo">
        <h2>Modo</h2>
        <div className="seg-choice-row" role="radiogroup" aria-label="Modo">
          {MODES.map((m) => (
            <button
              key={m.id}
              type="button"
              role="radio"
              aria-checked={prefs.mode === m.id}
              className="choice-card choice-small"
              data-on={prefs.mode === m.id}
              onClick={() => set({ mode: m.id })}
            >
              <span className="choice-name">{m.name}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="props-card" aria-label="Tema">
        <h2>Tema</h2>
        <div className="choice-grid" role="radiogroup" aria-label="Tema">
          {THEMES.map((t) => {
            const locked = t.supporter && !supporter;
            const on = prefs.theme === t.id && !locked;
            return (
              <button
                key={t.id}
                type="button"
                role="radio"
                aria-checked={on}
                aria-disabled={locked}
                className="choice-card"
                data-on={on}
                title={locked ? "De Supporter Edition" : undefined}
                onClick={() => !locked && set({ theme: t.id })}
              >
                <span className="theme-swatch" aria-hidden="true">
                  {t.swatch.map((c) => (
                    <span key={c} style={{ background: c }} />
                  ))}
                </span>
                <span className="choice-name">
                  {t.name} {locked && <Lock size={13} aria-label="Supporter" />}
                </span>
                <span className="field-help">{t.description}</span>
              </button>
            );
          })}
        </div>
      </section>

      <section className="props-card" aria-label="Fondo">
        <h2>Fondo</h2>
        <div className="choice-grid" role="radiogroup" aria-label="Fondo">
          {BACKGROUNDS.map((b) => {
            const locked = b.supporter && !supporter;
            const on = prefs.background === b.id && !locked;
            return (
              <button
                key={b.id}
                type="button"
                role="radio"
                aria-checked={on}
                aria-disabled={locked}
                className="choice-card"
                data-on={on}
                onClick={() => !locked && set({ background: b.id })}
              >
                <span className="choice-name">
                  {b.name} {locked && <Lock size={13} aria-label="Supporter" />}
                </span>
                <span className="field-help">{b.description}</span>
              </button>
            );
          })}
        </div>
        <p className="field-help">Si tu sistema tiene activado «reducir animaciones», los fondos se quedan quietos.</p>
      </section>

      <section className="props-card" aria-label="Icono de la app">
        <h2>Icono de la app</h2>
        <div className="choice-grid icon-grid" role="radiogroup" aria-label="Icono de la app">
          {ICONS.map((i) => {
            const locked = i.supporter && !supporter;
            const on = prefs.icon === i.id && !locked;
            return (
              <button
                key={i.id}
                type="button"
                role="radio"
                aria-checked={on}
                aria-disabled={locked}
                className="choice-card icon-choice"
                data-on={on}
                onClick={() => !locked && set({ icon: i.id })}
              >
                <img src={iconDataUrl(i.id)} alt="" width={44} height={44} />
                <span className="choice-name">
                  {i.name} {locked && <Lock size={13} aria-label="Supporter" />}
                </span>
              </button>
            );
          })}
        </div>
        <p className="field-help">
          Cambia el icono de la ventana y de la barra de tareas mientras MercHosting está abierto. El acceso directo del escritorio
          sigue con el icono oficial.
        </p>
      </section>

      <section className="props-card" aria-label="Extras">
        <h2>Extras {!supporter && <Lock size={14} aria-label="Supporter" />}</h2>
        <label className="check">
          <input type="checkbox" disabled={!supporter} checked={supporter && prefs.badge} onChange={(e) => set({ badge: e.target.checked })} />
          Mostrar la insignia «Colaborador» en la barra
        </label>
        <label className="check">
          <input
            type="checkbox"
            disabled={!supporter}
            checked={supporter && prefs.celebrations}
            onChange={(e) => set({ celebrations: e.target.checked })}
          />
          Animación de celebración cuando un servidor termina de encender
        </label>
      </section>
    </div>
  );
}
