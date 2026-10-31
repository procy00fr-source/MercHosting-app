import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { levelOf } from "./components/Console";
import { dangerOf } from "./consoleCommands";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open() {
  const fake = new FakeApi();
  fake.servers = [makeSummary({}, { state: "ONLINE", has_console: true, pid: 9 })];
  fake.consoles.set(id, [
    { seq: 1, at_unix_ms: Date.UTC(2026, 9, 31, 15, 4, 5), stream: "stdout", text: "[15:04:05] [Server thread/INFO]: Done (3.2s)!" },
    { seq: 2, at_unix_ms: 2, stream: "stdout", text: "[15:04:06] [Server thread/WARN]: Can't keep up!" },
    { seq: 3, at_unix_ms: 3, stream: "stdout", text: "[15:04:07 ERROR]: Could not pass event" },
  ]);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Consola" }));
  const con = within(await screen.findByRole("region", { name: "Consola del servidor" }));
  await con.findByText(/Done \(3.2s\)/);
  return { fake, user, con };
}

describe("Consola", () => {
  beforeEach(() => localStorage.clear());

  it("reconoce niveles de vanilla, Forge y Paper", () => {
    const l = (text: string, stream: "stdout" | "stderr" = "stdout") => levelOf({ seq: 1, at_unix_ms: 0, stream, text });
    expect(l("[12:00:00] [Server thread/WARN]: x")).toBe("warn");
    expect(l("[30Oct2026 12:00:00.123] [main/ERROR] [net.minecraftforge/]: x")).toBe("error");
    expect(l("[12:00:00 INFO]: x")).toBe("info");
    expect(l("\tat java.base/java.lang.Thread.run(Thread.java:1583)")).toBe("error");
    expect(l("algo raro", "stderr")).toBe("error");
  });

  it("filtra por nivel y muestra la hora", async () => {
    const { user, con } = await open();
    const levels = within(con.getByRole("group", { name: "Niveles" }));
    expect(levels.getByRole("button", { name: /Avisos 1/ })).toBeInTheDocument();
    await user.click(levels.getByRole("button", { name: /Info/ }));
    expect(con.queryByText(/Done \(3.2s\)/)).not.toBeInTheDocument();
    expect(con.getByText(/Can't keep up/)).toBeInTheDocument();
    await user.click(levels.getByRole("button", { name: /Errores/ }));
    expect(con.queryByText(/Could not pass event/)).not.toBeInTheDocument();
    await user.click(levels.getByRole("button", { name: /Info/ }));
    await user.click(con.getByRole("checkbox", { name: "Hora" }));
    expect(con.getAllByText(/^\d{2}:\d{2}:\d{2}$/).length).toBeGreaterThan(0);
  });

  it("pide confirmar los comandos peligrosos y no los envía si cancelas", async () => {
    const { fake, user, con } = await open();
    const input = con.getByLabelText("Comando de Minecraft");
    await user.type(input, "/op Steve{Enter}");
    const dlg = within(await screen.findByRole("dialog", { name: "¿Enviar este comando?" }));
    expect(dlg.getByText(/poder total/)).toBeInTheDocument();
    await user.click(dlg.getByRole("button", { name: "Cancelar" }));
    expect(fake.calls.some((c) => c.op === "send_command")).toBe(false);
    expect(input).toHaveValue("/op Steve");

    await user.click(con.getByRole("button", { name: "Enviar" }));
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Enviar igualmente" }));
    expect(fake.calls).toContainEqual({ op: "send_command", id, command: "/op Steve" });

    await user.type(input, "say hola{Enter}");
    expect(fake.calls).toContainEqual({ op: "send_command", id, command: "say hola" });
  });

  it("guarda favoritos que rellenan la línea sin enviarla", async () => {
    const { fake, user, con } = await open();
    const input = con.getByLabelText("Comando de Minecraft");
    await user.type(input, "time set day");
    await user.click(con.getByRole("button", { name: "Guardar en favoritos" }));
    await user.clear(input);
    const favs = within(con.getByRole("list", { name: "Comandos favoritos" }));
    await user.click(favs.getByRole("button", { name: "/time set day" }));
    expect(input).toHaveValue("time set day");
    expect(fake.calls.some((c) => c.op === "send_command")).toBe(false);
    expect(JSON.parse(localStorage.getItem("merchosting.consoleFavs")!)[id]).toEqual(["time set day"]);
    await user.click(favs.getByRole("button", { name: "Quitar /time set day de favoritos" }));
    expect(con.queryByRole("list", { name: "Comandos favoritos" })).not.toBeInTheDocument();
  });

  it("lista de peligrosos", () => {
    expect(dangerOf("stop")).not.toBeNull();
    expect(dangerOf("minecraft:kill @e[type=item]")).not.toBeNull();
    expect(dangerOf("kill Steve")).toBeNull();
    expect(dangerOf("whitelist add Alex")).toBeNull();
    expect(dangerOf("whitelist off")).not.toBeNull();
    expect(dangerOf("say stop")).toBeNull();
  });
});
