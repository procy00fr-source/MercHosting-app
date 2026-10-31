import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import type { Diagnosis } from "./api/types";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

const DIAG: Diagnosis = {
  checked_at_unix_ms: 1,
  sources: ["mods/ (3 archivos)", "crash-reports/crash-2026-10-30_16.00.00-server.txt (30/10 16:00)"],
  mods_scanned: 3,
  plugins_scanned: 0,
  findings: [
    {
      code: "duplicate",
      title: "El mod «jei» está 2 veces",
      confidence: "alta",
      explanation: "Deja solo uno.",
      evidence: ["mods/jei-19.0.jar (versión 19.0)", "mods/jei-19.1.jar (versión 19.1)"],
      source: "mods/",
      actions: [
        { type: "disable_content", kind: "mod", file: "jei-19.0.jar" },
        { type: "disable_content", kind: "mod", file: "jei-19.1.jar" },
      ],
    },
    {
      code: "missing_dependency",
      title: "Falta «geckolib», que necesita Mi Mod",
      confidence: "alta",
      explanation: "Instálalo.",
      evidence: ["mods/mymod.jar: requiere «geckolib»"],
      source: "mods/",
      actions: [{ type: "search_content", kind: "mod", query: "geckolib" }],
    },
    {
      code: "out_of_memory",
      title: "Al servidor se le acabó la memoria",
      confidence: "alta",
      explanation: "Sube la memoria.",
      evidence: ["java.lang.OutOfMemoryError: Java heap space"],
      source: "crash-reports/crash-2026-10-30_16.00.00-server.txt",
      actions: [{ type: "set_memory", max_mb: 6144 }],
    },
  ],
};

async function open(setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "neo_forge" }, { state: "CRASHED" }));
  fake.diagnosis = structuredClone(DIAG);
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  return { fake, user };
}

describe("Diagnóstico", () => {
  it("tras un cierre inesperado lleva al diagnóstico, con la prueba de cada problema", async () => {
    const { user } = await open();
    await user.click(await screen.findByRole("button", { name: /Ver diagnóstico/ }));
    const panel = within(await screen.findByRole("tabpanel", { name: "Diagnóstico" }));
    const list = within(await panel.findByRole("list", { name: "Problemas encontrados" }));
    expect(list.getByText("El mod «jei» está 2 veces")).toBeInTheDocument();
    expect(list.getAllByText("Confianza alta")).toHaveLength(3);
    expect(list.getByText(/mods\/jei-19.0.jar \(versión 19.0\)/)).toBeInTheDocument();
    expect(list.getByText(/java.lang.OutOfMemoryError/)).toBeInTheDocument();
    expect(panel.getByText(/crash-reports\/crash-2026-10-30_16.00.00-server.txt \(30\/10 16:00\)/)).toBeInTheDocument();
  });

  it("los arreglos se confirman y son reversibles", async () => {
    const { fake, user } = await open();
    await user.click(await screen.findByRole("tab", { name: "Diagnóstico" }));
    const panel = within(await screen.findByRole("tabpanel", { name: "Diagnóstico" }));
    await user.click(await panel.findByRole("button", { name: "Desactivar jei-19.0.jar" }));
    const dlg = within(screen.getByRole("dialog", { name: "Desactivar jei-19.0.jar" }));
    expect(dlg.getByText(/no se borra nada/)).toBeInTheDocument();
    expect(fake.calls.some((c) => c.op === "content_toggle")).toBe(false);
    await user.click(dlg.getByRole("button", { name: "Aplicar" }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({ op: "content_toggle", id, kind: "mod", file_name: "jei-19.0.jar", enabled: false }),
    );
    expect(await panel.findByText(/jei-19.0.jar desactivado/)).toBeInTheDocument();

    await user.click(panel.getByRole("button", { name: "Subir la memoria a 6 GB" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Aplicar" }));
    await waitFor(() => expect(fake.calls).toContainEqual({ op: "set_launch", id, max_memory_mb: 6144 }));
  });

  it("buscar la dependencia abre el catálogo de mods con la búsqueda hecha", async () => {
    const { fake, user } = await open();
    await user.click(await screen.findByRole("tab", { name: "Diagnóstico" }));
    const panel = within(await screen.findByRole("tabpanel", { name: "Diagnóstico" }));
    await user.click(await panel.findByRole("button", { name: "Buscar «geckolib»" }));
    await waitFor(() => expect(fake.calls.some((c) => c.op === "content_search" && c.query === "geckolib")).toBe(true));
  });

  it("sin problemas no promete que todo esté bien", async () => {
    const { user } = await open((f) => {
      f.diagnosis = { ...DIAG, findings: [] };
    });
    await user.click(await screen.findByRole("tab", { name: "Diagnóstico" }));
    expect(await screen.findByText("No se encontraron problemas conocidos.")).toBeInTheDocument();
    expect(screen.getByText(/no garantiza que todo esté bien/)).toBeInTheDocument();
  });

  it("Ajustes › Java y memoria guarda la memoria con el servidor detenido", async () => {
    const { fake, user } = await open((f) => {
      f.servers[0].status = { ...f.servers[0].status, state: "STOPPED" };
    });
    await user.click(await screen.findByRole("tab", { name: "Ajustes" }));
    const card = within(await screen.findByRole("region", { name: "Java y memoria" }));
    await user.selectOptions(card.getByRole("combobox", { name: "Memoria máxima" }), "8192");
    await user.click(card.getByRole("button", { name: "Guardar memoria" }));
    await waitFor(() => expect(fake.calls).toContainEqual({ op: "set_launch", id, max_memory_mb: 8192 }));
  });
});
