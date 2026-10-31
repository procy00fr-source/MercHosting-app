import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Las pruebas comprueban los textos en español (el idioma por defecto).
Object.defineProperty(window.navigator, "language", { value: "es-ES", configurable: true });

afterEach(() => cleanup());
