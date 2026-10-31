import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import type { ModSideReport } from "./api/types";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

function mod(p: Partial<ModSideReport> & { file: string }): ModSideReport {
  return { name: p.file, enabled: true, side: "unknown", confidence: "baja", evidence: [], needed_by: [], suggested: false, ...p };
}

async function open(setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "neo_forge" }));
  fake.modSidesState = {
    folder: "mods-cuarentena",
    catalog_ok: true,
    quarantined: [],
    items: [
      mod({
        file: "zoomify.jar",
        name: "Zoomify",
        side: "client_only",
        confidence: "alta",
        evidence: ["Modrinth: no funciona en servidores."],
        suggested: true,
      }),
      mod({ file: "minimapa.jar", name: "Minimapa", side: "client_only", confidence: "alta", needed_by: ["Waypoints Sync"] }),
      mod({ file: "lithium.jar", name: "Lithium", side: "both", confidence: "alta" }),
    ],
  };
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Mods" }));
  const panel = within(await screen.findByRole("tabpanel", { name: "Mods" }));
  return { fake, user, panel };
}

async function enable(user: ReturnType<typeof userEvent.setup>, panel: ReturnType<typeof within>) {
  await user.click(panel.getByRole("button", { name: /Separar cliente\/servidor/ }));
  await user.click(await panel.findByRole("button", { name: /Activar el separador/ }));
  return within(await panel.findByRole("region", { name: "Separador de mods" }));
}

describe("Separador de mods cliente/servidor", () => {
  beforeEach(() => localStorage.clear());

  it("viene apagado: no revisa nada ni avisa hasta que lo activas", async () => {
    const { fake, user, panel } = await open((f) => {
      f.installedMods[0].server_side = "unsupported";
    });
    await panel.findByText("Just Enough Items");
    expect(panel.queryByRole("note")).not.toBeInTheDocument();
    await user.click(panel.getByRole("button", { name: /Separar cliente\/servidor/ }));
    expect(await panel.findByText(/Separador de mods cliente\/servidor: apagado/)).toBeInTheDocument();
    expect(fake.calls.some((c) => c.op === "mod_sides")).toBe(false);
    await user.click(panel.getByRole("button", { name: /Activar el separador/ }));
    await panel.findByRole("checkbox", { name: "Mover Zoomify a la cuarentena" });
    expect(fake.calls.some((c) => c.op === "mod_sides")).toBe(true);
    await user.click(panel.getByRole("button", { name: /Desactivar el separador/ }));
    expect(await panel.findByRole("button", { name: /Activar el separador/ })).toBeInTheDocument();
  });

  it("clasifica con su porqué, preselecciona solo lo seguro y mueve a una cuarentena reversible", async () => {
    const { fake, user, panel } = await open();
    const sec = await enable(user, panel);
    expect(await sec.findByText(/no se borra nada/)).toBeInTheDocument();

    // Nada se marca solo: hay que pedirlo.
    expect(sec.getByRole("checkbox", { name: "Mover Zoomify a la cuarentena" })).not.toBeChecked();
    await user.click(sec.getByRole("button", { name: "Marcar los sugeridos (1)" }));
    expect(sec.getByRole("checkbox", { name: "Mover Zoomify a la cuarentena" })).toBeChecked();
    // Lo necesita otro mod: no se preselecciona y se avisa.
    expect(sec.getByRole("checkbox", { name: "Mover Minimapa a la cuarentena" })).not.toBeChecked();
    expect(sec.getByText(/Lo necesita: Waypoints Sync/)).toBeInTheDocument();
    expect(sec.getByRole("checkbox", { name: "Mover Lithium a la cuarentena" })).not.toBeChecked();

    const rows = sec.getAllByRole("button", { name: /Por qué/ });
    await user.click(rows[0]);
    expect(sec.getByText("Modrinth: no funciona en servidores.")).toBeInTheDocument();

    await user.click(sec.getByRole("button", { name: /Mover a la cuarentena/ }));
    const dialog = within(await screen.findByRole("dialog"));
    expect(dialog.getByText(/siguen en tu PC/)).toBeInTheDocument();
    await user.click(dialog.getByRole("button", { name: "Mover" }));

    expect(await sec.findByText(/1 mod en cuarentena/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "quarantine_mods", id, files: ["zoomify.jar"] });
    const q = within(sec.getByRole("region", { name: "En cuarentena" }));
    expect(q.getByText("zoomify.jar")).toBeInTheDocument();

    await user.click(q.getByRole("button", { name: "Devolver zoomify.jar" }));
    expect(await sec.findByText(/zoomify.jar volvió a la carpeta mods/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "restore_mods", id, files: ["zoomify.jar"] });
  });

  it("avisa si eliges algo dudoso", async () => {
    const { user, panel } = await open();
    const sec = await enable(user, panel);
    await user.click(await sec.findByRole("checkbox", { name: "Mover Lithium a la cuarentena" }));
    await user.click(sec.getByRole("button", { name: /Mover a la cuarentena/ }));
    const dialog = within(await screen.findByRole("dialog"));
    expect(dialog.getByText(/Ojo: Lithium/)).toBeInTheDocument();
  });

  it("con el separador activado, el listado de instalados avisa de los mods de solo cliente", async () => {
    localStorage.setItem("merchosting.modSides", JSON.stringify({ [id]: true }));
    const { user, panel } = await open((f) => {
      f.installedMods[0].server_side = "unsupported";
    });
    const note = within(await panel.findByRole("note"));
    expect(note.getByText(/Hay 1 mod que solo sirve/)).toBeInTheDocument();
    await user.click(note.getByRole("button", { name: /Revisarlos/ }));
    expect(await panel.findByRole("region", { name: "Separador de mods" })).toBeInTheDocument();
  });
});
