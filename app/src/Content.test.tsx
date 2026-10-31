import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import type { InstanceSummary } from "./api/types";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(kind: InstanceSummary["instance"]["kind"] = "neo_forge", setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind }));
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  return { fake, user };
}

describe("Mods y plugins", () => {
  it("la pestaña depende del tipo de servidor", async () => {
    await open("paper");
    expect(await screen.findByRole("tab", { name: "Plugins" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Mods" })).not.toBeInTheDocument();
  });

  it("vanilla no tiene mods ni plugins, pero sí datapacks del mundo activo", async () => {
    const { fake, user } = await open("vanilla", (f) => {
      f.installedMods = [];
    });
    await user.click(await screen.findByRole("tab", { name: "Datapacks" }));
    expect(screen.queryByRole("tab", { name: "Mods" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Plugins" })).not.toBeInTheDocument();
    const panel = screen.getByRole("tabpanel", { name: "Datapacks" });
    expect(await within(panel).findByText("Todavía no hay datapacks")).toBeInTheDocument();
    expect(panel).toHaveTextContent("Del mundo activo");
    expect(panel).toHaveTextContent("arrastrar tus .zip");
    expect(panel).toHaveTextContent("Haz una copia del mundo");
    await user.click(within(panel).getByRole("button", { name: /Buscar en Modrinth/ }));
    expect(fake.calls).toContainEqual({ op: "content_search", id, kind: "datapack", source: "modrinth", query: "", sort: "downloads", offset: 0 });
  });

  it("Paper busca plugins también en Hangar", async () => {
    const { fake, user } = await open("paper");
    await user.click(await screen.findByRole("tab", { name: "Plugins" }));
    const panel = screen.getByRole("tabpanel", { name: "Plugins" });
    await user.click(within(panel).getByRole("button", { name: "Buscar en Modrinth y Hangar" }));
    await user.click(within(panel).getByRole("button", { name: "Hangar" }));
    expect(within(panel).getByText(/catálogo oficial de plugins de PaperMC/)).toBeInTheDocument();
    await waitFor(() =>
      expect(fake.calls).toContainEqual({ op: "content_search", id, kind: "plugin", source: "hangar", query: "", sort: "downloads", offset: 0 }),
    );
    await user.click(await within(panel).findByRole("button", { name: "Ver Mapa Mundial" }));
    await screen.findByRole("dialog", { name: "Mapa Mundial" });
    await waitFor(() => expect(fake.calls.some((c) => c.op === "content_project" && c.source === "hangar")).toBe(true));
  });

  it("lista lo instalado, desactiva, quita con confirmación y ofrece actualizar", async () => {
    const { fake, user } = await open();
    await user.click(await screen.findByRole("tab", { name: "Mods" }));
    const panel = screen.getByRole("tabpanel", { name: "Mods" });
    const w = within(panel);
    expect(await w.findByText("Just Enough Items")).toBeInTheDocument();
    expect(w.getByText("Desactivado")).toBeInTheDocument();
    expect(w.getByText(/no está en Modrinth/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /Actualizar a 20.0/ })).toBeInTheDocument();

    await user.click(w.getByRole("checkbox", { name: "Desactivar Just Enough Items" }));
    expect(fake.calls).toContainEqual({ op: "content_toggle", id, kind: "mod", file_name: "jei-19.0.jar", enabled: false });
    expect(await w.findByText(/Desactivado: no se cargará/)).toBeInTheDocument();

    await user.click(w.getByRole("button", { name: "Quitar viejo.jar.disabled" }));
    const dlg = screen.getByRole("dialog", { name: /Quitar «viejo.jar.disabled»/ });
    expect(dlg).toHaveTextContent("Papelera y copias");
    await user.click(within(dlg).getByRole("button", { name: "Mover a la papelera" }));
    expect(fake.calls).toContainEqual({ op: "content_remove", id, kind: "mod", file_name: "viejo.jar.disabled" });
  });

  it("busca, muestra el plan con dependencias e instala", async () => {
    const { fake, user } = await open();
    await user.click(await screen.findByRole("tab", { name: "Mods" }));
    const panel = screen.getByRole("tabpanel", { name: "Mods" });
    await user.click(within(panel).getByRole("button", { name: /Buscar en Modrinth/ }));
    expect(fake.calls).toContainEqual({ op: "content_search", id, kind: "mod", source: "modrinth", query: "", sort: "downloads", offset: 0 });
    await user.type(within(panel).getByRole("searchbox", { name: "Buscar mods" }), "mapa");
    await user.click(within(panel).getByRole("button", { name: "Buscar" }));
    expect(fake.calls).toContainEqual({ op: "content_search", id, kind: "mod", source: "modrinth", query: "mapa", sort: "relevance", offset: 0 });

    await user.click(await within(panel).findByRole("button", { name: "Ver Mapa Mundial" }));
    const dlg = await screen.findByRole("dialog", { name: "Mapa Mundial" });
    const plan = await within(dlg).findByRole("region", { name: "Qué se va a instalar" });
    expect(plan).toHaveTextContent("Librería Base");
    expect(plan).toHaveTextContent("Lo necesita «Mapa Mundial»");
    expect(plan).toHaveTextContent("Extras del mapa");
    expect(fake.calls).toContainEqual({ op: "content_plan", id, kind: "mod", source: "modrinth", version_id: "map2" });

    // Cambiar a una beta avisa.
    await user.selectOptions(within(dlg).getByLabelText("Versión"), "jei20");
    expect(within(dlg).getByText(/versión de prueba/)).toBeInTheDocument();
    await user.selectOptions(within(dlg).getByLabelText("Versión"), "map2");

    await user.click(await within(dlg).findByRole("button", { name: "Instalar (2 archivos)" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(fake.calls).toContainEqual({ op: "content_install", id, kind: "mod", source: "modrinth", version_id: "map2" });
    expect(await within(panel).findByText(/Instalado: Mapa Mundial 2.0 y 1 dependencia/)).toBeInTheDocument();
    expect(within(panel).getByText("Librería Base")).toBeInTheDocument();
  });

  it("con clave de CurseForge se puede buscar e instalar desde ahí", async () => {
    const { fake, user } = await open("neo_forge", (f) => {
      f.curseforge = true;
    });
    await user.click(await screen.findByRole("tab", { name: "Mods" }));
    const panel = screen.getByRole("tabpanel", { name: "Mods" });
    await user.click(within(panel).getByRole("button", { name: /Buscar en Modrinth y CurseForge/ }));
    await user.click(within(panel).getByRole("button", { name: /^CurseForge$/ }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({ op: "content_search", id, kind: "mod", source: "curseforge", query: "", sort: "downloads", offset: 0 }),
    );
    expect(within(panel).getByText(/no permiten descargar sus mods desde otras apps/)).toBeInTheDocument();
    await user.click(await within(panel).findByRole("button", { name: "Ver Mapa Mundial" }));
    const dlg = await screen.findByRole("dialog", { name: "Mapa Mundial" });
    expect(await within(dlg).findByRole("button", { name: /Ver en CurseForge/ })).toBeInTheDocument();
    await user.click(await within(dlg).findByRole("button", { name: "Instalar (2 archivos)" }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({ op: "content_install", id, kind: "mod", source: "curseforge", version_id: "map2" }),
    );
  });

  it("sin clave, CurseForge aparece como «Próximamente» y no se puede usar", async () => {
    const { fake, user } = await open();
    await user.click(await screen.findByRole("tab", { name: "Mods" }));
    const panel = screen.getByRole("tabpanel", { name: "Mods" });
    await user.click(within(panel).getByRole("button", { name: "Buscar en Modrinth" }));
    const cf = within(panel).getByRole("button", { name: /CurseForge.*Próximamente/ });
    expect(cf).toBeDisabled();
    expect(within(panel).getByText(/CurseForge llegará próximamente/)).toBeInTheDocument();
    expect(fake.calls.some((c) => c.op === "content_search" && c.source === "curseforge")).toBe(false);
  });

  it("los plugins no muestran CurseForge", async () => {
    const { user } = await open("paper");
    await user.click(await screen.findByRole("tab", { name: "Plugins" }));
    const panel = screen.getByRole("tabpanel", { name: "Plugins" });
    await user.click(within(panel).getByRole("button", { name: /Buscar en Modrinth/ }));
    expect(within(panel).queryByRole("button", { name: /CurseForge/ })).not.toBeInTheDocument();
  });

  it("los bloqueos impiden instalar", async () => {
    const { user } = await open("neo_forge", (f) => {
      f.contentPlanResult = { ...f.contentPlanResult, blockers: ["«Mapa Mundial» es solo para el juego de cada jugador (cliente)."] };
    });
    await user.click(await screen.findByRole("tab", { name: "Mods" }));
    const panel = screen.getByRole("tabpanel", { name: "Mods" });
    await user.click(within(panel).getByRole("button", { name: /Buscar en Modrinth/ }));
    await user.click(await within(panel).findByRole("button", { name: "Ver Mapa Mundial" }));
    const dlg = await screen.findByRole("dialog", { name: "Mapa Mundial" });
    expect(await within(dlg).findByText(/solo para el juego de cada jugador/)).toBeInTheDocument();
    expect(within(dlg).getByRole("button", { name: /Instalar/ })).toBeDisabled();
  });

  it("encendido no deja cambiar nada y sin conexión lo dice", async () => {
    const { user } = await open("neo_forge", (f) => {
      f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 3 };
      f.contentOffline = true;
    });
    await user.click(await screen.findByRole("tab", { name: "Mods" }));
    const panel = screen.getByRole("tabpanel", { name: "Mods" });
    expect(await within(panel).findByText(/detén el servidor primero/)).toBeInTheDocument();
    expect(await within(panel).findByText(/Sin conexión con Modrinth/)).toBeInTheDocument();
    expect(within(panel).getByRole("checkbox", { name: /Desactivar jei-19.0.jar/ })).toBeDisabled();
  });
});
