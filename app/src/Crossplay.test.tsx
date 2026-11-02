import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { geyserPlatform } from "./components/CrossplayCard";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(kind: "paper" | "forge", setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind }));
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Conexión" }));
  const panel = screen.getByRole("tabpanel", { name: "Conexión" });
  const card = within(await within(panel).findByRole("region", { name: "Jugadores de Bedrock" }));
  return { fake, user, card };
}

const geyser = {
  geyser: "plugins/Geyser-Spigot.jar",
  geyser_enabled: true,
  floodgate: null,
  floodgate_enabled: false,
  config: null,
  port: 19132,
  port_from_config: false,
  lan: ["192.168.1.20:19132"],
  tunnels: [],
  tests: [],
};

describe("Jugadores de Bedrock (Geyser)", () => {
  it("elige la plataforma compatible con Geyser", () => {
    expect(geyserPlatform([{ kind: "plugin", folder: "plugins", loaders: ["paper", "spigot"], experimental: false, sources: ["modrinth"] }])?.kind).toBe("plugin");
    expect(geyserPlatform([{ kind: "mod", folder: "mods", loaders: ["neoforge"], experimental: false, sources: ["modrinth"] }])?.kind).toBe("mod");
    expect(geyserPlatform([{ kind: "mod", folder: "mods", loaders: ["forge"], experimental: false, sources: ["modrinth"] }])).toBeNull();
  });

  it("en Forge dice que no es compatible y no ofrece instalar", async () => {
    const { card } = await open("forge");
    expect(await card.findByText(/no es compatible/)).toBeInTheDocument();
    expect(card.queryByRole("button", { name: "Instalar Geyser" })).not.toBeInTheDocument();
  });

  it("instala Geyser desde Modrinth con el diálogo de siempre", async () => {
    const { fake, user, card } = await open("paper");
    await user.click(await card.findByRole("button", { name: "Instalar Geyser" }));
    const dlg = within(await screen.findByRole("dialog"));
    expect(fake.calls).toContainEqual({ op: "content_project", id, kind: "plugin", source: "modrinth", project: "geyser" });
    await user.click(await dlg.findByRole("button", { name: /^Instalar/ }));
    expect(fake.calls.some((c) => c.op === "content_install" && c.kind === "plugin" && c.source === "modrinth")).toBe(true);
    expect(await card.findByText(/Instalado:/)).toBeInTheDocument();
  });

  it("prueba Bedrock de verdad y solo deja copiar la dirección que respondió", async () => {
    const { fake, user, card } = await open("paper", (f) => {
      f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
      f.bedrock = structuredClone(geyser);
    });
    expect(await card.findByText(/necesitarán una cuenta de Java/)).toBeInTheDocument();
    expect(card.getByText(/el de fábrica/)).toBeInTheDocument();
    // Sin túnel de Bedrock: guía para crearlo en Playit.
    expect(within(card.getByLabelText("Túnel de Bedrock")).getByText("Minecraft Bedrock")).toBeInTheDocument();

    await user.click(card.getByRole("button", { name: "Probar Bedrock 192.168.1.20:19132" }));
    expect(fake.calls).toContainEqual({ op: "test_bedrock_address", id, address: "192.168.1.20:19132" });
    expect(await card.findByText(/no respondió como un servidor Bedrock/)).toBeInTheDocument();

    fake.bedrock!.tunnels = [{ display_address: "147.185.221.20:19132", destination: "127.0.0.1:19132", is_disabled: false, disabled_reason: null }];
    fake.reachableBedrock.add("147.185.221.20:19132");
    const copyBtn = await card.findByRole("button", { name: "Copiar Bedrock 147.185.221.20:19132" }, { timeout: 5000 });
    expect(copyBtn).toBeDisabled();
    await user.click(card.getByRole("button", { name: "Probar Bedrock 147.185.221.20:19132" }));
    expect(await card.findByText(/Funciona: responde en 52 ms · Bedrock 1.21.40/)).toBeInTheDocument();
    expect(card.getByRole("button", { name: "Copiar Bedrock 147.185.221.20:19132" })).toBeEnabled();
  }, 20_000);
});
