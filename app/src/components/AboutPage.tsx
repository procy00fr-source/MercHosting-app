// «Acerca de»: versión, autor, licencia (GPL-3.0), código fuente, novedades, privacidad y
// componentes de terceros. Los textos salen de los mismos archivos del repositorio.

import { Code, ExternalLink, FileText, Heart, Info, MessageSquare, Package, Search, ShieldCheck } from "lucide-react";
import { useMemo, useState } from "react";
import changelog from "../../../CHANGELOG.md?raw";
import privacyEs from "../../../docs/PRIVACIDAD.md?raw";
import privacyEn from "../../../docs/PRIVACIDAD.en.md?raw";
import license from "../../../LICENSE?raw";
import type { MercApi } from "../api/client";
import { AUTHOR, LICENSE_ID, RELEASES_URL, SOURCE_URL } from "../about/links";
import { changelogVersions, Markdown } from "../about/markdown";
import thirdParty from "../about/terceros.json";
import { uiLang } from "../i18n/lang";
import { Dialog } from "./Dialog";
import { Logo } from "./Logo";
import { SOCIALS } from "./SupporterPage";

interface Props {
  api: MercApi;
  version: string | null;
  onOpenSupporter: () => void;
  onOpenFeedback?: () => void;
}

type Open = "license" | "privacy" | "third" | "changelog" | null;

export function AboutPage({ api, version, onOpenSupporter, onOpenFeedback }: Props) {
  const [open, setOpen] = useState<Open>(null);
  const [q, setQ] = useState("");
  const versions = useMemo(() => changelogVersions(changelog), []);
  const shown = useMemo(() => {
    const s = q.trim().toLowerCase();
    return s ? thirdParty.filter((t) => t.name.toLowerCase().includes(s) || t.license.toLowerCase().includes(s)) : thirdParty;
  }, [q]);
  const go = (url: string) => void api.openUrl(url);

  return (
    <div className="admin-panel companions-page about-page" role="region" aria-label="Acerca de">
      <header className="page-head about-head">
        <Logo size={72} />
        <div>
          <p className="page-eyebrow">
            <Info size={13} aria-hidden="true" /> Acerca de
          </p>
          <h1 className="page-title">MercHosting</h1>
          <p className="page-sub">
            {version ? `Versión ${version}` : "Versión desconocida"} · por <span translate="no">{AUTHOR}</span>
          </p>
        </div>
      </header>

      <p className="field-help">
        App gratuita para Windows y Linux que crea y administra servidores de Minecraft en tu propia PC. Minecraft es una marca de Mojang
        Studios y Microsoft; MercHosting no está afiliado a ellos.
      </p>

      <div className="about-grid">
        <section className="props-card" aria-label="Licencia y código fuente">
          <h2>
            <FileText size={16} aria-hidden="true" /> Licencia y código fuente
          </h2>
          <p>
            MercHosting es software libre bajo la licencia <strong translate="no">{LICENSE_ID}</strong>: puedes usarlo, estudiarlo,
            modificarlo y compartirlo, y las copias que distribuyas deben seguir siendo libres.
          </p>
          <div className="row-actions">
            <button type="button" className="btn btn-sm" onClick={() => setOpen("license")}>
              Ver la licencia
            </button>
            <button type="button" className="btn btn-sm" disabled={!SOURCE_URL} onClick={() => SOURCE_URL && go(SOURCE_URL)}>
              <Code size={14} aria-hidden="true" /> Código fuente
            </button>
            <button type="button" className="btn btn-sm btn-quiet" onClick={() => go(RELEASES_URL)}>
              <ExternalLink size={14} aria-hidden="true" /> Versiones publicadas
            </button>
          </div>
          {!SOURCE_URL && <p className="field-help">El repositorio público del código se está preparando: el enlace aparecerá aquí.</p>}
        </section>

        <section className="props-card" aria-label="Novedades">
          <h2>Novedades</h2>
          {versions.slice(0, 1).map((v) => (
            <div key={v.version}>
              <p>
                <strong>{v.version}</strong>
              </p>
              <ul className="about-list" translate="no">
                {v.items.slice(0, 5).map((it) => (
                  <li key={it}>{it}</li>
                ))}
              </ul>
            </div>
          ))}
          <button type="button" className="btn btn-sm btn-quiet" onClick={() => setOpen("changelog")}>
            Ver todas las versiones
          </button>
        </section>

        <section className="props-card" aria-label="Privacidad">
          <h2>
            <ShieldCheck size={16} aria-hidden="true" /> Privacidad
          </h2>
          <p>
            Sin cuentas, sin anuncios y sin telemetría. Tus servidores, mundos y copias se quedan en tu PC. MercHosting solo se
            conecta a Internet para lo que pides (descargas oficiales, catálogos, Playit, actualizaciones).
          </p>
          <button type="button" className="btn btn-sm" onClick={() => setOpen("privacy")}>
            Leer la política de privacidad
          </button>
        </section>

        <section className="props-card" aria-label="Componentes de terceros">
          <h2>
            <Package size={16} aria-hidden="true" /> Componentes de terceros
          </h2>
          <p>
            MercHosting usa {thirdParty.length} componentes de código abierto (Tauri, React, iroh, Rust…). Java, el agente de Playit,
            los programas de servidor, los mods y los plugins se descargan de su fuente oficial con su propia licencia.
          </p>
          <button type="button" className="btn btn-sm" onClick={() => setOpen("third")}>
            Ver la lista y sus licencias
          </button>
        </section>

        <section className="props-card" aria-label="Autor y comunidad">
          <h2>Autor y comunidad</h2>
          <p>
            Un proyecto de <strong translate="no">{AUTHOR}</strong>. Sígueme para ver novedades y tutoriales.
          </p>
          <div className="row-actions">
            {SOCIALS.map((s) => (
              <button key={s.name} type="button" className="btn btn-sm btn-quiet" onClick={() => go(s.url)}>
                <ExternalLink size={14} aria-hidden="true" /> {s.name} <span translate="no">{s.handle}</span>
              </button>
            ))}
          </div>
          <div className="row-actions">
            <button type="button" className="btn btn-sm" onClick={onOpenSupporter}>
              <Heart size={14} aria-hidden="true" /> Apoyar MercHosting
            </button>
            {onOpenFeedback && (
              <button type="button" className="btn btn-sm btn-quiet" onClick={onOpenFeedback}>
                <MessageSquare size={14} aria-hidden="true" /> Enviar un comentario
              </button>
            )}
          </div>
          <p className="field-help">Apoyar es opcional y solo cosmético: MercHosting es gratis y completo sin pagar.</p>
        </section>
      </div>

      {open === "license" && (
        <Dialog wide title={`Licencia (${LICENSE_ID})`} onClose={() => setOpen(null)} footer={<Close onClose={() => setOpen(null)} />}>
          <pre className="about-license" translate="no">
            {license}
          </pre>
        </Dialog>
      )}
      {open === "privacy" && (
        <Dialog wide title="Política de privacidad" onClose={() => setOpen(null)} footer={<Close onClose={() => setOpen(null)} />}>
          <div className="about-doc" translate="no">
            <Markdown text={uiLang() === "en" ? privacyEn : privacyEs} skipTitle />
          </div>
        </Dialog>
      )}
      {open === "changelog" && (
        <Dialog wide title="Todas las versiones" onClose={() => setOpen(null)} footer={<Close onClose={() => setOpen(null)} />}>
          <div className="about-doc" translate="no">
            <Markdown text={changelog} skipTitle />
          </div>
        </Dialog>
      )}
      {open === "third" && (
        <Dialog
          wide
          title="Componentes de terceros"
          lead={`${thirdParty.length} componentes incluidos en MercHosting, con su versión y licencia.`}
          onClose={() => setOpen(null)}
          footer={<Close onClose={() => setOpen(null)} />}
        >
          <div className="console-search rp-search">
            <Search size={15} aria-hidden="true" />
            <input type="search" aria-label="Buscar componente" placeholder="Buscar por nombre o licencia" value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          <div className="md-table-wrap about-third">
            <table className="md-table" translate="no">
              <thead>
                <tr>
                  <th>Componente</th>
                  <th>Versión</th>
                  <th>Licencia</th>
                </tr>
              </thead>
              <tbody>
                {shown.map((t) => (
                  <tr key={`${t.name}@${t.version}`}>
                    <td title={t.url}>{t.name}</td>
                    <td className="mono">{t.version}</td>
                    <td>{t.license}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Dialog>
      )}
    </div>
  );
}

function Close({ onClose }: { onClose: () => void }) {
  return (
    <button type="button" className="btn" onClick={onClose}>
      Cerrar
    </button>
  );
}
