import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

async function openTab(tab: string, setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary());
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: tab }));
  const panel = screen.getByRole("tabpanel", { name: tab });
  return { fake, user, panel, w: within(panel) };
}

const online = (f: FakeApi) => {
  f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
};

describe("Jugadores", () => {
  it("con el servidor apagado muestra las listas y explica por qué no se puede cambiar", async () => {
    const { w } = await openTab("Jugadores");
    expect(await w.findByText("Steve")).toBeInTheDocument();
    expect(w.getByText("nivel 4")).toBeInTheDocument();
    expect(w.getByText("Alex")).toBeInTheDocument();
    expect(w.getByText("Romper casas")).toBeInTheDocument();
    expect(w.getByText(/Enciende el servidor para cambiar estas listas/)).toBeInTheDocument();
    await userEvent.type(w.getByLabelText("Nombre del jugador"), "Luna");
    expect(w.getByRole("button", { name: /Añadir a la lista blanca/ })).toBeDisabled();
  });

  it("encendido: añade, banea con confirmación y motivo, y valida el nombre", async () => {
    const { fake, user, w } = await openTab("Jugadores", (f) => {
      online(f);
      f.players = ["Alex"];
    });
    await w.findByText("Conectados");
    await user.type(w.getByLabelText("Nombre del jugador"), "mal nombre!");
    expect(w.getByText(/Solo letras, números y _/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /Añadir a la lista blanca/ })).toBeDisabled();
    await user.clear(w.getByLabelText("Nombre del jugador"));
    await user.type(w.getByLabelText("Nombre del jugador"), "Luna");
    await user.click(w.getByRole("button", { name: /Añadir a la lista blanca/ }));
    expect(fake.calls).toContainEqual({ op: "player_action", id: fake.servers[0].instance.id, action: "whitelist_add", name: "Luna", reason: null });

    // Banear a un conectado pide confirmación y permite motivo.
    const row = (await w.findAllByText("Alex")).map((el) => el.closest("li")!).find((li) => within(li).queryByRole("button", { name: "Banear" }))!;
    await user.click(within(row).getByRole("button", { name: "Banear" }));
    const dialog = screen.getByRole("dialog", { name: /Banear jugador: Alex/ });
    await user.type(within(dialog).getByLabelText(/Motivo/), "Spam en el chat");
    await user.click(within(dialog).getByRole("button", { name: "Banear" }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({ op: "player_action", id: fake.servers[0].instance.id, action: "ban", name: "Alex", reason: "Spam en el chat" }),
    );
    // Dar OP también pide confirmación.
    await user.type(w.getByLabelText("Nombre del jugador"), "Luna");
    await user.click(w.getByRole("button", { name: /Dar OP/ }));
    expect(screen.getByRole("dialog", { name: /Dar permisos de operador: Luna/ })).toHaveTextContent("todos los comandos");
  });
});

describe("Ajustes (server.properties)", () => {
  it("el paquete de recursos se comprueba en el supervisor y guarda su huella", async () => {
    const { fake, user, w } = await openTab("Ajustes");
    const card = within(await w.findByRole("region", { name: "Paquete de recursos" }));
    const input = card.getByLabelText("Enlace del paquete (.zip)");
    await user.type(input, "http://inseguro.dev/p.zip");
    expect(card.getByText("Tiene que empezar por https://")).toBeInTheDocument();
    expect(card.getByRole("button", { name: "Comprobar y guardar" })).toBeDisabled();
    await user.clear(input);
    await user.type(input, "https://ejemplo.dev/texturas.zip");
    await user.click(card.getByRole("checkbox", { name: /Obligatorio/ }));
    await user.click(card.getByRole("button", { name: "Comprobar y guardar" }));
    expect(fake.calls).toContainEqual({ op: "set_resource_pack", id: fake.servers[0].instance.id, url: "https://ejemplo.dev/texturas.zip", required: true });
    expect(await card.findByText(/Paquete comprobado y guardado/)).toBeInTheDocument();
    expect(card.getByText(/SHA-1: 0123456789abcdef/)).toBeInTheDocument();
    // No aparece duplicado en «Avanzado».
    expect(w.queryByLabelText("resource-pack-sha1")).not.toBeInTheDocument();
    await user.click(card.getByRole("button", { name: "Quitar paquete" }));
    expect(await card.findByText(/Paquete quitado/)).toBeInTheDocument();
  });

  it("edita con controles claros, valida y guarda solo lo cambiado", async () => {
    const { fake, user, w } = await openTab("Ajustes");
    expect(await w.findByLabelText("Dificultad")).toHaveValue("easy");
    // El MOTD se edita en su pestaña (colores y dos líneas), no aquí ni en «Avanzado».
    expect(w.queryByLabelText("Mensaje en la lista de servidores")).not.toBeInTheDocument();
    expect(w.queryByLabelText("motd")).not.toBeInTheDocument();
    expect(w.getByRole("button", { name: /Guardar cambios/ })).toBeDisabled();

    const players = w.getByLabelText("Jugadores máximos");
    await user.clear(players);
    await user.type(players, "abc");
    expect(w.getByText("Escribe un número")).toBeInTheDocument();
    expect(w.getByRole("button", { name: /Guardar cambios/ })).toBeDisabled();
    await user.clear(players);
    await user.type(players, "12");
    await user.click(w.getByRole("checkbox", { name: /Combate entre jugadores/ }));
    await user.click(w.getByRole("checkbox", { name: /online-mode/ }));
    expect(w.getByText(/usando el nombre de otro/)).toBeInTheDocument();
    expect(w.getByText("3 cambios sin guardar")).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: /Guardar cambios/ }));
    await w.findByText(/Guardado/);
    const call = fake.calls.find((c) => c.op === "set_properties");
    expect(call).toEqual({
      op: "set_properties",
      id: fake.servers[0].instance.id,
      changes: { "max-players": "12", pvp: "false", "online-mode": "false" },
      expected_modified: 1000,
    });
    // Las opciones desconocidas están en Avanzado.
    await user.click(w.getByRole("button", { name: /Avanzado: 1 opciones más/ }));
    expect(w.getByLabelText("mod-especial")).toHaveValue("x");
  });

  it("si otro programa cambió el archivo, no lo pisa y permite recargar", async () => {
    const { fake, user, w } = await openTab("Ajustes");
    await w.findByLabelText("Dificultad");
    fake.propsModified = 2000;
    await user.selectOptions(w.getByLabelText("Dificultad"), "hard");
    await user.click(w.getByRole("button", { name: /Guardar cambios/ }));
    expect(await w.findByText(/cambió desde que lo abriste/)).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Volver a cargar" }));
    await waitFor(() => expect(w.getByLabelText("Dificultad")).toHaveValue("easy"));
  });
});

describe("Archivos", () => {
  it("navega, abre, edita y guarda un archivo de texto", async () => {
    const { fake, user, w } = await openTab("Archivos");
    await user.click(await w.findByRole("button", { name: "config" }));
    await user.click(await w.findByRole("button", { name: "a.toml" }));
    const editor = await w.findByLabelText("Contenido de config/a.toml");
    expect(editor).toHaveValue("a = 1\n");
    await user.type(editor, "b = 2");
    await user.click(w.getByRole("button", { name: "Guardar" }));
    await w.findByText(/Guardado config\/a.toml/);
    expect(fake.files.get("config/a.toml")).toBe("a = 1\nb = 2");
    await user.click(w.getByRole("button", { name: "Carpeta del servidor" }));
    expect(await w.findByRole("button", { name: "server.properties" })).toBeInTheDocument();
  });

  it("renombra, sube y manda a la papelera con confirmación", async () => {
    const { fake, user, w } = await openTab("Archivos");
    await user.click(await w.findByRole("button", { name: "Renombrar server.properties" }));
    const ren = screen.getByRole("dialog", { name: /Renombrar/ });
    await user.clear(within(ren).getByLabelText("Nombre nuevo"));
    await user.type(within(ren).getByLabelText("Nombre nuevo"), "viejo.properties");
    await user.click(within(ren).getByRole("button", { name: "Renombrar" }));
    expect(await w.findByRole("button", { name: "viejo.properties" })).toBeInTheDocument();

    await user.click(w.getByRole("button", { name: /Subir archivo/ }));
    expect(await w.findByRole("button", { name: "mod.jar" })).toBeInTheDocument();

    await user.click(w.getByRole("button", { name: "Mover world a la papelera" }));
    const dlg = screen.getByRole("dialog", { name: /Mover «world» a la papelera/ });
    expect(dlg).toHaveTextContent("parece un mundo");
    await user.click(within(dlg).getByRole("button", { name: "Mover a la papelera" }));
    await waitFor(() => expect(w.queryByRole("button", { name: "world" })).not.toBeInTheDocument());
    expect(fake.calls.some((c) => c.op === "trash_file")).toBe(true);
  });

  it("con el servidor encendido solo deja ver y descargar", async () => {
    const { fake, user, w } = await openTab("Archivos", online);
    expect(await w.findByText(/para cambiar archivos deténlo antes/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /Subir archivo/ })).toBeDisabled();
    expect(w.getByRole("button", { name: "Mover world a la papelera" })).toBeDisabled();
    await user.click(w.getByRole("button", { name: "Descargar server.properties" }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({
        op: "download_file",
        id: fake.servers[0].instance.id,
        path: "server.properties",
        dest: "C:\\Users\\ana\\Desktop\\copia.txt",
      }),
    );
    await user.click(w.getByRole("button", { name: "server.properties" }));
    expect(await w.findByLabelText("Contenido de server.properties")).toHaveAttribute("readonly");
  });
});

describe("Consola", () => {
  it("exporta el registro a donde elija el usuario", async () => {
    const { fake, user } = await openTab("Consola");
    await user.click(screen.getByRole("button", { name: /Exportar registro/ }));
    await waitFor(() => expect(fake.calls.some((c) => c.op === "export_console")).toBe(true));
  });
});
