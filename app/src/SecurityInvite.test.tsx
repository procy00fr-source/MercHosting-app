import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "neo_forge" }));
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  return { fake, user };
}

describe("Seguridad", () => {
  it("muestra los peligros primero y arregla un ajuste con confirmación", async () => {
    const { fake, user } = await open((f) => {
      f.security = {
        checked_at_unix_ms: 1,
        public_addresses: ["dark-sea.joinmc.link"],
        checks: [
          {
            code: "online_mode",
            level: "peligro",
            title: "Cualquiera puede entrar con el nombre que quiera",
            detail: "online-mode está desactivado.",
            action: { type: "set_property", key: "online-mode", value: "true" },
          },
          { code: "backups", level: "aviso", title: "No hay copias de seguridad", detail: "…", action: { type: "open_tab", tab: "backups" } },
          { code: "companion", level: "info", title: "Shandé puede administrar este servidor", detail: "…", action: { type: "open_companions" } },
        ],
      };
    });
    await user.click(await screen.findByRole("tab", { name: "Seguridad" }));
    const panel = within(await screen.findByRole("tabpanel", { name: "Seguridad" }));
    expect(await panel.findByText("1 en peligro · 1 para revisar.")).toBeInTheDocument();
    await user.click(panel.getByRole("button", { name: "Cambiar online-mode a true" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Aplicar" }));
    await waitFor(() => expect(fake.calls.some((c) => c.op === "set_properties" && c.changes["online-mode"] === "true")).toBe(true));
    // Ir a copias y a compañeros.
    await user.click(panel.getByRole("button", { name: "Abrir copias de seguridad" }));
    expect(await screen.findByRole("tabpanel", { name: /Copias/ })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Seguridad" }));
    await user.click(await screen.findByRole("button", { name: "Revisar compañeros" }));
    expect(await screen.findByRole("region", { name: "Tu PC" })).toBeInTheDocument();
  });
});

describe("Invitar jugadores", () => {
  it("arma el mensaje con dirección probada, loader y mods necesarios con su página oficial", async () => {
    const { fake, user } = await open((f) => {
      f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
      f.playit = { ...f.playit, enabled: true, installed: true, running: true };
      f.approveClaim("dark-sea.joinmc.link");
      f.reachable.add("dark-sea.joinmc.link");
      f.installedMods = [
        { ...f.installedMods[0], title: "Create", version_number: "6.0.4", client_side: "required", page_url: "https://modrinth.com/project/create" },
        { ...f.installedMods[0], file_name: "spark.jar", title: "spark", version_number: "1.10", client_side: "unsupported", page_url: "https://modrinth.com/project/spark" },
        { ...f.installedMods[0], file_name: "raro.jar", title: null, client_side: null, page_url: null, project_id: null },
      ];
      f.props = f.props.map((p) => (p.key === "white-list" ? { ...p, value: "true" } : p));
      if (!f.props.some((p) => p.key === "white-list")) f.props.push({ key: "white-list", value: "true" });
    });
    // Probar la dirección primero (en Conexión).
    await user.click(await screen.findByRole("tab", { name: "Conexión" }));
    await user.click(await screen.findByRole("button", { name: "Probar dark-sea.joinmc.link" }, { timeout: 5000 }));
    await screen.findByText(/Funciona: responde/);
    await user.click(screen.getByRole("tab", { name: "Invitar jugadores" }));
    const panel = within(await screen.findByRole("tabpanel", { name: "Invitar jugadores" }));
    const msg = await panel.findByLabelText("Mensaje de invitación");
    await waitFor(() => expect(msg).toHaveTextContent("Dirección: dark-sea.joinmc.link"));
    await waitFor(() => expect(msg).toHaveTextContent("- Create 6.0.4 — https://modrinth.com/project/create"));
    expect(msg).toHaveTextContent("Minecraft Java 1.21.1 con NeoForge 21.1.259");
    expect(msg).not.toHaveTextContent("spark");
    expect(msg).toHaveTextContent("lista blanca");
    expect(panel.getByText(/Sin datos de si hacen falta en el juego/)).toHaveTextContent("raro.jar");
    // Añadir a la lista blanca desde aquí.
    await user.type(panel.getByRole("textbox", { name: "Nombre de Minecraft" }), "Shande_07");
    await user.click(panel.getByRole("button", { name: "Añadir" }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({ op: "player_action", id, action: "whitelist_add", name: "Shande_07", reason: null }),
    );
    expect(panel.getByRole("button", { name: /Copiar mensaje/ })).toBeEnabled();
  }, 20_000);

  it("una dirección sin probar no se puede copiar", async () => {
    const { user } = await open((f) => {
      f.playit = { ...f.playit, enabled: true, installed: true, running: true };
      f.approveClaim("dark-sea.joinmc.link");
    });
    await user.click(await screen.findByRole("tab", { name: "Invitar jugadores" }));
    const panel = within(await screen.findByRole("tabpanel", { name: "Invitar jugadores" }));
    expect(await panel.findByText(/sin probar: pruébala en Conexión/)).toBeInTheDocument();
    expect(panel.getByRole("button", { name: /Copiar mensaje/ })).toBeDisabled();
  });
});
