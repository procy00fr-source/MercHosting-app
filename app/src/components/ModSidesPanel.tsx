// Separador de mods cliente/servidor: clasifica cada mod con su evidencia y mueve los que el
// servidor no necesita a una cuarentena reversible (mods-cuarentena). Nunca borra nada.

import { ArchiveRestore, ChevronDown, Power, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId, ModSide, ModSidesInfo } from "../api/types";
import { modSidesEnabled, setModSidesEnabled } from "../modSidesPref";
import { Dialog } from "./Dialog";

const SIDE_LABEL: Record<ModSide, string> = {
  client_only: "Solo cliente",
  server_only: "Solo servidor",
  both: "Cliente y servidor",
  unknown: "Sin datos",
};
const SIDE_HELP: Record<ModSide, string> = {
  client_only: "Solo funciona en el juego de cada jugador. En el servidor sobra (y en Forge o NeoForge puede impedir que arranque).",
  server_only: "Solo hace falta en el servidor: tus amigos no lo necesitan.",
  both: "Va en el servidor y en el juego de los jugadores.",
  unknown: "No hay datos suficientes o se contradicen: revísalo tú.",
};
const ORDER: ModSide[] = ["client_only", "unknown", "server_only", "both"];

type Props = { sup: Supervisor; id: InstanceId; running: boolean; onChanged: () => void; onToggle?: (on: boolean) => void };

export function ModSidesPanel(props: Props) {
  const [on, setOn] = useState(() => modSidesEnabled(props.id));
  const toggle = (v: boolean) => {
    setModSidesEnabled(props.id, v);
    setOn(v);
    props.onToggle?.(v);
  };
  if (!on) {
    return (
      <section className="sides-panel" aria-label="Separador de mods">
        <div className="card sides-off">
          <h3>Separador de mods cliente/servidor: apagado</h3>
          <p className="field-help">
            Si lo activas, MercHosting revisa los mods de este servidor y te dice cuáles solo sirven en el juego de los jugadores
            (minimapas, shaders, menús). No se mueve nada por su cuenta: tú eliges cuáles apartar y confirmas cada vez, y se pueden
            devolver.
          </p>
          <p className="field-help">
            Mientras esté apagado no se revisa ningún mod ni aparecen avisos. Si ya apartaste alguno, sigue en mods-cuarentena:
            activa el separador para devolverlo.
          </p>
          <button type="button" className="btn btn-go" onClick={() => toggle(true)}>
            <Power size={15} aria-hidden="true" /> Activar el separador
          </button>
        </div>
      </section>
    );
  }
  return <SidesOn {...props} onOff={() => toggle(false)} />;
}

function SidesOn({ sup, id, running, onChanged, onOff }: Props & { onOff: () => void }) {
  const [info, setInfo] = useState<ModSidesInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [open, setOpen] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);

  const load = useCallback(() => {
    setError(null);
    sup.modSides(id).then(
      (i) => {
        setInfo(i);
        setSel(new Set());
      },
      (e) => setError(errorMessage(e)),
    );
  }, [sup, id]);
  useEffect(load, [load]);

  const counts = useMemo(() => {
    const c: Record<ModSide, number> = { client_only: 0, server_only: 0, both: 0, unknown: 0 };
    info?.items.forEach((i) => c[i.side]++);
    return c;
  }, [info]);

  async function run(fn: () => Promise<ModSidesInfo>, msg: string) {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const i = await fn();
      setInfo(i);
      setSel(new Set());
      setNote(msg);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
    setBusy(false);
  }

  const pick = (f: string) =>
    setSel((s) => {
      const n = new Set(s);
      if (n.has(f)) n.delete(f);
      else n.add(f);
      return n;
    });

  if (!info) {
    return error ? (
      <p className="form-error" role="alert">
        {error}
      </p>
    ) : (
      <p className="wiz-loading" role="status">
        Revisando cada mod (su ficha y el catálogo)…
      </p>
    );
  }

  const chosen = info.items.filter((i) => sel.has(i.file));
  const suggested = info.items.filter((i) => i.suggested);
  const risky = chosen.filter((i) => i.side !== "client_only" || i.needed_by.length > 0);

  return (
    <section className="sides-panel" aria-label="Separador de mods">
      <p className="field-help">
        Los modpacks de cliente y los servidores copiados desde un juego suelen traer mods que solo sirven en el juego (minimapas, shaders,
        menús). Aquí ves de qué lado es cada uno y por qué. Los que muevas van a la carpeta <span className="mono">{info.folder}</span> del
        servidor: no se borra nada y puedes devolverlos cuando quieras.
      </p>
      {!info.catalog_ok && (
        <p className="field-help warn-text">Sin conexión con algún catálogo: la clasificación se basa sobre todo en la ficha de cada mod.</p>
      )}
      <div className="sides-summary" role="list" aria-label="Resumen">
        {ORDER.map((s) => (
          <span key={s} role="listitem" className={`side-chip side-${s}`}>
            <b>{counts[s]}</b> {SIDE_LABEL[s].toLowerCase()}
          </span>
        ))}
      </div>
      <div className="files-bar">
        <span className="field-help">
          {suggested.length
            ? `Sugerencia: ${suggested.length} de solo cliente sin dudas ni dependencias. No se marcan solos.`
            : "No hay sugerencias seguras: revisa uno por uno si quieres apartar alguno."}
        </span>
        <span className="sides-actions">
          {suggested.length > 0 && (
            <button type="button" className="btn btn-sm" disabled={busy} onClick={() => setSel(new Set(suggested.map((x) => x.file)))}>
              Marcar los sugeridos ({suggested.length})
            </button>
          )}
          <button type="button" className="btn btn-sm btn-quiet" onClick={onOff}>
            <Power size={14} aria-hidden="true" /> Desactivar el separador
          </button>
        </span>
      </div>
      {running && <p className="field-help warn-text">Detén el servidor para mover o devolver mods.</p>}
      {note && (
        <p className="field-help ok-text" role="status">
          {note}
        </p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      {info.items.length === 0 ? (
        <p className="field-help">No hay mods en la carpeta mods.</p>
      ) : (
        <ul className="sides-list" aria-label="Mods">
          {info.items.map((i) => (
            <li key={i.file} className={`sides-row side-${i.side}`}>
              <label className="sides-check">
                <input type="checkbox" checked={sel.has(i.file)} disabled={busy} onChange={() => pick(i.file)} aria-label={`Mover ${i.name} a la cuarentena`} />
              </label>
              <div className="sides-main">
                <div className="sides-title">
                  <b>{i.name}</b>
                  <span className={`badge side-badge side-${i.side}`}>{SIDE_LABEL[i.side]}</span>
                  {i.confidence !== "alta" && <span className="badge badge-muted">confianza {i.confidence}</span>}
                  {!i.enabled && <span className="badge badge-muted">Desactivado</span>}
                </div>
                <span className="field-help mono">{i.file}</span>
                {i.needed_by.length > 0 && (
                  <span className="field-help warn-text">Lo necesita{i.needed_by.length > 1 ? "n" : ""}: {i.needed_by.join(", ")}. Si lo mueves, esos podrían fallar.</span>
                )}
                <button type="button" className="link-btn sides-why" aria-expanded={open === i.file} onClick={() => setOpen(open === i.file ? null : i.file)}>
                  Por qué <ChevronDown size={13} aria-hidden="true" />
                </button>
                {open === i.file && (
                  <ul className="sides-evidence">
                    <li>{SIDE_HELP[i.side]}</li>
                    {i.evidence.map((e) => (
                      <li key={e}>{e}</li>
                    ))}
                  </ul>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}

      <div className="files-bar">
        <span className="field-help">{chosen.length ? `${chosen.length} elegido${chosen.length > 1 ? "s" : ""}` : "Marca los que quieras apartar."}</span>
        <button type="button" className="btn btn-go" disabled={!chosen.length || running || busy} onClick={() => setConfirm(true)}>
          <ShieldCheck size={15} aria-hidden="true" /> Mover a la cuarentena
        </button>
      </div>

      <section className="props-card" aria-label="En cuarentena">
        <h3>En cuarentena ({info.quarantined.length})</h3>
        {info.quarantined.length === 0 ? (
          <p className="field-help">Ninguno. Lo que muevas aparecerá aquí para devolverlo con un clic.</p>
        ) : (
          <>
            <ul className="sides-q">
              {info.quarantined.map((q) => (
                <li key={q.file}>
                  <span>
                    <span className="mono">{q.file}</span>
                    <span className="field-help">
                      {q.reason ? ` · ${q.reason}` : ""}
                      {q.moved_at_unix ? ` · ${new Date(q.moved_at_unix * 1000).toLocaleDateString()}` : ""}
                    </span>
                  </span>
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={running || busy}
                    onClick={() => run(() => sup.restoreMods(id, [q.file]), `${q.file} volvió a la carpeta mods.`)}
                    aria-label={`Devolver ${q.file}`}
                  >
                    <ArchiveRestore size={14} aria-hidden="true" /> Devolver
                  </button>
                </li>
              ))}
            </ul>
            <button
              type="button"
              className="btn btn-sm btn-quiet"
              disabled={running || busy}
              onClick={() => run(() => sup.restoreMods(id, info.quarantined.map((q) => q.file)), "Todos volvieron a la carpeta mods.")}
            >
              Devolver todos
            </button>
          </>
        )}
      </section>

      {confirm && (
        <Dialog
          title={`Mover ${chosen.length} mod${chosen.length > 1 ? "s" : ""} a la cuarentena`}
          lead={`Se mueven a ${info.folder}: el servidor ya no los carga, pero siguen en tu PC y puedes devolverlos cuando quieras.`}
          onClose={() => setConfirm(false)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirm(false)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                onClick={() => {
                  setConfirm(false);
                  void run(
                    () => sup.quarantineMods(id, chosen.map((c) => c.file)),
                    `${chosen.length} mod${chosen.length > 1 ? "s" : ""} en cuarentena. Enciende el servidor para comprobar que todo va bien.`,
                  );
                }}
              >
                Mover
              </button>
            </>
          }
        >
          <ul className="sides-confirm">
            {chosen.map((c) => (
              <li key={c.file}>
                {c.name} <span className="field-help">· {SIDE_LABEL[c.side]}</span>
              </li>
            ))}
          </ul>
          {risky.length > 0 && (
            <p className="warn-text">
              Ojo: {risky.map((r) => r.name).join(", ")} no {risky.length > 1 ? "son" : "es"} claramente de solo cliente o algún mod lo necesita.
              Si el servidor falla al encender, devuélvelo{risky.length > 1 ? "s" : ""}.
            </p>
          )}
        </Dialog>
      )}
    </section>
  );
}
