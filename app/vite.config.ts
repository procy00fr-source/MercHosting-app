/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri espera el servidor de desarrollo en un puerto fijo.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // «Acerca de» incluye textos de la raíz del repositorio (licencia, novedades, privacidad).
  server: { port: 5173, strictPort: true, host: "127.0.0.1", fs: { allow: [".."] } },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
  },
});
