// Buscar un paquete de recursos en Modrinth y elegir una versión. Al elegir, quien llama lo pone
// con SetResourcePack (descarga, comprueba que es un paquete y calcula su SHA-1).

import { ArrowLeft, ExternalLink, Search } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { Supervisor } from "../api/client";
import { errorMessage } from "../api/client";
import type { ContentHit, ContentProject, ContentVersion, InstanceId } from "../api/types";
import { formatSize } from "../format";
import { Dialog } from "./Dialog";
import { uiLocale } from "../i18n/lang";

interface Props {
  sup: Supervisor;
  id: InstanceId;
  mcVersion: string | null;
  openUrl?: (url: string) => void;
  onPick: (url: string, label: string) => void;
  onClose: () => void;
}

const fmt = (n: number) => new Intl.NumberFormat(uiLocale(), { notation: "compact" }).format(n);

export function ResourcePackBrowser({ sup, id, mcVersion, openUrl, onPick, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ContentHit[] | null>(null);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [detail, setDetail] = useState<{ project: ContentProject; versions: ContentVersion[] } | null>(null);
  const [loading, setLoading] = useState(false);

  const search = useCallback(
    async (q: string) => {
      setLoading(true);
      setError(null);
      try {
        const r = await sup.resourcePackSearch(id, q, q.trim() ? "relevance" : "downloads");
        setHits(r.hits);
        setTotal(r.total);
      } catch (e) {
        setError(errorMessage(e));
      }
      setLoading(false);
    },
    [sup, id],
  );
  useEffect(() => {
    void search("");
  }, [search]);

  async function openProject(h: ContentHit) {
    setError(null);
    setLoading(true);
    try {
      setDetail(await sup.resourcePackProject(id, h.project_id));
    } catch (e) {
      setError(errorMessage(e));
    }
    setLoading(false);
  }

  const fits = (v: ContentVersion) => !mcVersion || v.game_versions.includes(mcVersion);

  return (
    <Dialog
      wide
      title={detail ? detail.project.title : "Paquetes de recursos de Modrinth"}
      lead={
        detail
          ? detail.project.description
          : `Texturas y sonidos que se ofrecen a cada jugador al entrar${mcVersion ? `, para Minecraft ${mcVersion}` : ""}. Solo se usa el enlace oficial de Modrinth.`
      }
      onClose={onClose}
      footer={
        <button type="button" className="btn" onClick={onClose}>
          Cerrar
        </button>
      }
    >
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {detail ? (
        <div className="rp-detail">
          <div className="files-bar">
            <button type="button" className="btn btn-sm btn-quiet" onClick={() => setDetail(null)}>
              <ArrowLeft size={14} aria-hidden="true" /> Volver a la lista
            </button>
            <span className="field-help">Licencia: {detail.project.license || "sin indicar"}</span>
            {openUrl && (
              <button type="button" className="btn btn-sm" onClick={() => openUrl(detail.project.page_url)}>
                <ExternalLink size={14} aria-hidden="true" /> Ver en Modrinth
              </button>
            )}
          </div>
          {detail.versions.length === 0 ? (
            <p className="field-help">Este paquete no tiene versiones publicadas.</p>
          ) : (
            <ul className="rp-versions" aria-label="Versiones">
              {detail.versions.map((v) => {
                const file = v.files.find((f) => f.primary) ?? v.files[0];
                const ok = fits(v) && !!file && file.url.startsWith("https://");
                return (
                  <li key={v.id} className="rp-version" data-fits={fits(v)}>
                    <span className="rp-version-main">
                      <b>{v.version_number}</b>
                      <span className="field-help">
                        Minecraft {v.game_versions.slice(0, 6).join(", ")}
                        {v.game_versions.length > 6 ? "…" : ""}
                        {file ? ` · ${formatSize(file.size)}` : ""}
                      </span>
                      {!fits(v) && mcVersion && <span className="field-help warn-text">No es para Minecraft {mcVersion}.</span>}
                    </span>
                    <button
                      type="button"
                      className="btn btn-sm btn-go"
                      disabled={!ok}
                      onClick={() => file && onPick(file.url, `${detail.project.title} ${v.version_number}`)}
                      aria-label={`Usar ${detail.project.title} ${v.version_number}`}
                    >
                      Usar este
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      ) : (
        <>
          <form
            className="console-search rp-search"
            role="search"
            onSubmit={(e) => {
              e.preventDefault();
              void search(query);
            }}
          >
            <Search size={15} aria-hidden="true" />
            <input type="search" aria-label="Buscar paquetes de recursos" placeholder="Buscar (por ejemplo: faithful)" value={query} onChange={(e) => setQuery(e.target.value)} />
          </form>
          {loading && !hits && (
            <p className="wiz-loading" role="status">
              Buscando en Modrinth…
            </p>
          )}
          {hits && hits.length === 0 && <p className="field-help">No se encontró nada{mcVersion ? ` para Minecraft ${mcVersion}` : ""}.</p>}
          {hits && hits.length > 0 && (
            <>
              <p className="field-help">{total} resultado{total === 1 ? "" : "s"}</p>
              <ul className="rp-hits" aria-label="Resultados">
                {hits.map((h) => (
                  <li key={h.project_id}>
                    <button type="button" className="rp-hit" onClick={() => openProject(h)} disabled={loading}>
                      {h.icon_url ? <img src={h.icon_url} alt="" width={36} height={36} loading="lazy" /> : <span className="rp-noicon" aria-hidden="true" />}
                      <span className="rp-hit-main">
                        <b>{h.title}</b>
                        <span className="field-help">
                          de {h.author} · {fmt(h.downloads)} descargas
                        </span>
                        <span className="field-help">{h.description}</span>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      )}
    </Dialog>
  );
}
