import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

async function openPlayers(setup: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary());
  fake.servers[0].status = { ...fake.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
  setup(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Jugadores" }));
  const panel = screen.getByRole("tabpanel", { name: "Jugadores" });
  return { fake, user, w: within(panel) };
}

describe("Jugadores: acciones e historial", () => {
  it("manda un mensaje, cambia el modo y teletransporta a un jugador conectado", async () => {
    const { fake, user, w } = await openPlayers((f) => {
      f.players = ["Alex", "Steve"];
    });
    await user.click(await w.findByRole("button", { name: "Más acciones con Alex" }));
    let dlg = within(screen.getByRole("dialog", { name: "Acciones con Alex" }));
    await user.type(dlg.getByLabelText(/Mensaje privado/), "hola");
    await user.click(dlg.getByRole("button", { name: "Enviar mensaje" }));
    expect(await w.findByText("Mensaje enviado a Alex.")).toBeInTheDocument();

    await user.click(w.getByRole("button", { name: "Más acciones con Alex" }));
    dlg = within(screen.getByRole("dialog", { name: "Acciones con Alex" }));
    await user.click(dlg.getAllByRole("button", { name: "Modo de juego" })[0]);
    await user.selectOptions(dlg.getByLabelText("Modo de juego"), "creative");
    expect(dlg.getByText(/cualquier objeto y volar/)).toBeInTheDocument();
    await user.click(dlg.getByRole("button", { name: "Cambiar modo" }));
    await w.findByText(/ahora está en modo creativo/);

    await user.click(w.getByRole("button", { name: "Más acciones con Alex" }));
    dlg = within(screen.getByRole("dialog", { name: "Acciones con Alex" }));
    await user.click(dlg.getAllByRole("button", { name: "Teletransportar" })[0]);
    expect(dlg.getByLabelText("Jugador")).toHaveValue("Steve");
    await user.click(dlg.getByRole("radio", { name: /coordenadas/ }));
    await user.clear(dlg.getByLabelText("X"));
    await user.type(dlg.getByLabelText("X"), "abc");
    const submitTp = () => dlg.getAllByRole("button", { name: "Teletransportar" }).at(-1)!;
    expect(submitTp()).toBeDisabled();
    await user.clear(dlg.getByLabelText("X"));
    await user.type(dlg.getByLabelText("X"), "-120");
    await user.click(submitTp());
    await w.findByText(/fue llevado a -120 64 0/);
    expect(fake.playerCommands).toEqual([
      { name: "Alex", command: { type: "message", text: "hola" } },
      { name: "Alex", command: { type: "game_mode", mode: "creative" } },
      { name: "Alex", command: { type: "teleport_coords", x: -120, y: 64, z: 0 } },
    ]);
  });

  it("muestra el historial, guarda notas y lo borra con confirmación", async () => {
    const now = Math.floor(Date.now() / 1000);
    const { fake, user, w } = await openPlayers((f) => {
      f.players = ["Alex"];
      f.history = [
        { name: "Alex", first_seen_unix: now - 90000, last_seen_unix: now - 600, sessions: 3, total_secs: 5400, online_since_unix: now - 600, note: null },
        { name: "Steve", first_seen_unix: now - 90000, last_seen_unix: now - 7200, sessions: 1, total_secs: 1500, online_since_unix: null, note: "Amigo de Alex" },
      ];
    });
    const card = within(await w.findByRole("region", { name: "Historial de jugadores" }));
    const alex = card.getByRole("row", { name: /^Alex/ });
    expect(within(alex).getByText("Conectado")).toBeInTheDocument();
    expect(within(alex).getByText("1 h 40 min")).toBeInTheDocument();
    const steve = card.getByRole("row", { name: /^Steve/ });
    expect(within(steve).getByText("hace 2 h")).toBeInTheDocument();
    expect(within(steve).getByText("25 min")).toBeInTheDocument();
    expect(within(steve).getByText("Amigo de Alex")).toBeInTheDocument();

    await user.click(card.getByRole("button", { name: "Nota de Alex" }));
    const dlg = within(screen.getByRole("dialog", { name: "Nota sobre Alex" }));
    await user.type(dlg.getByLabelText("Nota"), "Constructor");
    await user.click(dlg.getByRole("button", { name: /Guardar nota/ }));
    await waitFor(() => expect(card.getByText("Constructor")).toBeInTheDocument());

    await user.click(card.getByRole("button", { name: /Borrar historial y notas/ }));
    await user.click(within(screen.getByRole("dialog", { name: "Borrar historial y notas" })).getByRole("button", { name: "Borrar" }));
    expect(await card.findByText("Todavía no entró nadie.")).toBeInTheDocument();
    expect(fake.history).toEqual([]);
  });
});
