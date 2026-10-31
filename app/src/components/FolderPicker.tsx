// Elegir una carpeta del servidor como destino de «Mover» o «Copiar».

import { ChevronRight, Folder, House } from "lucide-react";
import { useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId } from "../api/types";
import { Dialog } from "./Dialog";

interface Props {
  sup: Supervisor;
  id: InstanceId;
  /** Lo que se mueve o copia (ruta relativa). */
  item: string;
  isDir: boolean;
  mode: "move" | "copy";
  start: string;
  onPick: (dest: string) => void;
  onClose: () => void;
}

export function FolderPicker({ sup, id, item, isDir, mode, start, onPick, onClose }: Props) {
  const [dir, setDir] = useState(start);
  const [dirs, setDirs] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const name = item.split("/").pop()!;
  const parent = item.includes("/") ? item.slice(0, item.lastIndexOf("/")) : "";

  useEffect(() => {
    let alive = true;
    setDirs(null);
    setError(null);
    sup.listFiles(id, dir).then(
      (r) => alive && setDirs(r.filter((e) => e.is_dir && !e.link).map((e) => e.name)),
      (e) => alive && setError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [sup, id, dir]);

  const crumbs = dir ? dir.split("/") : [];
  const inside = isDir && (dir === item || dir.startsWith(`${item}/`));
  const same = dir === parent;
  const verb = mode === "move" ? "Mover" : "Copiar";

  return (
    <Dialog
      title={`${verb} «${name}»`}
      lead="Elige la carpeta de destino dentro del servidor. Si ya hay algo con el mismo nombre, no se sobrescribe."
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            Cancelar
          </button>
          <button type="button" className="btn btn-go" disabled={inside || same} onClick={() => onPick(dir)}>
            {verb} aquí
          </button>
        </>
      }
    >
      <nav className="crumbs" aria-label="Carpeta de destino">
        <button type="button" className="crumb" onClick={() => setDir("")} aria-label="Carpeta del servidor">
          <House size={14} aria-hidden="true" />
        </button>
        {crumbs.map((c, i) => (
          <span key={i} className="crumb-wrap">
            <ChevronRight size={13} aria-hidden="true" />
            <button type="button" className="crumb" onClick={() => setDir(crumbs.slice(0, i + 1).join("/"))}>
              {c}
            </button>
          </span>
        ))}
      </nav>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {dirs === null && !error ? (
        <p className="wiz-loading" role="status">
          Leyendo…
        </p>
      ) : dirs && dirs.length === 0 ? (
        <p className="field-help">No hay subcarpetas aquí.</p>
      ) : (
        <ul className="picker-list" aria-label="Subcarpetas">
          {dirs?.map((d) => {
            const full = dir ? `${dir}/${d}` : d;
            const self = isDir && full === item;
            return (
              <li key={d}>
                <button type="button" className="file-name" disabled={self} onClick={() => setDir(full)} title={self ? "Es la carpeta que estás moviendo" : undefined}>
                  <Folder size={15} aria-hidden="true" className="ic-folder" /> {d}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {inside && <p className="field-help warn-text">No se puede meter una carpeta dentro de sí misma.</p>}
      {same && !inside && <p className="field-help">Ya está en esta carpeta: elige otra.</p>}
    </Dialog>
  );
}
