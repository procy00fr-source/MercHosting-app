// Bienvenida del primer inicio: qué es el hosting local, qué tiene tu PC (datos reales), seguridad
// y cómo prefieres ver la app. Se puede saltar en cualquier momento y volver a ver desde Ayuda.

import { Cpu, HardDrive, House, MemoryStick, ShieldCheck, Sparkles } from "lucide-react";
import { useState } from "react";
import type { SystemInfo } from "../api/types";
import type { UiLevel } from "../uiLevel";
import { formatMemory } from "../format";
import { Logo } from "./Logo";

interface Props {
  system: SystemInfo | null;
  level: UiLevel;
  onLevel: (l: UiLevel) => void;
  /** tour: empezar el tutorial de primeros pasos al cerrar. */
  onClose: (startTour: boolean) => void;
}

/** Memoria orientativa para un servidor según la RAM total de la PC (en MB). */
export function suggestedServerMemory(totalBytes: number | null): { min: number; max: number } | null {
  if (!totalBytes) return null;
  const gb = totalBytes / 1024 ** 3;
  if (gb < 6) return { min: 1024, max: 2048 };
  if (gb < 12) return { min: 2048, max: 4096 };
  if (gb < 24) return { min: 4096, max: 6144 };
  return { min: 6144, max: 10240 };
}

const STEPS = ["Bienvenida", "Tu PC", "Seguridad", "Empezar"];

export function WelcomeDialog({ system, level, onLevel, onClose }: Props) {
  const [step, setStep] = useState(0);
  const mem = suggestedServerMemory(system?.total_memory_bytes ?? null);
  const totalGb = system?.total_memory_bytes ? system.total_memory_bytes / 1024 ** 3 : null;

  return (
    <div className="dialog-backdrop welcome-backdrop">
      <div className="dialog welcome" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
        <ol className="wiz-steps" aria-label="Pasos de la bienvenida">
          {STEPS.map((s, i) => (
            <li key={s} data-state={i < step ? "done" : i === step ? "current" : "todo"} aria-current={i === step ? "step" : undefined}>
              <span className="wiz-dot" aria-hidden="true">
                {i + 1}
              </span>
              {s}
            </li>
          ))}
        </ol>

        {step === 0 && (
          <section className="welcome-body">
            <Logo size={64} />
            <h1 id="welcome-title">Bienvenido a MercHosting</h1>
            <p className="welcome-slogan">«Tu primer servidor no debería necesitar una mensualidad».</p>
            <ul className="welcome-points">
              <li>
                <House size={18} aria-hidden="true" /> <span><strong>Tu PC es el servidor.</strong> Minecraft corre aquí, con tu memoria, tu procesador y tu internet. No es un hosting en la nube.</span>
              </li>
              <li>
                <HardDrive size={18} aria-hidden="true" /> <span><strong>Mientras tu PC esté encendida</strong> y conectada, tus amigos pueden entrar. Si la apagas, el servidor se apaga.</span>
              </li>
              <li>
                <Sparkles size={18} aria-hidden="true" /> <span><strong>Todo desde aquí:</strong> crear, encender, mods, copias y amigos, sin usar CMD.</span>
              </li>
            </ul>
          </section>
        )}

        {step === 1 && (
          <section className="welcome-body">
            <h1 id="welcome-title">Tu PC</h1>
            <div className="welcome-facts">
              <div>
                <MemoryStick size={20} aria-hidden="true" />
                <strong>{totalGb ? `${totalGb.toFixed(totalGb < 10 ? 1 : 0)} GB` : "No disponible"}</strong>
                <span>de memoria (RAM)</span>
              </div>
              <div>
                <Cpu size={20} aria-hidden="true" />
                <strong>{system ? system.logical_cpus : "No disponible"}</strong>
                <span>núcleos lógicos</span>
              </div>
            </div>
            {mem ? (
              <p>
                Para un servidor con pocos amigos, una cifra orientativa es darle <strong>{formatMemory(mem.min)}</strong> a{" "}
                <strong>{formatMemory(mem.max)}</strong>. Los modpacks grandes piden más.
              </p>
            ) : (
              <p>No se pudo leer la memoria de tu PC. El asistente te ayudará a elegirla al crear el servidor.</p>
            )}
            {totalGb !== null && totalGb < 8 && (
              <p className="warn-text">Tu PC tiene poca memoria: un servidor pequeño funcionará, pero jugar en la misma PC a la vez puede ir lento.</p>
            )}
            <p className="field-help">Si juegas en esta misma PC, el juego y el servidor se reparten la memoria y el procesador.</p>
          </section>
        )}

        {step === 2 && (
          <section className="welcome-body">
            <h1 id="welcome-title">
              <ShieldCheck size={22} aria-hidden="true" /> Seguridad
            </h1>
            <ul className="plain-list">
              <li>La EULA de Minecraft la aceptas tú. MercHosting nunca la acepta por ti.</li>
              <li>Los mods y plugins son programas que se ejecutan en tu PC. Instálalos solo de fuentes de confianza.</li>
              <li>Deja activado «Comprobar cuentas oficiales» y usa la lista blanca si solo juegas con amigos.</li>
              <li>Da acceso de compañero solo a personas de confianza: los archivos de tus servidores son archivos reales de tu PC.</li>
              <li>MercHosting nunca te pedirá tu contraseña de Minecraft ni la de tu PC.</li>
            </ul>
          </section>
        )}

        {step === 3 && (
          <section className="welcome-body">
            <h1 id="welcome-title">¿Cómo prefieres empezar?</h1>
            <div className="choice-grid" role="radiogroup" aria-label="Modo de la app">
              {(
                [
                  ["principiante", "Principiante", "Lo esencial a la vista. Las secciones avanzadas quedan ocultas hasta que las actives."],
                  ["avanzado", "Avanzado", "Todo: archivos, tareas programadas, salud, mundos, actividad y seguridad."],
                ] as const
              ).map(([id, name, desc]) => (
                <button key={id} type="button" role="radio" aria-checked={level === id} className="choice-card" data-on={level === id} onClick={() => onLevel(id)}>
                  <span className="choice-name">{name}</span>
                  <span className="field-help">{desc}</span>
                </button>
              ))}
            </div>
            <p className="field-help">Puedes cambiarlo cuando quieras en Ayuda y tutoriales.</p>
          </section>
        )}

        <div className="dialog-foot">
          <button type="button" className="btn btn-quiet" onClick={() => onClose(false)}>
            Saltar
          </button>
          <span className="dialog-spacer" />
          {step > 0 && (
            <button type="button" className="btn btn-quiet" onClick={() => setStep(step - 1)}>
              Atrás
            </button>
          )}
          {step < STEPS.length - 1 ? (
            <button type="button" className="btn btn-go" onClick={() => setStep(step + 1)}>
              Siguiente
            </button>
          ) : (
            <>
              <button type="button" className="btn" onClick={() => onClose(false)}>
                Empezar sin tutorial
              </button>
              <button type="button" className="btn btn-go" onClick={() => onClose(true)}>
                Ver el tutorial
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
