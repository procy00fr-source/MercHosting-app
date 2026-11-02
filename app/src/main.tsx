import "@fontsource-variable/unbounded";
import "@fontsource-variable/manrope";
import "@fontsource-variable/jetbrains-mono";
import "./styles.css";
import "./light.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { tauriApi } from "./api/tauri";
import { mobileApi, mobileIdentity } from "./api/mobile";
import { phoneApi } from "./api/phone";
import { MobileApp } from "./components/MobileApp";
import { isMobile, platform } from "./platform";
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
  if (isMobile()) document.documentElement.dataset.mobile = "true";
  createRoot(root).render(
    <StrictMode>{isMobile() ? <MobileApp api={mobileApi} identity={mobileIdentity} phone={platform() === "android" ? phoneApi : undefined} /> : <App api={tauriApi} />}</StrictMode>,
  );
}

void boot();
