// MercApi de la app del celular (app/src-tauri/src/mobile.rs). No hay supervisor: solo existen
// las peticiones de Compañeros (unirse con un código, PCs guardadas y peticiones a una PC, que
// decide esa PC). Elegir archivos, carpetas o el Explorador no existe aquí.

import { invoke } from "@tauri-apps/api/core";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { MercApi } from "./client";
import type { AppUpdateCheck, RequestBody, ResponseBody } from "./types";
import { MercError } from "./types";

function toMercError(e: unknown): MercError {
  if (typeof e === "object" && e !== null && "code" in e && "message" in e) {
    const { code, message } = e as { code: string; message: string };
    return new MercError(code as MercError["code"], message);
  }
  return new MercError("internal", String(e));
}

const notOnPhone = async (): Promise<never> => {
  throw new MercError("bad_request", "En el celular no se puede: hazlo desde MercHosting en una PC.");
};

export async function mobileIdentity(): Promise<string> {
  try {
    return await invoke<string>("mobile_identity");
  } catch (e) {
    throw toMercError(e);
  }
}

export const mobileApi: MercApi = {
  async call(body: RequestBody) {
    try {
      return await invoke<ResponseBody>("mobile_call", { body });
    } catch (e) {
      throw toMercError(e);
    }
  },
  info: notOnPhone,
  javaFor: notOnPhone,
  pickFolder: async () => null,
  pickFile: async () => null,
  pickSavePath: async () => null,
  openUrl: (url: string) => openUrl(url),
  openServerFolder: notOnPhone,
  openBackupsFolder: notOnPhone,
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
  setWindowIcon: async () => undefined,
  // En el celular la app se actualiza instalando el APK nuevo (ver docs/MOVIL.md).
  appUpdateCheck: async (): Promise<AppUpdateCheck> => ({
    enabled: false,
    current: "",
    available: false,
    version: null,
    notes: null,
    date: null,
    self_install: false,
  }),
  appUpdateInstall: notOnPhone,
  onFileDrop: () => () => undefined,
  canTransfer: false,
};
