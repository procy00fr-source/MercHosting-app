// Barra lateral: marca, botón «Servidores», secciones agrupadas y pie (sonido, lado de la barra,
// estado del supervisor). La usan la lista de servidores y la vista de cada servidor.

import { Bell, ChevronsUpDown, CircleHelp, PanelLeft, PanelRight, Server, Volume2, VolumeX } from "lucide-react";
import type { ReactNode } from "react";
import type { StateTone } from "../format";
import { Logo } from "./Logo";

type LogoTone = StateTone | "brand";

export interface NavItem {
  id: string;
  label: string;
  icon: ReactNode;
  /** Punto de aviso (p. ej. actualizaciones o mantenimiento). */
  dot?: boolean;
}

export interface NavGroup {
  title: string;
  items: NavItem[];
}

export interface SidebarCommon {
  brandTone: LogoTone;
  brandSub: string;
  introSound: boolean;
  onToggleSound: () => void;
  side: "left" | "right";
  onToggleSide: () => void;
  supervisorOk: boolean;
  supervisorText: string;
  alertsUnread: number;
  onOpenAlerts: () => void;
  /** Abre «Ayuda y tutoriales». */
  onHelp?: () => void;
  /** Hay una versión nueva de MercHosting (se ve en Ayuda). */
  appUpdate?: string | null;
  /** Insignia «Colaborador» (Supporter, opcional). */
  badge?: boolean;
}

interface Props extends SidebarCommon {
  /** Servidor abierto (si lo hay): el botón «Servidores» lo muestra y vuelve a la lista. */
  current?: { name: string; sub: string; tone: LogoTone } | null;
  onServers: () => void;
  serversActive: boolean;
  groups: NavGroup[];
  active: string | null;
  onSelect: (id: string) => void;
  /** Contenido extra bajo las secciones (por ejemplo, crear o importar). */
  extra?: ReactNode;
}

export function Sidebar(p: Props) {
  return (
    <nav className="sidebar" aria-label="Navegación principal">
      <div className="brand">
        <Logo size={40} tone={p.brandTone} />
        <div>
          <p className="brand-name">MercHosting</p>
          <p className="brand-sub">{p.brandSub}</p>
          {p.badge && (
            <span className="supporter-badge" title="Gracias por apoyar MercHosting">
              Colaborador
            </span>
          )}
        </div>
      </div>

      {p.current ? (
        <button type="button" className="server-switch" onClick={p.onServers} title="Elegir otro servidor">
          <Logo size={30} tone={p.current.tone} />
          <span className="server-switch-text">
            <span className="server-switch-label">Servidores</span>
            <span className="server-switch-name" translate="no">{p.current.name}</span>
            <span className="server-switch-sub">{p.current.sub}</span>
          </span>
          <ChevronsUpDown size={16} aria-hidden="true" />
        </button>
      ) : (
        <button type="button" className="nav-item nav-servers" data-tour="nav-servers" aria-current={p.serversActive ? "page" : undefined} onClick={p.onServers}>
          <Server size={18} aria-hidden="true" /> Servidores
        </button>
      )}

      <div className="nav-groups">
        {p.groups.map((g) => (
          <div key={g.title} className="nav-group">
            <p className="nav-title">{g.title}</p>
            <div role="tablist" aria-orientation="vertical" aria-label={g.title} className="nav-list">
              {g.items.map((it) => (
                <button
                  key={it.id}
                  type="button"
                  role="tab"
                  aria-selected={p.active === it.id}
                  className="nav-item"
                  data-tour={`nav-${it.id}`}
                  onClick={() => p.onSelect(it.id)}
                >
                  {it.icon}
                  <span>{it.label}</span>
                  {it.dot && <span className="nav-dot" aria-hidden="true" />}
                </button>
              ))}
            </div>
          </div>
        ))}
        {p.extra}
      </div>

      <div className="sidebar-foot">
        <p className="supervisor-note" data-ok={p.supervisorOk}>
          <span className="dot" aria-hidden="true" />
          {p.supervisorText}
        </p>
        <div className="foot-tools">
          {p.onHelp && (
            <button
              type="button"
              className="icon-btn bell-btn"
              data-tour="ayuda"
              aria-label={p.appUpdate ? `Ayuda y tutoriales (MercHosting ${p.appUpdate} disponible)` : "Ayuda y tutoriales"}
              title={p.appUpdate ? `MercHosting ${p.appUpdate} disponible` : "Ayuda y tutoriales"}
              onClick={p.onHelp}
            >
              <CircleHelp size={16} />
              {p.appUpdate && (
                <span className="bell-count" aria-hidden="true">
                  1
                </span>
              )}
            </button>
          )}
          <button
            type="button"
            className="icon-btn bell-btn"
            aria-label={p.alertsUnread > 0 ? `Alertas: ${p.alertsUnread} sin leer` : "Alertas"}
            title="Alertas"
            onClick={p.onOpenAlerts}
          >
            <Bell size={16} />
            {p.alertsUnread > 0 && (
              <span className="bell-count" aria-hidden="true">
                {p.alertsUnread > 99 ? "99+" : p.alertsUnread}
              </span>
            )}
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-pressed={p.introSound}
            aria-label={`Sonido de entrada: ${p.introSound ? "activado" : "silenciado"}`}
            title={`Sonido de entrada: ${p.introSound ? "activado" : "silenciado"}`}
            onClick={p.onToggleSound}
          >
            {p.introSound ? <Volume2 size={16} /> : <VolumeX size={16} />}
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label={p.side === "left" ? "Mover la barra a la derecha" : "Mover la barra a la izquierda"}
            title={p.side === "left" ? "Mover la barra a la derecha" : "Mover la barra a la izquierda"}
            onClick={p.onToggleSide}
          >
            {p.side === "left" ? <PanelRight size={16} /> : <PanelLeft size={16} />}
          </button>
        </div>
      </div>
    </nav>
  );
}
