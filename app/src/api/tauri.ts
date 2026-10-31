// Implementación real de MercApi sobre los comandos Tauri de app/src-tauri/src/bridge.rs.

import { defaultWindowIcon } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { FileDropEvent, MercApi } from "./client";
import type { AppUpdateCheck, JavaChoice, RequestBody, ResponseBody, SupervisorInfo } from "./types";
import { MercError } from "./types";

function toMercError(e: unknown): MercError {
  if (typeof e === "object" && e !== null && "code" in e && "message" in e) {
    const { code, message } = e as { code: string; message: string };
    return new MercError(code as MercError["code"], message);
  }
  return new MercError("internal", String(e));
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toMercError(e);
  }
}

/** Dibuja un SVG en un lienzo y lo devuelve como PNG. */
async function svgToPng(svg: string, size: number): Promise<Uint8Array> {
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  try {
    const img = new Image(size, size);
    img.src = url;
    await img.decode();
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    canvas.getContext("2d")!.drawImage(img, 0, 0, size, size);
    const blob = await new Promise<Blob | null>((r) => canvas.toBlob(r, "image/png"));
    if (!blob) throw new Error("No se pudo crear el icono");
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    URL.revokeObjectURL(url);
  }
}

export const tauriApi: MercApi = {
  call: (body: RequestBody) => call<ResponseBody>("supervisor_call", { body }),
  info: () => call<SupervisorInfo>("supervisor_info"),
  javaFor: (mcVersion: string | null) => call<JavaChoice>("java_for", { mcVersion }),
  async pickFolder() {
    const r = await open({ directory: true, multiple: false, title: "Elige la carpeta del servidor" });
    return typeof r === "string" ? r : null;
  },
  async pickFile(filter?: { name: string; extensions: string[] }) {
    const r = await open({
      directory: false,
      multiple: false,
      title: filter ? `Elige un archivo (${filter.name})` : "Elige el archivo que quieres subir al servidor",
      filters: filter ? [filter] : undefined,
    });
    return typeof r === "string" ? r : null;
  },
  async pickSavePath(defaultName: string) {
    const r = await save({ defaultPath: defaultName, title: "Guardar una copia" });
    return typeof r === "string" ? r : null;
  },
  openUrl: (url: string) => openUrl(url),
  openServerFolder: (id, rel) => call<void>("open_server_folder", { id, rel }),
  openBackupsFolder: (id) => call<void>("open_backups_folder", { id }),
  appUpdateCheck: () => call<AppUpdateCheck>("app_update_check"),
  async appUpdateInstall(onProgress) {
    const stop = await listen<{ downloaded: number; total: number | null }>("app-update-progress", (e) =>
      onProgress(e.payload.downloaded, e.payload.total),
    );
    try {
      await call<void>("app_update_install");
    } finally {
      stop();
    }
  },
  async setWindowIcon(svg: string | null) {
    const win = getCurrentWindow();
    if (svg === null) {
      const icon = await defaultWindowIcon();
      if (icon) await win.setIcon(icon);
      return;
    }
    await win.setIcon(await svgToPng(svg, 256));
  },
  async notify(title: string, body: string) {
    try {
      let ok = await isPermissionGranted();
      if (!ok) ok = (await requestPermission()) === "granted";
      if (ok) sendNotification({ title, body });
      return ok;
    } catch {
      return false;
    }
  },
  onFileDrop(handler: (e: FileDropEvent) => void) {
    let stop: (() => void) | null = null;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((ev) => {
        const p = ev.payload;
        if (p.type === "enter") handler({ type: "enter", paths: p.paths });
        else if (p.type === "drop") handler({ type: "drop", paths: p.paths });
        else if (p.type === "leave") handler({ type: "leave" });
      })
      .then((un) => {
        if (cancelled) un();
        else stop = un;
      })
      .catch(() => {
        /* sin arrastrar y soltar: sigue habiendo «Subir archivo» */
      });
    return () => {
      cancelled = true;
      stop?.();
    };
  },
};
