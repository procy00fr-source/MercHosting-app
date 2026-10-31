import "@fontsource-variable/unbounded";
import "@fontsource-variable/manrope";
import "@fontsource-variable/jetbrains-mono";
import "./styles.css";
import "./light.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { tauriApi } from "./api/tauri";
import { startDomTranslator } from "./i18n/dom";
import { uiLang } from "./i18n/lang";
import { setActiveTranslator, Translator } from "./i18n/translate";

async function boot() {
  const root = document.getElementById("root")!;
  if (uiLang() === "en") {
    document.documentElement.lang = "en";
    try {
      const dict = (await import("./i18n/en.json")).default;
      const tr = new Translator(dict as never);
      setActiveTranslator(tr);
      startDomTranslator(document.body, tr);
    } catch {
      /* sin diccionario: la interfaz sigue en español */
    }
  }
  createRoot(root).render(
    <StrictMode>
      <App api={tauriApi} />
    </StrictMode>,
  );
}

void boot();
