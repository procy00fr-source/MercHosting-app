// Pantalla «Servidores»: todos los servidores en tarjetas para elegir cuál administrar.

import { ChevronRight, FolderInput, Plus } from "lucide-react";
import type { InstanceSummary } from "../api/types";
import { KIND_LABEL, STATE_LABEL, STATE_TONE, hasProcess } from "../format";
import { Logo } from "./Logo";

interface Props {
  servers: InstanceSummary[] | null;
  onOpen: (id: string) => void;
  onCreate: () => void;
  onImport: () => void;
}

export function ServersHome({ servers, onOpen, onCreate, onImport }: Props) {
  const online = servers?.filter((s) => s.status.state === "ONLINE").length ?? 0;
  const active = (servers ?? []).filter((s) => !s.archived);
  const archived = (servers ?? []).filter((s) => s.archived);
  const tile = (s: InstanceSummary) => {
    const tone = STATE_TONE[s.status.state];
    const port = s.status.port ?? s.configured_port ?? 25565;
    return (
      <li key={s.instance.id}>
        <button
          type="button"
          className="card server-tile"
          data-tone={tone}
          onClick={() => onOpen(s.instance.id)}
          aria-label={`Abrir ${s.instance.name}`}
        >
          <span className="server-tile-top">
            <Logo size={46} tone={s.archived ? "idle" : tone} />
            <span className="status-pill" data-tone={s.archived ? "idle" : tone}>
              {s.archived ? "Archivado" : STATE_LABEL[s.status.state]}
            </span>
          </span>
          <span className="server-tile-name" translate="no">{s.instance.name}</span>
          <span className="server-tile-sub">
            {KIND_LABEL[s.instance.kind]} {s.instance.mc_version ?? ""} · puerto {port}
            {hasProcess(s.status.state) && !s.status.has_console ? " · sin consola" : ""}
          </span>
          <span className="server-tile-open">
            Administrar <ChevronRight size={15} aria-hidden="true" />
          </span>
        </button>
      </li>
    );
  };
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <p className="page-eyebrow">Inicio</p>
          <h1 className="page-title">Servidores</h1>
          <p className="page-sub">
            {servers === null
              ? "Cargando…"
              : servers.length === 0
                ? "Aquí aparecerán los servidores que crees o importes."
                : `${servers.length} servidor${servers.length === 1 ? "" : "es"} · ${online} en línea. Elige uno para administrarlo.`}
          </p>
        </div>
        <div className="page-actions">
          <button type="button" className="btn btn-ghost" onClick={onImport}>
            <FolderInput size={16} aria-hidden="true" /> Importar servidor
          </button>
          <button type="button" className="btn btn-go" onClick={onCreate}>
            <Plus size={17} aria-hidden="true" /> Crear servidor
          </button>
        </div>
      </header>

      {servers !== null && servers.length === 0 ? (
        <div className="empty">
          <Logo size={96} tone="idle" />
          <h2>Todavía no hay servidores</h2>
          <p>
            Crea uno nuevo (Vanilla, Paper, Fabric, NeoForge o un modpack) o importa la carpeta de uno que ya tengas. Se ejecuta en tu
            PC y MercHosting lo mantiene encendido aunque cierres esta ventana.
          </p>
          <div className="empty-actions">
            <button type="button" className="btn btn-lg btn-go" onClick={onCreate}>
              <Plus size={18} aria-hidden="true" /> Crear servidor
            </button>
            <button type="button" className="btn btn-lg" onClick={onImport}>
              <FolderInput size={18} aria-hidden="true" /> Importar servidor
            </button>
          </div>
        </div>
      ) : (
        <>
          <ul className="servers-grid">{active.map(tile)}</ul>
          {archived.length > 0 && (
            <details className="archived-group">
              <summary>Archivados ({archived.length})</summary>
              <p className="field-help">No se pueden encender hasta desarchivarlos. Sus carpetas siguen intactas.</p>
              <ul className="servers-grid">{archived.map(tile)}</ul>
            </details>
          )}
        </>
      )}
    </div>
  );
}
