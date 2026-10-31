import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { safeFolderName } from "./format";
import { FakeApi } from "./test/fakeApi";

const SLOW = { timeout: 5000 };

async function openWizard(fake = new FakeApi()) {
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click((await screen.findAllByRole("button", { name: "Crear servidor" }))[0]);
  const dialog = screen.getByRole("dialog", { name: "Crear un servidor" });
  return { fake, user, dialog, w: within(dialog) };
}

async function toSettings(fake?: FakeApi, version = "1.21.11") {
  const ctx = await openWizard(fake);
  const { user, w } = ctx;
  await user.click(w.getByRole("button", { name: /^Paper/ }));
  await user.click(w.getByRole("button", { name: /Siguiente/ }));
  await user.click(await w.findByRole("button", { name: new RegExp(`^${version.replace(/\./g, "\\.")}`) }));
  await w.findByLabelText("Java que usará el servidor");
  await user.click(w.getByRole("button", { name: /Siguiente/ }));
  await w.findByLabelText("Nombre del servidor");
  return ctx;
}

describe("Asistente de creación", () => {
  it("crea un Paper paso a paso con la EULA aceptada por el usuario", async () => {
    const { fake, user, w } = await toSettings();
    expect(w.getByLabelText("Nombre del servidor")).toHaveValue("Paper 1.21.11");
    expect(w.getByLabelText("Puerto")).toHaveValue("25566");
    await user.clear(w.getByLabelText("Nombre del servidor"));
    await user.type(w.getByLabelText("Nombre del servidor"), "Mi server");
    expect(w.getByText("C:\\Users\\ana\\MercHosting\\Servidores\\Mi server")).toBeInTheDocument();
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));

    // EULA: se puede leer y solo se acepta marcando la casilla.
    await user.click(w.getByRole("button", { name: /Leer la EULA/ }));
    expect(fake.openedUrls).toEqual(["https://aka.ms/MinecraftEULA"]);
    await user.click(w.getByRole("checkbox", { name: /He leído y acepto la EULA/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));

    expect(await w.findByText(/Huella\s+SHA-256 publicada por la fuente/)).toBeInTheDocument();
    expect(w.getByText("Aceptada por ti")).toBeInTheDocument();
    expect(w.getByText(/compilación 42/)).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Crear servidor" }));

    expect(await w.findByRole("progressbar", { name: "Descarga" }, SLOW)).toHaveAttribute("aria-valuenow", "50");
    expect(await w.findByText(/¡Listo!/, {}, SLOW)).toBeInTheDocument();
    expect(w.getByText("✔ Servidor creado")).toBeInTheDocument();
    expect(fake.created).toMatchObject({
      name: "Mi server",
      flavor: "paper",
      mc_version: "1.21.11",
      path: "C:\\Users\\ana\\MercHosting\\Servidores\\Mi server",
      java: "C:\\Java21\\bin\\java.exe",
      max_memory_mb: 4096,
      accept_eula: true,
      properties: {
        "server-port": "25566",
        "max-players": "10",
        gamemode: "survival",
        difficulty: "normal",
        "online-mode": "true",
        "white-list": "false",
      },
    });

    await user.click(w.getByRole("button", { name: /Encender ahora/ }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByRole("heading", { level: 1, name: "Mi server" })).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "start", id: "id-creado" });
  });

  it("elige el Java que pide cada versión y avisa si falta", async () => {
    const { fake, user, w } = await openWizard();
    await user.click(w.getByRole("button", { name: /^Paper/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: /^26\.1\.2/ }));
    expect(((await w.findByLabelText("Java que usará el servidor")) as HTMLSelectElement).value).toBe(
      "C:\\Java25\\bin\\java.exe",
    );

    fake.java = { candidates: [], recommended: null, recommended_major: 17 };
    await user.click(w.getByRole("button", { name: /^1\.20\.4/ }));
    expect(await w.findByText(/necesita Java 17 y no lo encontré/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /Siguiente/ })).toBeDisabled();
  });

  it("sin Internet lo dice y permite reintentar", async () => {
    const fake = new FakeApi();
    fake.catalogFails = 1;
    const { user, w } = await openWizard(fake);
    await user.click(w.getByRole("button", { name: /^Vanilla/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(await w.findByText(/No se pudo consultar la lista oficial/)).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Reintentar" }));
    expect(await w.findByRole("button", { name: /^26\.1\.2/ })).toBeInTheDocument();
    expect(w.getByText("Última")).toBeInTheDocument();
  });

  it("no deja usar una carpeta con archivos y advierte sobre online-mode", async () => {
    const fake = new FakeApi();
    fake.busyFolders.add("C:\\Users\\ana\\MercHosting\\Servidores\\Paper 1.21.11");
    const { user, w } = await toSettings(fake);
    expect(await w.findByText(/la carpeta no está vacía/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /Siguiente/ })).toBeDisabled();
    await user.type(w.getByLabelText("Nombre del servidor"), " B");
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());

    await user.click(w.getByRole("checkbox", { name: /online-mode/ }));
    expect(w.getByText(/cualquiera puede entrar usando cualquier nombre/)).toBeInTheDocument();
    await user.click(w.getByRole("checkbox", { name: /Lista blanca/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    // Sin marcar la EULA.
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(await w.findByText(/Pendiente/)).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Crear servidor" }));
    await w.findByText(/¡Listo!/, {}, SLOW);
    expect(fake.created?.accept_eula).toBe(false);
    expect(fake.created?.properties).toMatchObject({ "online-mode": "false", "white-list": "true", "enforce-whitelist": "true" });
    expect(w.getByText(/Acepta la EULA en su página/)).toBeInTheDocument();
  });

  it("muestra el error real si la creación falla", async () => {
    const fake = new FakeApi();
    fake.jobError = "descarga dañada: la huella del archivo no coincide con la publicada";
    const { user, w } = await toSettings(fake);
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: "Crear servidor" }));
    expect(await w.findByText(/No se pudo crear el servidor/, {}, SLOW)).toBeInTheDocument();
    expect(w.getByRole("alert")).toHaveTextContent("la huella del archivo no coincide");
    expect(w.getByRole("button", { name: /Volver al resumen/ })).toBeInTheDocument();
  });

  it("se puede cancelar mientras crea", async () => {
    const fake = new FakeApi();
    fake.holdJob = true;
    const { user, w } = await toSettings(fake);
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: "Crear servidor" }));
    await user.click(await w.findByRole("button", { name: "Cancelar creación" }));
    expect(await w.findByText(/Creación cancelada/, {}, SLOW)).toBeInTheDocument();
    expect(fake.calls.some((c) => c.op === "cancel_job")).toBe(true);
  });
});

describe("tipos de servidor", () => {
  it("ofrece los nueve tipos agrupados por para qué sirven", async () => {
    const { w } = await openWizard();
    for (const name of ["Vanilla", "Paper", "Purpur", "Spigot", "Fabric", "Quilt", "Forge", "NeoForge", "Arclight"])
      expect(w.getByRole("button", { name: new RegExp(`^${name}`) })).toBeInTheDocument();
    expect(w.getByRole("group", { name: "Plugins (Bukkit / Spigot)" })).toHaveTextContent("Spigot");
    expect(w.getByRole("group", { name: "Mods + plugins" })).toHaveTextContent("Arclight");
  });

  it("Spigot solo ofrece Java con compilador (JDK) y avisa si no hay", async () => {
    const { fake, user, w } = await openWizard();
    await user.click(w.getByRole("button", { name: /^Spigot/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: /^1\.21\.11/ }));
    const sel = (await w.findByLabelText("Java que usará el servidor")) as HTMLSelectElement;
    expect([...sel.options].map((o) => o.value)).toEqual(["C:\\Java25\\bin\\java.exe"]);

    fake.java = { ...fake.java, candidates: fake.java.candidates.map((c) => ({ ...c, jdk: false })) };
    await user.click(w.getByRole("button", { name: /^26\.1\.2/ }));
    expect(await w.findByText(/necesita un JDK 25/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /Siguiente/ })).toBeDisabled();
  });
});

describe("nombres de carpeta", () => {
  it("limpia caracteres que Windows no permite", () => {
    expect(safeFolderName('Mi: server / "pro"?')).toBe("Mi- server - -pro--");
    expect(safeFolderName("con")).toBe("con-servidor");
    expect(safeFolderName("  ...  ")).toBe("Servidor");
    expect(safeFolderName("Hola.")).toBe("Hola");
  });
});

describe("Asistente con modpack", () => {
  it("elige un modpack de Modrinth y crea el servidor con su loader exacto", async () => {
    const { fake, user, w } = await openWizard();
    await user.click(w.getByRole("button", { name: /Modpack de Modrinth/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: /Aventura Épica/ }));
    expect(await w.findByText(/Se creará un servidor/)).toHaveTextContent("NeoForge");
    expect(w.getByText(/loader 21.1.77/)).toBeInTheDocument();
    expect(w.getByText(/14 archivos solo para el juego/)).toBeInTheDocument();
    await w.findByLabelText("Java que usará el servidor");
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(await w.findByLabelText("Nombre del servidor")).toHaveValue("Aventura Épica");
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(await w.findByText(/Aventura Épica 2.0 · 210 archivos/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "resolve", flavor: "neo_forge", mc_version: "1.21.1", loader_version: "21.1.77" });
    await user.click(w.getByRole("button", { name: "Crear servidor" }));
    await waitFor(() =>
      expect(fake.created).toMatchObject({ flavor: "neo_forge", mc_version: "1.21.1", modpack_version: "packv2", modpack_source: "modrinth" }),
    );
  });

  it("CurseForge aparece como «Próximamente» sin clave", async () => {
    const { w } = await openWizard();
    const cf = await w.findByRole("button", { name: /Modpack de CurseForge/ });
    expect(cf).toBeDisabled();
    expect(cf).toHaveTextContent("Próximamente");
    expect(cf).toHaveTextContent("clave oficial de CurseForge");
  });

  it("con clave, el modpack de CurseForge usa ese catálogo de principio a fin", async () => {
    const fake = new FakeApi();
    fake.curseforge = true;
    const { user, w } = await openWizard(fake);
    const cf = await w.findByRole("button", { name: /Modpack de CurseForge/ });
    await waitFor(() => expect(cf).toBeEnabled());
    await user.click(cf);
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(w.getByPlaceholderText("Buscar modpacks en CurseForge…")).toBeInTheDocument();
    await user.click(await w.findByRole("button", { name: /Aventura Épica/ }));
    await w.findByText(/Se creará un servidor/);
    expect(fake.calls).toContainEqual({ op: "modpack_inspect", source: "curseforge", version_id: "packv2" });
    await w.findByLabelText("Java que usará el servidor");
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await w.findByLabelText("Nombre del servidor");
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: "Crear servidor" }));
    await waitFor(() => expect(fake.created).toMatchObject({ modpack_version: "packv2", modpack_source: "curseforge" }));
  });

  it("las plantillas fijan valores reales, visibles y que se pueden cambiar", async () => {
    const { fake, user, w } = await toSettings();
    await user.click(w.getByRole("radio", { name: /Con mods/ }));
    expect(w.getByText(/Además fija: PvP activado, se permite volar, sin protección de spawn, distancia de visión 8, distancia de simulación 6/)).toBeInTheDocument();
    expect(w.getByLabelText("Jugadores máximos")).toHaveValue("8");
    await user.click(w.getByRole("radio", { name: /Minijuegos/ }));
    expect(w.getByLabelText("Modo de juego")).toHaveValue("adventure");
    expect(w.getByLabelText("Dificultad")).toHaveValue("peaceful");
    // Se puede cambiar a mano lo que puso la plantilla.
    await user.selectOptions(w.getByLabelText("Dificultad"), "easy");
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(w.getByRole("checkbox", { name: /He leído y acepto la EULA/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: "Crear servidor" }));
    await waitFor(() =>
      expect(fake.created?.properties).toMatchObject({
        gamemode: "adventure",
        difficulty: "easy",
        "max-players": "20",
        pvp: "true",
        "allow-flight": "true",
        "spawn-protection": "0",
        "view-distance": "8",
        "simulation-distance": "6",
      }),
    );
  });
});
