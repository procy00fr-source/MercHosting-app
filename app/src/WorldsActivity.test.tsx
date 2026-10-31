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
  return { fake, user, w: within(panel) };
}

const online = (f: FakeApi) => {
  f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
};
const id = "11111111-1111-4111-8111-111111111111";

describe("Mundos", () => {
  it("lista los mundos, cambia el activo, duplica, exporta e importa", async () => {
    const { fake, user, w } = await openTab("Mundos");
    expect(await w.findByText("world")).toBeInTheDocument();
    expect(w.getByText(/con world_nether y world_the_end/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: "Mover world a la papelera" })).toBeDisabled();

    await user.click(w.getByRole("button", { name: "Usar este mundo" }));
    expect(await w.findByText(/Ahora el servidor cargará «creativo»/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "set_active_world", id, name: "creativo" });

    await user.click(w.getByRole("button", { name: "Duplicar world" }));
    const dlg = screen.getByRole("dialog", { name: /Duplicar «world»/ });
    expect(within(dlg).getByLabelText("Nombre de la carpeta del mundo")).toHaveValue("world-copia");
    await user.click(within(dlg).getByRole("button", { name: "Duplicar" }));
    expect(await w.findByText("world-copia")).toBeInTheDocument();

    await user.click(w.getByRole("button", { name: "Exportar world" }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({ op: "export_world", id, name: "world", dest: "C:\\Users\\ana\\Desktop\\copia.txt" }),
    );

    await user.click(w.getByRole("button", { name: /Importar .zip/ }));
    const imp = screen.getByRole("dialog", { name: "Importar un mundo" });
    expect(within(imp).getByLabelText("Nombre de la carpeta del mundo")).toHaveValue("mapa");
    await user.click(within(imp).getByRole("button", { name: "Importar" }));
    expect(await w.findByText("mapa")).toBeInTheDocument();
  });

  it("encendido: no deja tocar mundos, pero sí leer y cambiar reglas", async () => {
    const { fake, user, w } = await openTab("Mundos", online);
    expect(await w.findByText(/detén el servidor/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /Importar .zip/ })).toBeDisabled();
    const keep = await w.findByRole("checkbox", { name: /Conservar el inventario/ });
    expect(keep).not.toBeChecked();
    await user.click(keep);
    expect(fake.calls).toContainEqual({ op: "set_game_rule", id, key: "keep_inventory", value: "true" });
    expect(w.queryByText(/Aparecen phantoms/)).not.toBeInTheDocument();
    expect(w.getByText(/1 regla no respondió/)).toBeInTheDocument();
    expect(w.getByRole("heading", { name: "Jugadores" })).toBeInTheDocument();
    const tick = w.getByLabelText(/Velocidad de crecimiento/);
    await user.clear(tick);
    await user.type(tick, "10");
    await user.tab();
    expect(fake.calls).toContainEqual({ op: "set_game_rule", id, key: "random_tick_speed", value: "10" });
  });

  it("muestra el progreso de una operación de mundos y permite cancelarla", async () => {
    const { fake, user, w } = await openTab("Mundos", (f) => {
      f.admin.world_op = { kind: "export", world: "world", done_bytes: 50 * 1024 * 1024, total_bytes: 100 * 1024 * 1024 };
    });
    expect(await w.findByText(/Exportando «world»/)).toBeInTheDocument();
    expect(w.getByRole("progressbar", { name: "Progreso" })).toHaveAttribute("aria-valuenow", "50");
    expect(w.getByRole("button", { name: /Importar .zip/ })).toBeDisabled();
    await user.click(w.getByRole("button", { name: "Cancelar" }));
    expect(fake.calls).toContainEqual({ op: "cancel_world_op", id });
    await waitFor(() => expect(w.queryByText(/Exportando/)).not.toBeInTheDocument());
  });

  it("apagado, las reglas explican que hace falta encenderlo", async () => {
    const { w } = await openTab("Mundos");
    expect(await w.findByText(/Enciende el servidor para ver y cambiar las reglas/)).toBeInTheDocument();
  });
});

describe("Actividad y papelera", () => {
  it("muestra la actividad con textos claros", async () => {
    const { w } = await openTab("Actividad");
    expect(await w.findByText("Encendido")).toBeInTheDocument();
    expect(w.getByText("Ajustes cambiados")).toBeInTheDocument();
    expect(w.getByText("max-players=12")).toBeInTheDocument();
    expect(w.getAllByText("Tú, desde la app").length).toBe(2);
  });

  it("restaura desde la papelera y borra definitivamente con confirmación", async () => {
    const { fake, user, w } = await openTab("Archivos");
    await user.click(await w.findByRole("button", { name: /Papelera y copias/ }));
    const dlg = screen.getByRole("dialog", { name: "Papelera y copias" });
    await user.click(await within(dlg).findByRole("button", { name: "Restaurar mods/roto.jar (papelera)" }));
    await waitFor(() => expect(within(dlg).queryByText("mods/roto.jar")).not.toBeInTheDocument());
    expect(fake.calls).toContainEqual({ op: "restore_saved", id, item_id: "papelera/20261030-041500-123" });

    await user.click(within(dlg).getByRole("button", { name: "Borrar definitivamente server.properties" }));
    const confirm = screen.getByRole("dialog", { name: "Borrar definitivamente" });
    expect(confirm).toHaveTextContent("no se podrá recuperar");
    await user.click(within(confirm).getByRole("button", { name: "Borrar para siempre" }));
    await waitFor(() => expect(fake.saved).toHaveLength(0));
  });
});

describe("Reiniciar", () => {
  it("ofrece reiniciar ya o avisar a los jugadores antes", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary());
    online(fake);
    fake.players = ["Alex"];
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("button", { name: "Reiniciar" }));
    const dlg = screen.getByRole("dialog", { name: "Reiniciar el servidor" });
    await waitFor(() => expect(dlg).toHaveTextContent("Hay 1 jugador conectado"));
    await user.selectOptions(within(dlg).getByLabelText("Tiempo de aviso"), "120");
    await user.click(within(dlg).getByRole("button", { name: /Avisar y reiniciar/ }));
    await waitFor(() => expect(fake.calls).toContainEqual({ op: "restart_with_notice", id, seconds: 120 }));
  });

  it("un reinicio avisado se puede cancelar", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary());
    online(fake);
    fake.admin.restart_in_seconds = 90;
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    expect(await screen.findByText(/se reiniciará en 1 min/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancelar reinicio" }));
    expect(fake.calls).toContainEqual({ op: "cancel_restart", id });
    await waitFor(() => expect(screen.queryByText(/se reiniciará en/)).not.toBeInTheDocument());
  });
});

describe("Mantenimiento y carpeta", () => {
  it("activa y termina el modo mantenimiento", async () => {
    const { fake, user } = await openTab("Jugadores", online);
    const card = await screen.findByRole("region", { name: "Modo mantenimiento" });
    await user.click(within(card).getByRole("button", { name: "Activar mantenimiento" }));
    expect(fake.calls).toContainEqual({ op: "set_maintenance", id, on: true });
    expect(await screen.findByText(/Modo mantenimiento: solo pueden entrar/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Terminar mantenimiento" }));
    expect(fake.calls).toContainEqual({ op: "set_maintenance", id, on: false });
    await waitFor(() => expect(screen.queryByText(/Modo mantenimiento: solo pueden entrar/)).not.toBeInTheDocument());
  });

  it("apagado, el mantenimiento pide encender el servidor", async () => {
    await openTab("Jugadores");
    const card = await screen.findByRole("region", { name: "Modo mantenimiento" });
    expect(within(card).getByRole("button", { name: "Activar mantenimiento" })).toBeDisabled();
    expect(card).toHaveTextContent("Enciende el servidor");
  });

  it("abre la carpeta que estás viendo en el Explorador", async () => {
    const { fake, user, w } = await openTab("Archivos");
    await user.click(await w.findByRole("button", { name: /Ver carpeta del servidor/ }));
    expect(fake.openedFolders).toEqual([{ id, rel: "" }]);
  });
});
