// Permisos con LuckPerms: grupos, usuarios, nodos, herencia, contextos y caducidades.
// Lo que se ve sale de `luckperms export` (lo hace el supervisor con el servidor encendido).
// Cada cambio se muestra antes de aplicarse (comando exacto), se guarda una copia y después se
// comprueba con otro export.

import { CheckCircle2, KeyRound, Plus, RefreshCw, ShieldAlert, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { InstanceId, LpChange, LpNode, LpSnapshot, LpTarget, LuckPermsInfo, LuckPermsPreview } from "../api/types";
import { Dialog } from "./Dialog";

const GROUP_RE = /^[a-z0-9_-]{1,36}$/;
const NODE_RE = /^[^\s"'\\]{1,200}$/;

export function contextText(ctx: Record<string, string[]>): string {
  return Object.entries(ctx)
    .flatMap(([k, vs]) => vs.map((v) => `${k}=${v}`))
    .join(", ");
}

function expiryText(unix: number): string {
  return `caduca el ${new Date(unix * 1000).toLocaleString()}`;
}

function NodeTable({
  nodes,
  onUnset,
  onToggle,
}: {
  nodes: LpNode[];
  onUnset: (n: LpNode) => void;
  onToggle: (n: LpNode) => void;
}) {
  if (nodes.length === 0) return <p className="field-help">Sin permisos propios.</p>;
  return (
    <table className="history-table lp-nodes">
      <thead>
        <tr>
          <th scope="col">Permiso</th>
          <th scope="col">Valor</th>
          <th scope="col">Dónde / hasta cuándo</th>
          <th scope="col">
            <span className="sr-only">Acciones</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {nodes.map((n) => {
          const where = [contextText(n.context), n.expiry ? expiryText(n.expiry) : ""].filter(Boolean).join(" · ");
          const editable = n.kind === "permission" && n.expiry === null;
          return (
            <tr key={`${n.key}|${contextText(n.context)}|${n.expiry ?? ""}`}>
              <th scope="row">
                <code>{n.key}</code>
                {n.kind !== "permission" && <span className="chip chip-kind lp-kind">{n.kind}</span>}
              </th>
              <td>{n.value ? "Sí" : "No"}</td>
              <td>{where || "En todo el servidor"}</td>
              <td className="lp-actions">
                {editable && Object.keys(n.context).length === 0 && (
                  <button type="button" className="btn btn-sm btn-quiet" onClick={() => onToggle(n)} aria-label={`Cambiar ${n.key} a ${n.value ? "No" : "Sí"}`}>
                    Cambiar a {n.value ? "No" : "Sí"}
                  </button>
                )}
                {n.expiry === null && (
                  <button type="button" className="btn btn-sm btn-quiet" onClick={() => onUnset(n)} aria-label={`Quitar ${n.key}`}>
                    <Trash2 size={14} aria-hidden="true" /> Quitar
                  </button>
                )}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function AddPermission({ onAdd }: { onAdd: (node: string, value: boolean) => void }) {
  const [node, setNode] = useState("");
  const [value, setValue] = useState(true);
  const lower = node.trim().toLowerCase();
  const err = !node.trim()
    ? null
    : !NODE_RE.test(node.trim())
      ? "Sin espacios ni comillas, hasta 200 caracteres."
      : lower.startsWith("group.")
        ? "Para heredar un grupo usa «Añadir grupo»."
        : null;
  return (
    <form
      className="lp-add"
      onSubmit={(e) => {
        e.preventDefault();
        if (node.trim() && !err) {
          onAdd(node.trim(), value);
          setNode("");
        }
      }}
    >
      <div className="field">
        <label htmlFor="lp-node">Permiso nuevo</label>
        <input id="lp-node" type="text" placeholder="essentials.fly" value={node} onChange={(e) => setNode(e.target.value)} />
        {err && <span className="field-help">{err}</span>}
      </div>
      <div className="field">
        <label htmlFor="lp-value">Valor</label>
        <select id="lp-value" value={value ? "true" : "false"} onChange={(e) => setValue(e.target.value === "true")}>
          <option value="true">Permitir</option>
          <option value="false">Negar</option>
        </select>
      </div>
      <button type="submit" className="btn btn-sm" disabled={!node.trim() || !!err}>
        <Plus size={14} aria-hidden="true" /> Añadir permiso
      </button>
    </form>
  );
}

function Parents({
  parents,
  all,
  exclude,
  onAdd,
  onRemove,
}: {
  parents: string[];
  all: string[];
  exclude?: string;
  onAdd: (g: string) => void;
  onRemove: (g: string) => void;
}) {
  const options = all.filter((g) => g !== exclude && !parents.includes(g));
  const [pick, setPick] = useState("");
  const chosen = options.includes(pick) ? pick : (options[0] ?? "");
  return (
    <div className="lp-parents">
      <div className="lp-chips">
        {parents.length === 0 && <span className="field-help">Ninguno.</span>}
        {parents.map((p) => (
          <span key={p} className="chip">
            {p}
            <button type="button" className="chip-x" aria-label={`Quitar el grupo ${p}`} onClick={() => onRemove(p)}>
              <X size={12} aria-hidden="true" />
            </button>
          </span>
        ))}
      </div>
      {options.length > 0 && (
        <div className="lp-inline field">
          <label htmlFor="lp-parent" className="sr-only">
            Grupo para añadir
          </label>
          <select id="lp-parent" value={chosen} onChange={(e) => setPick(e.target.value)}>
            {options.map((g) => (
              <option key={g}>{g}</option>
            ))}
          </select>
          <button type="button" className="btn btn-sm" onClick={() => chosen && onAdd(chosen)}>
            <Plus size={14} aria-hidden="true" /> Añadir grupo
          </button>
        </div>
      )}
    </div>
  );
}

export function LuckPermsPanel(props: Parameters<typeof LuckPermsInner>[0]) {
  return (
    <div className="admin-panel" role="tabpanel" aria-label="Permisos (LuckPerms)">
      <LuckPermsInner {...props} />
    </div>
  );
}

function LuckPermsInner({
  sup,
  id,
  online,
  onSearchPlugin,
}: {
  sup: Supervisor;
  id: InstanceId;
  online: boolean;
  /** Abre el buscador de plugins o mods con «LuckPerms», si este servidor tiene uno. */
  onSearchPlugin?: () => void;
}) {
  const [info, setInfo] = useState<LuckPermsInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [view, setView] = useState<"groups" | "users">("groups");
  const [group, setGroup] = useState<string | null>(null);
  const [user, setUser] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [newGroup, setNewGroup] = useState<string | null>(null);
  const [pending, setPending] = useState<{ change: LpChange; preview: LuckPermsPreview | null; error: string | null; busy: boolean } | null>(null);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);

  const load = useCallback(() => {
    setLoading(true);
    setError(null);
    sup.luckPerms(id).then(
      (i) => {
        setInfo(i);
        setLoading(false);
      },
      (e) => {
        setError(errorMessage(e));
        setLoading(false);
      },
    );
  }, [sup, id]);
  useEffect(() => {
    load();
  }, [load, online]);

  const snap: LpSnapshot | null = info?.snapshot ?? null;
  const groupNames = useMemo(() => snap?.groups.map((g) => g.name) ?? [], [snap]);
  const selGroup = snap?.groups.find((g) => g.name === group) ?? snap?.groups[0] ?? null;
  const users = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return (snap?.users ?? []).filter((u) => !q || (u.username ?? u.uuid).toLowerCase().includes(q));
  }, [snap, filter]);
  const selUser = snap?.users.find((u) => u.uuid === user) ?? users[0] ?? null;

  async function propose(change: LpChange) {
    setResult(null);
    setPending({ change, preview: null, error: null, busy: true });
    try {
      const preview = await sup.luckPermsPreview(id, change);
      setPending({ change, preview, error: null, busy: false });
    } catch (e) {
      setPending({ change, preview: null, error: errorMessage(e), busy: false });
    }
  }

  async function apply() {
    if (!pending?.preview) return;
    const { change, preview } = pending;
    setPending({ ...pending, busy: true });
    try {
      const r = await sup.luckPermsApply(id, change);
      setInfo((i) => (i ? { ...i, snapshot: r.snapshot, backups: [r.backup, ...i.backups.filter((b) => b !== r.backup)].slice(0, 10) } : i));
      setResult(
        r.verified
          ? { ok: true, text: `Hecho y comprobado: ${preview.summary}` }
          : { ok: false, text: "LuckPerms todavía no muestra el cambio. Mira la consola por si dio un error y pulsa Actualizar." },
      );
      if (change.type === "create_group") setGroup(change.group);
      if (change.type === "delete_group") setGroup(null);
      setPending(null);
    } catch (e) {
      setPending({ ...pending, busy: false, error: errorMessage(e) });
    }
  }

  const target = (kind: "group" | "user", name: string): LpTarget => ({ kind, name });
  const nodeHandlers = (t: LpTarget) => ({
    onUnset: (n: LpNode) => propose({ type: "unset_permission", target: t, node: n.key, context: n.context }),
    onToggle: (n: LpNode) => propose({ type: "set_permission", target: t, node: n.key, value: !n.value }),
  });

  if (!info) {
    return (
      <section className="card" aria-label="Permisos con LuckPerms">
        {error ? (
          <p className="form-error" role="alert">
            {error}
          </p>
        ) : (
          <p className="field-help">Leyendo los permisos…</p>
        )}
        {error && (
          <button type="button" className="btn btn-sm" onClick={load}>
            Reintentar
          </button>
        )}
      </section>
    );
  }

  if (!info.install) {
    return (
      <section className="card lp-card" aria-label="Permisos con LuckPerms">
        <h2>
          <KeyRound size={18} aria-hidden="true" /> Permisos con LuckPerms
        </h2>
        <p>
          LuckPerms no está instalado en este servidor. Sin él, los permisos son los de Minecraft: operador o no (pestaña Jugadores). Con
          LuckPerms puedes armar grupos (por ejemplo «vip» o «moderador») con permisos de cada plugin.
        </p>
        {onSearchPlugin ? (
          <button type="button" className="btn" onClick={onSearchPlugin}>
            Buscar LuckPerms
          </button>
        ) : (
          <p className="field-help">Este tipo de servidor no admite plugins ni mods, así que no puede usar LuckPerms.</p>
        )}
      </section>
    );
  }

  return (
    <section className="card lp-card" aria-label="Permisos con LuckPerms">
      <header className="lp-head">
        <h2>
          <KeyRound size={18} aria-hidden="true" /> Permisos con LuckPerms
        </h2>
        <span className="chip">{info.install.jar}</span>
        {snap && (
          <>
            <button type="button" className="btn btn-sm btn-quiet" onClick={load} disabled={loading}>
              <RefreshCw size={14} aria-hidden="true" /> Actualizar
            </button>
            <button type="button" className="btn btn-sm" onClick={() => setNewGroup("")}>
              <Plus size={14} aria-hidden="true" /> Crear grupo
            </button>
          </>
        )}
      </header>

      {!info.online && (
        <p className="field-help">
          Enciende el servidor para ver y cambiar los permisos: LuckPerms los guarda en su propia base de datos y MercHosting se los pide
          a él.
        </p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {result && (
        <div className={`notice ${result.ok ? "" : "notice-warn"}`} role="status">
          {result.ok ? <CheckCircle2 size={18} aria-hidden="true" className="notice-icon" /> : <ShieldAlert size={18} aria-hidden="true" className="notice-icon" />}
          <p>{result.text}</p>
        </div>
      )}

      {snap && (
        <>
          <div className="seg" role="group" aria-label="Ver">
            <button type="button" className="seg-btn" aria-pressed={view === "groups"} onClick={() => setView("groups")}>
              Grupos ({snap.groups.length})
            </button>
            <button type="button" className="seg-btn" aria-pressed={view === "users"} onClick={() => setView("users")}>
              Usuarios ({snap.users.length})
            </button>
          </div>

          {view === "groups" && selGroup && (
            <div className="lp-split">
              <ul className="lp-list" aria-label="Grupos">
                {snap.groups.map((g) => (
                  <li key={g.name}>
                    <button type="button" aria-pressed={g.name === selGroup.name} onClick={() => setGroup(g.name)}>
                      <strong>{g.name}</strong>
                      {g.weight !== null && <span className="field-help"> peso {g.weight}</span>}
                    </button>
                  </li>
                ))}
              </ul>
              <div className="lp-detail" aria-label={`Grupo ${selGroup.name}`} role="region">
                <h3>{selGroup.name}</h3>
                {selGroup.prefix && (
                  <p className="field-help">
                    Prefijo en el chat: <code>{selGroup.prefix}</code>
                  </p>
                )}
                <h4>Hereda de</h4>
                <Parents
                  key={`g-${selGroup.name}`}
                  parents={selGroup.parents}
                  all={groupNames}
                  exclude={selGroup.name}
                  onAdd={(g) => propose({ type: "add_parent", target: target("group", selGroup.name), group: g })}
                  onRemove={(g) => propose({ type: "remove_parent", target: target("group", selGroup.name), group: g })}
                />
                <h4>Permisos</h4>
                <NodeTable nodes={selGroup.nodes} {...nodeHandlers(target("group", selGroup.name))} />
                <AddPermission onAdd={(node, value) => propose({ type: "set_permission", target: target("group", selGroup.name), node, value })} />
                {selGroup.name !== "default" && (
                  <button type="button" className="btn btn-sm btn-stop" onClick={() => propose({ type: "delete_group", group: selGroup.name })}>
                    <Trash2 size={14} aria-hidden="true" /> Borrar el grupo {selGroup.name}
                  </button>
                )}
              </div>
            </div>
          )}

          {view === "users" && (
            <div className="lp-split">
              <div>
                <div className="field">
                  <label htmlFor="lp-filter">Buscar usuario</label>
                  <input id="lp-filter" type="search" value={filter} onChange={(e) => setFilter(e.target.value)} />
                </div>
                {users.length === 0 && <p className="field-help">{snap.users.length ? "Nadie coincide." : "LuckPerms no tiene usuarios guardados todavía: aparecen cuando entran al servidor."}</p>}
                <ul className="lp-list" aria-label="Usuarios">
                  {users.map((u) => (
                    <li key={u.uuid}>
                      <button type="button" aria-pressed={u.uuid === selUser?.uuid} onClick={() => setUser(u.uuid)}>
                        <strong>{u.username ?? u.uuid}</strong>
                        <span className="field-help"> {u.primary_group}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
              {selUser && (
                <div className="lp-detail" aria-label={`Usuario ${selUser.username ?? selUser.uuid}`} role="region">
                  <h3>{selUser.username ?? "Sin nombre"}</h3>
                  <p className="field-help">
                    <code>{selUser.uuid}</code> · grupo principal: {selUser.primary_group}
                  </p>
                  <h4>Grupos</h4>
                  <Parents
                    key={`u-${selUser.uuid}`}
                    parents={selUser.parents}
                    all={groupNames}
                    onAdd={(g) => propose({ type: "add_parent", target: target("user", selUser.uuid), group: g })}
                    onRemove={(g) => propose({ type: "remove_parent", target: target("user", selUser.uuid), group: g })}
                  />
                  <h4>Permisos propios</h4>
                  <NodeTable nodes={selUser.nodes} {...nodeHandlers(target("user", selUser.uuid))} />
                  <AddPermission onAdd={(node, value) => propose({ type: "set_permission", target: target("user", selUser.uuid), node, value })} />
                </div>
              )}
            </div>
          )}

          {snap.tracks.length > 0 && (
            <p className="field-help lp-tracks">
              Escalas de rangos:{" "}
              {snap.tracks.map((t) => (
                <span key={t.name}>
                  <strong>{t.name}</strong> ({t.groups.join(" → ")}){" "}
                </span>
              ))}
            </p>
          )}
        </>
      )}

      {info.backups.length > 0 && (
        <p className="field-help">
          Hay {info.backups.length} {info.backups.length === 1 ? "copia" : "copias"} de los permisos guardadas antes de cambios (la más
          reciente: {info.backups[0]}). Están en la carpeta de datos de MercHosting, en «luckperms-copias», y LuckPerms las puede cargar
          con «luckperms import» si las copias a {info.install.data_dir}.
        </p>
      )}

      {newGroup !== null && (
        <Dialog
          title="Crear grupo"
          lead="Se crea vacío: después le añades permisos y de qué grupos hereda."
          onClose={() => setNewGroup(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setNewGroup(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-go"
                disabled={!GROUP_RE.test(newGroup) || groupNames.includes(newGroup)}
                onClick={() => {
                  const g = newGroup;
                  setNewGroup(null);
                  void propose({ type: "create_group", group: g });
                }}
              >
                Continuar
              </button>
            </>
          }
        >
          <div className="field">
            <label htmlFor="lp-new">Nombre del grupo</label>
            <input id="lp-new" type="text" maxLength={36} value={newGroup} onChange={(e) => setNewGroup(e.target.value.toLowerCase())} />
            <span className="field-help">
              {groupNames.includes(newGroup) ? "Ya existe un grupo con ese nombre." : "Minúsculas, números, _ y - (hasta 36)."}
            </span>
          </div>
        </Dialog>
      )}

      {pending && (
        <Dialog
          title="Revisar el cambio"
          lead="Esto es exactamente lo que se le pedirá a LuckPerms. Antes se guarda una copia de todos los permisos."
          onClose={() => setPending(null)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setPending(null)}>
                Cancelar
              </button>
              <button
                type="button"
                className={pending.change.type === "delete_group" ? "btn btn-stop" : "btn btn-go"}
                disabled={!pending.preview || pending.busy}
                onClick={apply}
              >
                {pending.change.type === "delete_group" ? "Borrar grupo" : "Aplicar"}
              </button>
            </>
          }
        >
          {pending.busy && !pending.preview && <p className="field-help">Comprobando con LuckPerms…</p>}
          {pending.preview && (
            <>
              <p>{pending.preview.summary}</p>
              <p className="field-help">
                Comando: <code>{pending.preview.command}</code>
              </p>
            </>
          )}
          {pending.busy && pending.preview && <p className="field-help">Aplicando y comprobando…</p>}
          {pending.error && (
            <p className="form-error" role="alert">
              {pending.error}
            </p>
          )}
        </Dialog>
      )}
    </section>
  );
}
