// «Ayuda y tutoriales»: repetir cualquier tutorial o la bienvenida, cambiar de modo y preguntas
// frecuentes con respuestas honestas.

import { BookOpen, CirclePlay, LifeBuoy } from "lucide-react";
import type { MercApi, Supervisor } from "../api/client";
import type { AppUpdateCheck } from "../api/types";
import { TOURS } from "../tours";
import { AppUpdateCard } from "./AppUpdateCard";
import type { UiLevel } from "../uiLevel";

const FAQ: { q: string; a: string }[] = [
  {
    q: "Mis amigos no pueden entrar",
    a: "Si están en tu misma red, usa la dirección de «En tu misma red» (Conexión). Si están lejos, hace falta un túnel: activa Playit en Conexión. Usa «Probar» para ver si la dirección responde desde Internet. Si el servidor tiene lista blanca, añádelos en Jugadores.",
  },
  {
    q: "¿Por qué tiene que estar encendida mi PC?",
    a: "Porque el servidor corre en tu PC. MercHosting no es un hosting en la nube: si apagas o suspendes la PC, el servidor se apaga para todos.",
  },
  {
    q: "¿Cuánta memoria le doy al servidor?",
    a: "Para unos pocos amigos en Vanilla o Paper, 2–4 GB suelen bastar. Los modpacks grandes piden 6 GB o más. No le des toda la memoria de tu PC: el sistema (y el juego, si juegas aquí) también la necesitan.",
  },
  {
    q: "¿Qué es la EULA?",
    a: "El acuerdo de Mojang para usar el servidor de Minecraft. Tienes que leerlo y aceptarlo tú: MercHosting nunca lo acepta por ti.",
  },
  {
    q: "¿Los mods son seguros?",
    a: "Los mods y plugins son programas que se ejecutan en tu PC. MercHosting comprueba que el archivo descargado sea el publicado, pero no puede garantizar qué hace. Instala solo de fuentes de confianza.",
  },
  {
    q: "El servidor se cierra solo al arrancar",
    a: "Abre la pestaña Diagnóstico del servidor: revisa los registros y los mods y te dice qué encontró, con la prueba y un arreglo reversible cuando lo hay.",
  },
  {
    q: "¿Pierdo algo si cierro MercHosting?",
    a: "No. Tus servidores siguen encendidos aunque cierres la ventana. Al volver a abrir MercHosting recupera el control.",
  },
];

interface Props {
  level: UiLevel;
  hasServer: boolean;
  onLevel: (l: UiLevel) => void;
  onStartTour: (id: string) => void;
  onWelcome: () => void;
  api?: MercApi;
  sup?: Supervisor;
  appUpdate?: AppUpdateCheck | null;
}

export function HelpPage({ level, hasServer, onLevel, onStartTour, onWelcome, api, sup, appUpdate }: Props) {
  return (
    <div className="admin-panel companions-page help-page" role="region" aria-label="Ayuda y tutoriales">
      <header className="page-head">
        <div>
          <p className="page-eyebrow">MercHosting</p>
          <h1 className="page-title">Ayuda y tutoriales</h1>
          <p className="page-sub">Repite cualquier tutorial: señalan los botones de verdad de la app.</p>
        </div>
      </header>

      <section className="props-card" aria-label="Tutoriales">
        <h2>
          <CirclePlay size={18} aria-hidden="true" /> Tutoriales
        </h2>
        <ul className="tour-list">
          {TOURS.map((t) => {
            const blocked = t.needsServer && !hasServer;
            return (
              <li key={t.id}>
                <div>
                  <strong>{t.name}</strong>
                  <span className="field-help">{blocked ? "Necesitas un servidor: crea o importa uno primero." : t.description}</span>
                </div>
                <button type="button" className="btn btn-sm" disabled={blocked} onClick={() => onStartTour(t.id)} aria-label={`Empezar: ${t.name}`}>
                  Empezar
                </button>
              </li>
            );
          })}
        </ul>
        <button type="button" className="btn btn-sm btn-quiet" onClick={onWelcome}>
          <BookOpen size={14} aria-hidden="true" /> Ver la bienvenida otra vez
        </button>
      </section>

      <section className="props-card" aria-label="Modo de la app">
        <h2>Modo</h2>
        <div className="seg-choice-row" role="radiogroup" aria-label="Modo de la app">
          {(
            [
              ["principiante", "Principiante"],
              ["avanzado", "Avanzado"],
            ] as const
          ).map(([id, name]) => (
            <button key={id} type="button" role="radio" aria-checked={level === id} className="choice-card choice-small" data-on={level === id} onClick={() => onLevel(id)}>
              <span className="choice-name">{name}</span>
            </button>
          ))}
        </div>
        <p className="field-help">
          {level === "principiante"
            ? "Ocultas ahora: Archivos, Tareas programadas, Salud y rendimiento, Actividad, Seguridad, Mundos, Datapacks y LuckPerms. No se borra ni se desactiva nada."
            : "Ves todas las secciones de cada servidor."}
        </p>
      </section>

      {api && sup && <AppUpdateCard key={appUpdate?.version ?? "x"} api={api} sup={sup} initial={appUpdate} />}

      <section className="props-card" aria-label="Preguntas frecuentes">
        <h2>
          <LifeBuoy size={18} aria-hidden="true" /> Preguntas frecuentes
        </h2>
        <div className="faq">
          {FAQ.map((f) => (
            <details key={f.q}>
              <summary>{f.q}</summary>
              <p>{f.a}</p>
            </details>
          ))}
        </div>
      </section>
    </div>
  );
}
