// Arrastrar archivos desde el Explorador de Windows sobre un panel. Solo escucha mientras el
// panel está montado y `enabled`; al soltar entrega las rutas tal cual (quien llama filtra).

import { useEffect, useRef, useState } from "react";
import type { MercApi } from "./api/client";

export function useFileDrop(api: MercApi, enabled: boolean, onDrop: (paths: string[]) => void): { dragging: boolean; count: number } {
  const [state, setState] = useState({ dragging: false, count: 0 });
  const cb = useRef(onDrop);
  cb.current = onDrop;
  useEffect(() => {
    if (!enabled) {
      setState({ dragging: false, count: 0 });
      return;
    }
    return api.onFileDrop((e) => {
      if (e.type === "enter") setState({ dragging: true, count: e.paths.length });
      else if (e.type === "leave") setState({ dragging: false, count: 0 });
      else {
        setState({ dragging: false, count: 0 });
        if (e.paths.length) cb.current(e.paths);
      }
    });
  }, [api, enabled]);
  return state;
}

export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Copia cada archivo a `destDir` del servidor, uno por uno; nunca sobrescribe (lo rechaza el supervisor). */
export async function uploadAll(
  upload: (src: string) => Promise<string>,
  paths: string[],
): Promise<{ ok: string[]; failed: { name: string; error: string }[] }> {
  const ok: string[] = [];
  const failed: { name: string; error: string }[] = [];
  for (const p of paths) {
    try {
      await upload(p);
      ok.push(baseName(p));
    } catch (e) {
      failed.push({ name: baseName(p), error: e instanceof Error ? e.message : String(e) });
    }
  }
  return { ok, failed };
}

export function dropSummary(ok: string[], failed: { name: string; error: string }[], skipped: string[], what: string): string {
  const parts: string[] = [];
  if (ok.length) parts.push(`Copiado${ok.length > 1 ? "s" : ""} a ${what}: ${ok.join(", ")}.`);
  if (failed.length) parts.push(`No se copiaron: ${failed.map((f) => `${f.name} (${f.error})`).join("; ")}.`);
  if (skipped.length) parts.push(`Ignorados por no ser del tipo correcto: ${skipped.join(", ")}.`);
  return parts.join(" ");
}
