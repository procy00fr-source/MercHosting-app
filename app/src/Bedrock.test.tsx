import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { importSummary, isAddonFile } from "./components/AddonsPanel";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

function bedrockServer(f: FakeApi, online = false) {
  f.servers.push(
    makeSummary({
      kind: "bedrock",
      mc_version: "1.21.124.2",
      loader_version: null,
      launch: { java: "", min_memory_mb: 0, max_memory_mb: 0, extra_jvm_args: [], target: { type: "native", exe: "bedrock_server.exe" } },
    }),
  );
  if (online) f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
}

describe("Crear un servidor Bedrock", () => {
  it("sin Java ni memoria, con claves de Bedrock y la EULA + privacidad marcadas por el usuario", async () => {
    const fake = new FakeApi();
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click((await screen.findAllByRole("button", { name: "Crear servidor" }))[0]);
    const w = within(screen.getByRole("dialog", { name: "Crear un servidor" }));
    await user.click(w.getByRole("button", { name: /^Bedrock/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: /^1\.21\.124\.2/ }));
    expect(w.getByText(/no necesita Java/)).toBeInTheDocument();
    expect(w.queryByLabelText("Java que usará el servidor")).not.toBeInTheDocument();
    await user.click(w.getByRole("button", { name: /Siguiente/ }));

    expect(await w.findByLabelText("Nombre del servidor")).toHaveValue("Bedrock 1.21.124.2");
    expect(w.getByLabelText("Puerto")).toHaveValue("19132");
    expect(w.queryByLabelText("Memoria máxima")).not.toBeInTheDocument();
    expect(w.queryByText(/keepInventory/)).not.toBeInTheDocument();
    await user.type(w.getByLabelText(/Nombre en la lista de servidores/), "Server de Ñandú");
    await user.click(w.getByRole("checkbox", { name: /allow-list/ }));
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));

    expect(w.getByText(/Bedrock no usa el archivo eula.txt/)).toBeInTheDocument();
    await user.click(w.getByRole("checkbox", { name: /acepto la EULA y la Política de privacidad/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(await w.findByText(/No hace falta \(programa nativo de Mojang\)/)).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Crear servidor" }));
    await waitFor(() => expect(fake.created).not.toBeNull());
    expect(fake.created).toMatchObject({
      flavor: "bedrock",
      mc_version: "1.21.124.2",
      java: "",
      max_memory_mb: 0,
      accept_eula: true,
      game_rules: {},
      properties: {
        "server-port": "19132",
        "allow-list": "true",
        "server-name": "Server de Ñandú",
        "online-mode": "true",
        gamemode: "survival",
      },
    });
    expect(fake.created!.properties).not.toHaveProperty("white-list");
    expect(fake.created!.properties).not.toHaveProperty("motd");
  }, 20_000);
});

describe("Servidor Bedrock", () => {
  it("oculta las pestañas de Java, muestra Addons y no habla de Java ni de memoria asignada", async () => {
    const fake = new FakeApi();
    bedrockServer(fake);
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    expect(await screen.findByRole("tab", { name: "Addons" })).toBeInTheDocument();
    for (const t of ["Diagnóstico", "Seguridad", "Actualizaciones", "Invitar jugadores", "MOTD e icono", "Mundos"])
      expect(screen.queryByRole("tab", { name: t })).not.toBeInTheDocument();
    // Jugadores sí, con la pantalla de Bedrock (lista blanca y permisos por XUID).
    expect(screen.getByRole("tab", { name: "Jugadores" })).toBeInTheDocument();
    expect(screen.getByText("No usa Java (Bedrock Dedicated Server)")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Ajustes" }));
    expect(await screen.findByRole("region", { name: "Si se cae y al iniciar sesión" })).toBeInTheDocument();
    expect(screen.queryByText(/Paquete de recursos/)).not.toBeInTheDocument();
  });

  it("Conexión: túnel Minecraft Bedrock y sin dominio SRV ni Geyser", async () => {
    const fake = new FakeApi();
    bedrockServer(fake, true);
    fake.playit = { ...fake.playit, enabled: true, installed: true, running: true };
    fake.approveClaim("147.185.221.20:19132", "127.0.0.1:25565");
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Conexión" }));
    const panel = within(screen.getByRole("tabpanel", { name: "Conexión" }));
    expect(await panel.findByText("147.185.221.20:19132")).toBeInTheDocument();
    expect(panel.queryByRole("button", { name: /Usar mi dominio/ })).not.toBeInTheDocument();
    expect(panel.queryByRole("region", { name: "Jugadores de Bedrock" })).not.toBeInTheDocument();
  });
});

describe("Addons", () => {
  it("reconoce archivos y resume la importación", () => {
    expect(isAddonFile("C:\\x\\Dragones.MCADDON")).toBe(true);
    expect(isAddonFile("pack.mcpack")).toBe(true);
    expect(isAddonFile("mod.jar")).toBe(false);
    expect(importSummary({ installed: ["A 1.0.0"], replaced: ["B 2.0.0"], unchanged: [], notes: [] })).toEqual([
      "Instalado y activado: A 1.0.0.",
      "Actualizado (la versión anterior está en la papelera): B 2.0.0.",
    ]);
  });

  it("instala un .mcaddon, avisa de scripts, desactiva y quita con confirmación", async () => {
    const fake = new FakeApi();
    bedrockServer(fake);
    fake.addonsState.packs = [
      { uuid: "99999999-0000-0000-0000-000000000000", version: [1, 21, 0], name: "Vanilla", description: "", kind: "behavior", folder: "behavior_packs/vanilla_1.21", active: false, bundled: true, scripts: false },
    ];
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Addons" }));
    const panel = within(screen.getByRole("tabpanel", { name: "Addons" }));
    expect(await panel.findByText(/Todavía no hay addons/)).toBeInTheDocument();
    expect(panel.getByText("Incluidos con el servidor (1)")).toBeInTheDocument();
    expect(panel.getByRole("button", { name: /Buscar en CurseForge/ })).toBeDisabled();
    expect(panel.getByText("Próximamente")).toBeInTheDocument();

    await user.click(panel.getByRole("button", { name: /Instalar addon/ }));
    expect(fake.calls).toContainEqual({ op: "addon_import", id, source: "C:\\Users\\ana\\Downloads\\dragones.mcaddon" });
    expect(await panel.findByText(/Instalado y activado: Dragones 1\.0\.0, Dragones \(texturas\) 1\.0\.0/)).toBeInTheDocument();
    expect(panel.getByText(/usa scripts/)).toBeInTheDocument();

    const toggle = panel.getByRole("checkbox", { name: "Activo en el mundo: Dragones" });
    expect(toggle).toBeChecked();
    await user.click(toggle);
    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(fake.calls).toContainEqual({ op: "addon_set_active", id, uuid: "11111111-2222-3333-4444-555555555555", active: false });

    await user.click(panel.getByRole("button", { name: "Quitar Dragones" }));
    const dlg = within(screen.getByRole("dialog", { name: /Quitar Dragones/ }));
    expect(dlg.getByText(/papelera de MercHosting/)).toBeInTheDocument();
    await user.click(dlg.getByRole("button", { name: "Quitar" }));
    await waitFor(() => expect(panel.queryByText("Dragones", { selector: "strong" })).not.toBeInTheDocument());
    expect(fake.calls).toContainEqual({ op: "addon_remove", id, uuid: "11111111-2222-3333-4444-555555555555" });
  }, 20_000);

  it("con la clave de CurseForge: busca addons de Bedrock, instala el elegido y avisa si el autor no deja descargarlo", async () => {
    const fake = new FakeApi();
    bedrockServer(fake);
    fake.addonsState.curseforge = true;
    const hit = (project_id: string, title: string, slug: string) => ({
      project_id,
      slug,
      title,
      author: "autora",
      description: `${title} para tu mundo`,
      icon_url: null,
      downloads: 1234,
      categories: [],
      client_side: "unknown" as const,
      server_side: "unknown" as const,
      date_modified: "",
      license: "",
    });
    fake.cfAddons = [
      {
        hit: hit("77", "Castillos", "castillos"),
        packs: [
          { uuid: "aaaaaaaa-2222-3333-4444-555555555555", version: [2, 0, 0], name: "Castillos", description: "", kind: "resource", folder: "resource_packs/Castillos_aaaaaaaa", active: true, bundled: false, scripts: false },
        ],
      },
      { hit: hit("88", "Cerrado", "cerrado"), packs: null },
    ];
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Addons" }));
    const panel = within(screen.getByRole("tabpanel", { name: "Addons" }));
    const box = within(await panel.findByRole("region", { name: "Buscar addons en CurseForge" }));
    expect(panel.queryByText("Próximamente")).not.toBeInTheDocument();

    await user.type(box.getByLabelText("Nombre del addon"), "cas");
    await user.click(box.getByRole("button", { name: /Buscar/ }));
    expect(fake.calls).toContainEqual({ op: "addon_search", id, query: "cas", sort: "relevance", offset: 0 });
    await user.click(await box.findByRole("button", { name: "Ver Castillos en CurseForge" }));
    expect(fake.openedUrls).toContain("https://www.curseforge.com/minecraft-bedrock/addons/castillos");
    await user.click(box.getByRole("button", { name: "Instalar Castillos" }));
    expect(fake.calls).toContainEqual({ op: "addon_install_curseforge", id, project_id: "77", file_id: null });
    expect(await panel.findByText(/Instalado y activado: Castillos 2\.0\.0/)).toBeInTheDocument();
    expect(panel.getByRole("checkbox", { name: "Activo en el mundo: Castillos" })).toBeChecked();

    await user.clear(box.getByLabelText("Nombre del addon"));
    await user.click(box.getByRole("button", { name: /Ver populares/ }));
    await user.click(await box.findByRole("button", { name: "Instalar Cerrado" }));
    expect(await box.findByText(/no permite descargarlo desde otras aplicaciones/)).toBeInTheDocument();
  }, 20_000);

  it("arrastrar: instala solo .mcpack/.mcaddon", async () => {
    const fake = new FakeApi();
    bedrockServer(fake);
    fake.addonImports["C:\\pack.mcpack"] = [
      { uuid: "22222222-2222-3333-4444-555555555555", version: [2, 0, 0], name: "Mapa", description: "", kind: "resource", folder: "resource_packs/Mapa_22222222", active: true, bundled: false, scripts: false },
    ];
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Addons" }));
    const panel = within(screen.getByRole("tabpanel", { name: "Addons" }));
    await panel.findByText(/Todavía no hay addons/);
    act(() => fake.dropFiles({ type: "drop", paths: ["C:\\pack.mcpack", "C:\\mod.jar"] }));
    expect(await panel.findByText(/Instalado y activado: Mapa 2\.0\.0/)).toBeInTheDocument();
    expect(panel.getByText(/Ignorados \(no son \.mcpack ni \.mcaddon\): mod\.jar/)).toBeInTheDocument();
  });
});
