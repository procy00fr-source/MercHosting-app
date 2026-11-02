import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { mergeLines } from "./components/Console";
import { MercError } from "./api/types";
import { FakeApi, makeSummary } from "./test/fakeApi";

function setup(fake = new FakeApi()) {
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  return { fake, user };
}

describe("MercHosting: primera pantalla", () => {
  it("sin servidores invita a importar uno", async () => {
    setup();
    expect(await screen.findByRole("heading", { name: "Todavía no hay servidores" })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Importar servidor" }).length).toBeGreaterThan(0);
  });

  it("muestra un error claro si no hay supervisor y permite reintentar", async () => {
    const fake = new FakeApi();
    let fail = true;
    fake.info = async () => {
      if (fail) throw new MercError("unavailable", "No se encontró el supervisor de MercHosting");
      return { server_version: "0.1.0", supervisor_pid: 1, data_dir: "x" };
    };
    const { user } = setup(fake);
    expect(await screen.findByText("No se pudo conectar con el supervisor")).toBeInTheDocument();
    expect(screen.getByText("No se encontró el supervisor de MercHosting")).toBeInTheDocument();
    fail = false;
    await user.click(screen.getByRole("button", { name: "Reintentar" }));
    expect(await screen.findByRole("heading", { name: "Todavía no hay servidores" })).toBeInTheDocument();
  });

  it("importa: detecta, recomienda Java 21 (no 25) y registra el servidor", async () => {
    const { fake, user } = setup();
    await user.click((await screen.findAllByRole("button", { name: "Importar servidor" }))[0]);
    const dialog = screen.getByRole("dialog", { name: "Importar un servidor" });
    const importBtn = within(dialog).getByRole("button", { name: "Importar servidor" });
    expect(importBtn).toBeDisabled();

    await user.click(within(dialog).getByRole("button", { name: "Elegir carpeta" }));
    expect(await within(dialog).findByText("NeoForge 1.21.1")).toBeInTheDocument();
    expect(within(dialog).getByText("Comprobado con los archivos del instalador")).toBeInTheDocument();
    const java = within(dialog).getByLabelText("Java") as HTMLSelectElement;
    expect(java.value).toBe("C:\\Java21\\bin\\java.exe");
    expect(within(dialog).getByLabelText("Nombre en MercHosting")).toHaveValue("nuevo");

    await user.click(importBtn);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const call = fake.calls.find((c) => c.op === "import_instance");
    expect(call).toMatchObject({ op: "import_instance", name: "nuevo", java: "C:\\Java21\\bin\\java.exe", max_memory_mb: 4096 });
    expect(await screen.findByRole("heading", { level: 1, name: "nuevo" })).toBeInTheDocument();
  });

  it("no deja importar una carpeta sin servidor", async () => {
    const fake = new FakeApi();
    fake.detected = { ...fake.detected, launch: null, kind: "unknown", warnings: ["No hay ningún JAR de servidor en esta carpeta."] };
    const { user } = setup(fake);
    await user.click((await screen.findAllByRole("button", { name: "Importar servidor" }))[0]);
    const dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Elegir carpeta" }));
    expect(await within(dialog).findByText("No se encontró un servidor en esta carpeta")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Importar servidor" })).toBeDisabled();
  });

  it("avisa cuando no hay un Java compatible", async () => {
    const fake = new FakeApi();
    fake.java = { candidates: [{ path: "C:\\J8\\java.exe", major: 8, description: "", error: null, jdk: false }], recommended: null, recommended_major: 21 };
    const { user } = setup(fake);
    await user.click((await screen.findAllByRole("button", { name: "Importar servidor" }))[0]);
    const dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Elegir carpeta" }));
    expect(await within(dialog).findByText(/Este servidor necesita Java 21/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Importar servidor" })).toBeDisabled();
  });

  it("inicia y detiene; los botones dependen del estado real", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary()];
    const { user } = setup(fake);
    await user.click(await screen.findByRole("button", { name: "Iniciar" }));
    expect(await screen.findByRole("button", { name: "Detener" })).toBeInTheDocument();
    expect(document.querySelector(".status-pill")).toHaveTextContent("En línea");
    expect(screen.queryByRole("button", { name: "Iniciar" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reiniciar" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Quitar de la lista" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Detener" }));
    expect(await screen.findByRole("button", { name: "Iniciar" })).toBeInTheDocument();
    expect(fake.calls.map((c) => c.op)).toEqual(expect.arrayContaining(["start", "stop"]));
  });

  it("la EULA solo se acepta tras marcar la casilla; sin ella no se puede iniciar", async () => {
    const fake = new FakeApi();
    const s = makeSummary();
    s.eula_accepted = false;
    fake.servers = [s];
    const { user } = setup(fake);
    expect(await screen.findByRole("button", { name: "Iniciar" })).toBeDisabled();
    const accept = screen.getByRole("button", { name: "Aceptar EULA" });
    expect(accept).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Leer la EULA" }));
    expect(fake.openedUrls).toEqual(["https://aka.ms/MinecraftEULA"]);
    expect(fake.calls.some((c) => c.op === "accept_eula")).toBe(false);
    await user.click(screen.getByLabelText("He leído la EULA de Minecraft y la acepto"));
    await user.click(accept);
    await waitFor(() => expect(screen.getByRole("button", { name: "Iniciar" })).toBeEnabled());
    expect(fake.calls.filter((c) => c.op === "accept_eula")).toEqual([{ op: "accept_eula", id: s.instance.id, accepted: true }]);
  });

  it("forzar cierre exige confirmar que se puede perder lo no guardado", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary({}, { state: "ONLINE", has_console: true, pid: 9 })];
    const { user } = setup(fake);
    await user.click(await screen.findByRole("button", { name: "Forzar cierre" }));
    const dialog = screen.getByRole("dialog", { name: "Forzar el cierre del servidor" });
    const confirm = within(dialog).getByRole("button", { name: "Forzar cierre" });
    expect(confirm).toBeDisabled();
    await user.click(within(dialog).getByRole("checkbox"));
    await user.click(confirm);
    await waitFor(() => expect(fake.calls.some((c) => c.op === "force_kill")).toBe(true));
  });

  it("un servidor huérfano se explica y no ofrece Detener ni consola", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary({}, { state: "UNKNOWN", has_console: false, pid: 5, hint: "x" })];
    setup(fake);
    expect(await screen.findByText(/siguió encendido mientras MercHosting estaba cerrado/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Detener" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Forzar cierre" })).toBeInTheDocument();
    expect(screen.getByText("Sin consola conectada no se pueden contar los jugadores.")).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("tab", { name: "Consola" }));
    expect(screen.getByLabelText("Comando de Minecraft")).toBeDisabled();
  });

  it("la consola envía comandos y muestra la respuesta", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary({}, { state: "ONLINE", has_console: true, pid: 9 })];
    const { user } = setup(fake);
    await user.click(await screen.findByRole("tab", { name: "Consola" }));
    const input = await screen.findByLabelText("Comando de Minecraft");
    await user.type(input, "say hola{Enter}");
    expect(await screen.findByText("> say hola")).toBeInTheDocument();
    expect(input).toHaveValue("");
    await user.type(input, "{ArrowUp}");
    expect(input).toHaveValue("say hola");
  });

  it("los errores del supervisor se muestran con su mensaje", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary()];
    const { user } = setup(fake);
    const start = await screen.findByRole("button", { name: "Iniciar" });
    fake.failOn = { op: "start", error: new MercError("conflict", "El puerto 25565 ya está en uso en esta PC.") };
    await act(async () => {
      await user.click(start);
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("El puerto 25565 ya está en uso en esta PC.");
  });

  it("quitar de la lista pide confirmación y aclara que no borra archivos", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary()];
    const { user } = setup(fake);
    await user.click(await screen.findByRole("button", { name: "Quitar de la lista" }));
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveTextContent("no se borran");
    await user.click(within(dialog).getByRole("button", { name: "Quitar de la lista" }));
    expect(await screen.findByRole("heading", { name: "Todavía no hay servidores" })).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "unregister", id: fake.calls.find((c) => c.op === "unregister")!.id, delete_files: false });
    expect(fake.recycled).toEqual([]);
  });

  it("borrar también la carpeta exige marcar la casilla y la manda a la Papelera", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary()];
    const root = fake.servers[0].instance.root;
    const { user } = setup(fake);
    await user.click(await screen.findByRole("button", { name: "Quitar de la lista" }));
    const dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByRole("checkbox", { name: /También borrar la carpeta/ }));
    expect(dialog).toHaveTextContent("Papelera");
    expect(dialog).toHaveTextContent("expórtalo antes");
    await user.click(within(dialog).getByRole("button", { name: "Quitar y mandar a la Papelera" }));
    expect(await screen.findByRole("heading", { name: "Todavía no hay servidores" })).toBeInTheDocument();
    expect(fake.recycled).toEqual([root]);
  });
});

describe("datos reales en el resumen", () => {
  it("muestra CPU, memoria y jugadores del supervisor, y las direcciones para entrar", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary({}, { state: "ONLINE", has_console: true, pid: 9, port: 25565, online_at_unix_ms: Date.now() })];
    fake.players = ["Alex", "Steve"];
    setup(fake);
    // Aparecen en la cabecera (en vivo) y en las tarjetas del resumen.
    expect(await screen.findAllByText("13 %")).toHaveLength(2);
    expect(screen.getByText("de toda tu PC (12 núcleos)")).toBeInTheDocument();
    expect(screen.getAllByText("2,0 GB")).toHaveLength(2);
    expect(screen.getByText(/con 4 GB asignados de 16 GB de tu PC/)).toBeInTheDocument();
    expect(screen.getByText("Alex")).toBeInTheDocument();
    expect(screen.getByText("Steve")).toBeInTheDocument();
    expect(screen.getAllByText("/ 20")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Copiar Desde esta PC: localhost" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copiar Desde tu red (LAN): 192.168.1.20:25565" })).toBeInTheDocument();
  });

  it("con el servidor detenido no inventa consumo", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary()];
    setup(fake);
    expect(await screen.findAllByText("Inicia el servidor para ver su consumo.")).toHaveLength(2);
    expect(screen.queryByText(/\d %$/)).not.toBeInTheDocument();
  });

  it("sin IP de red local solo ofrece localhost", async () => {
    const fake = new FakeApi();
    fake.lan = null;
    fake.servers = [makeSummary()];
    setup(fake);
    expect(await screen.findByRole("button", { name: /Desde esta PC/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Desde tu red/ })).not.toBeInTheDocument();
  });

  it("la consola no se reinicia con cada refresco de la lista (el parpadeo)", async () => {
    const fake = new FakeApi();
    fake.servers = [makeSummary({}, { state: "ONLINE", has_console: true, pid: 9 })];
    fake.consoles.set(fake.servers[0].instance.id, [{ seq: 1, at_unix_ms: 1, stream: "stdout", text: "linea uno" }]);
    setup(fake);
    await screen.findByText("linea uno");
    // Varios refrescos de la lista (1 s cada uno) no deben volver a pedir la consola desde el principio.
    await new Promise((r) => setTimeout(r, 2300));
    const fromStart = fake.calls.filter((c) => c.op === "console" && c.after === 0).length;
    expect(fromStart).toBe(1);
    expect(screen.getByText("linea uno")).toBeInTheDocument();
  }, 8000);
});

describe("entrada de la app", () => {
  it("muestra la animación de entrada con la marca mientras conecta", async () => {
    render(<App api={new FakeApi()} />);
    const splash = document.querySelector(".splash") as HTMLElement;
    expect(splash).not.toBeNull();
    expect(within(splash).getByRole("status")).toHaveTextContent(/Conectando con el supervisor|Listo/);
    expect(splash.querySelector(".logo-assemble")).not.toBeNull();
    expect(splash).toHaveTextContent("Tu primer servidor no debería necesitar una mensualidad");
  });
});

describe("sonido de entrada", () => {
  it("se puede silenciar y la elección se recuerda", async () => {
    localStorage.clear();
    const { user } = setup();
    const btn = await screen.findByRole("button", { name: /Sonido de entrada: activado/ });
    expect(btn).toHaveAttribute("aria-pressed", "true");
    await user.click(btn);
    expect(screen.getByRole("button", { name: /Sonido de entrada: silenciado/ })).toHaveAttribute("aria-pressed", "false");
    expect(localStorage.getItem("merchosting.introSound")).toBe("off");
  });
});

describe("mergeLines", () => {
  const l = (seq: number) => ({ seq, at_unix_ms: 0, stream: "stdout" as const, text: String(seq) });
  it("no duplica líneas ya vistas", () => {
    expect(mergeLines([l(1), l(2)], [l(2), l(3)]).map((x) => x.seq)).toEqual([1, 2, 3]);
  });
  it("limita la memoria a 3000 líneas", () => {
    const many = Array.from({ length: 3500 }, (_, i) => l(i + 1));
    const r = mergeLines([], many);
    expect(r.length).toBe(3000);
    expect(r[0].seq).toBe(501);
  });
});

describe("navegación: Servidores y barra lateral", () => {
  it("con varios servidores muestra la lista; al abrir uno aparecen sus secciones y se puede volver", async () => {
    const fake = new FakeApi();
    fake.servers = [
      makeSummary({ name: "Survival" }),
      makeSummary({ id: "22222222-2222-4222-8222-222222222222", name: "Minijuegos", kind: "fabric" }),
    ];
    const { user } = setup(fake);
    expect(await screen.findByRole("heading", { level: 1, name: "Servidores" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Consola" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Abrir Minijuegos" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Minijuegos" })).toBeInTheDocument();
    for (const name of ["Resumen", "Consola", "Archivos", "Ajustes", "Actividad", "Jugadores", "Mundos", "Mods"]) {
      expect(screen.getByRole("tab", { name })).toBeInTheDocument();
    }
    await user.click(screen.getByRole("tab", { name: "Archivos" }));
    expect(screen.getByRole("tab", { name: "Archivos" })).toHaveAttribute("aria-selected", "true");
    await user.click(screen.getByRole("button", { name: /Servidores\s*Minijuegos/ }));
    expect(await screen.findByRole("heading", { level: 1, name: "Servidores" })).toBeInTheDocument();
  });

  it("la barra se puede pasar al otro lado y se recuerda", async () => {
    localStorage.clear();
    const fake = new FakeApi();
    fake.servers = [makeSummary()];
    const { user } = setup(fake);
    await screen.findByRole("tab", { name: "Consola" });
    const toggle = screen.getByRole("button", { name: "Mover la barra a la izquierda" });
    const shell = document.querySelector(".shell") as HTMLElement;
    expect(shell).toHaveAttribute("data-side", "right");
    await user.click(toggle);
    await waitFor(() => expect(document.querySelector(".shell")).toHaveAttribute("data-side", "left"));
    expect(localStorage.getItem("merchosting.navSide")).toBe("left");
  });
});

describe("supervisor de una versión anterior", () => {
  it("avisa y reintenta el cambio con «Actualizar ahora»", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ kind: "paper" }));
    let calls = 0;
    fake.info = async () => {
      calls++;
      const i = { server_version: "1.0.0", supervisor_pid: 42, data_dir: "C:\\datos" };
      return calls === 1 ? { ...i, outdated: true, outdated_reason: "hay servidores encendidos" } : i;
    };
    const { user } = setup(fake);
    expect(await screen.findByText(/Termina de actualizar MercHosting/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Actualizar ahora" }));
    await waitFor(() => expect(screen.queryByText(/Termina de actualizar MercHosting/)).not.toBeInTheDocument());
    expect(calls).toBe(2);
  });
});
