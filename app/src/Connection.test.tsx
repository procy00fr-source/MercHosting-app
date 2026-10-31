import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "paper" }));
  fake.servers[0].status = { ...fake.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Conexión" }));
  const panel = screen.getByRole("tabpanel", { name: "Conexión" });
  return { fake, user, w: within(panel) };
}

describe("Conexión y Playit", () => {
  it("misma red: dirección local, probar y copiar", async () => {
    const { fake, user, w } = await open((f) => f.reachable.add("192.168.1.20:25565"));
    expect(await w.findByText("192.168.1.20:25565")).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Probar 192.168.1.20:25565" }));
    expect(fake.calls).toContainEqual({ op: "test_address", id, address: "192.168.1.20:25565" });
    expect(await w.findByText(/Funciona: responde en 48 ms/)).toBeInTheDocument();
  });

  it("activar, vincular con el enlace oficial, guía del túnel y dirección probada antes de copiar", async () => {
    const { fake, user, w } = await open();
    const card = within(await w.findByRole("region", { name: "Amigos por Internet" }));
    expect(card.getByText(/servicio externo y gratuito/)).toBeInTheDocument();
    expect(card.getByText(/tu propia cuenta de playit.gg/)).toBeInTheDocument();
    await user.click(card.getByRole("button", { name: /Activar Playit/ }));
    expect(fake.calls).toContainEqual({ op: "playit_enable" });

    await user.click(await card.findByRole("button", { name: /Vincular mi cuenta de Playit/ }));
    const claim = within(await card.findByLabelText("Vinculación"));
    expect(claim.getByText("https://playit.gg/claim/ab01ff1000")).toBeInTheDocument();
    await user.click(claim.getByRole("button", { name: /Abrir en el navegador/ }));
    expect(fake.openedUrls).toContain("https://playit.gg/claim/ab01ff1000");

    // Aprobado, pero todavía sin túnel: guía para crearlo en playit.gg.
    fake.approveClaim("");
    const guide = within(await card.findByLabelText("Crear el túnel", {}, { timeout: 5000 }));
    expect(guide.getByText(/Minecraft Java/)).toBeInTheDocument();
    expect(guide.getByText("25565")).toBeInTheDocument();
    await user.click(guide.getByRole("button", { name: /Abrir mi cuenta de playit.gg/ }));
    await waitFor(() => expect(fake.openedUrls).toContain("https://playit.gg/login/guest-account/abc"));

    // El túnel aparece solo; la dirección no se puede copiar hasta probarla.
    fake.approveClaim("dark-sea.joinmc.link");
    expect(await card.findByText("dark-sea.joinmc.link", {}, { timeout: 5000 })).toBeInTheDocument();
    expect(card.getByRole("button", { name: "Copiar dark-sea.joinmc.link" })).toBeDisabled();
    expect(card.getByText("Sin probar")).toBeInTheDocument();
    await user.click(card.getByRole("button", { name: "Probar dark-sea.joinmc.link" }));
    expect(await card.findByText(/No responde/)).toBeInTheDocument();
    expect(card.getByRole("button", { name: "Copiar dark-sea.joinmc.link" })).toBeDisabled();
    fake.reachable.add("dark-sea.joinmc.link");
    await user.click(card.getByRole("button", { name: "Probar dark-sea.joinmc.link" }));
    expect(await card.findByText(/Funciona: responde en 48 ms · Paper 1.21.1 · 2\/20 jugadores/)).toBeInTheDocument();
    expect(card.getByRole("button", { name: "Copiar dark-sea.joinmc.link" })).toBeEnabled();
  }, 20_000);

  it("desvincular pide confirmación y desactivar apaga el agente", async () => {
    const { fake, user, w } = await open((f) => {
      f.playit = { ...f.playit, enabled: true, installed: true, running: true };
      f.approveClaim();
    });
    const card = within(await w.findByRole("region", { name: "Amigos por Internet" }));
    await user.click(await card.findByRole("button", { name: /Desvincular esta PC/ }));
    const dlg = screen.getByRole("dialog", { name: /Desvincular esta PC/ });
    await user.click(within(dlg).getByRole("button", { name: "Desvincular" }));
    expect(fake.calls).toContainEqual({ op: "playit_unlink" });
    expect(await card.findByRole("button", { name: /Vincular mi cuenta de Playit/ })).toBeInTheDocument();
    await user.click(card.getByRole("button", { name: /Desactivar Playit/ }));
    expect(fake.calls).toContainEqual({ op: "playit_disable" });
    expect(await card.findByRole("button", { name: /Activar Playit/ })).toBeInTheDocument();
  });

  it("túnel hacia otro puerto: lo explica y ofrece usar ese puerto en este servidor", async () => {
    const { fake, user, w } = await open((f) => {
      f.servers[0].status = { ...f.servers[0].status, state: "STOPPED", has_console: false, pid: null };
      f.props.push({ key: "server-port", value: "25566" });
      f.playit = { ...f.playit, enabled: true, installed: true, running: true };
      f.approveClaim("domination-immigrants.tun.ply.gg", "127.0.0.1:25565");
    });
    const box = within(await w.findByLabelText("Puerto distinto"));
    expect(box.getByText(/lleva al puerto/)).toHaveTextContent("domination-immigrants.tun.ply.gg lleva al puerto 25565, pero este servidor usa el 25566");
    expect(box.getByText(/Origin Configuration/)).toBeInTheDocument();
    await user.click(box.getByRole("button", { name: "Usar el puerto 25565 en este servidor" }));
    expect(fake.calls).toContainEqual(expect.objectContaining({ op: "set_properties", changes: { "server-port": "25565" } }));
    expect(await w.findByText("domination-immigrants.tun.ply.gg", { selector: "code" })).toBeInTheDocument();
    expect(w.getByRole("button", { name: "Probar domination-immigrants.tun.ply.gg" })).toBeInTheDocument();
  });

  it("si el otro puerto es de otro servidor, no ofrece quitárselo", async () => {
    const { w } = await open((f) => {
      f.props.push({ key: "server-port", value: "25566" });
      f.portsInUse = [{ port: 25565, server: "Creativo" }];
      f.playit = { ...f.playit, enabled: true, installed: true, running: true };
      f.approveClaim("domination-immigrants.tun.ply.gg", "127.0.0.1:25565");
    });
    const box = within(await w.findByLabelText("Puerto distinto"));
    expect(box.getByText(/lo usa tu servidor «Creativo»/)).toBeInTheDocument();
    expect(box.queryByRole("button", { name: /Usar el puerto/ })).not.toBeInTheDocument();
  });

  it("chat de voz: puerto real, diagnóstico y escribir voice_host con confirmación", async () => {
    const tunnel = { display_address: "147.185.221.181:25732", destination: "127.0.0.1:24454", is_disabled: false, disabled_reason: null };
    const { fake, user, w } = await open((f) => {
      f.playit = { ...f.playit, enabled: true, installed: true, running: true };
      f.approveClaim();
      f.playit.agent!.tunnels.push(tunnel);
      f.voice = {
        installed: true,
        enabled: true,
        jar: "plugins/voicechat-bukkit-2.5.32.jar",
        config: "plugins/voicechat/voicechat-server.properties",
        port_setting: 24454,
        port: 24454,
        voice_host: "",
        bind_address: "",
        tunnels: [tunnel],
        suggested_port: null,
        listening: 24454,
        restart_needed: false,
        problems: [
          { code: "host_missing", level: "error", message: "«voice_host» está vacío" },
          { code: "client_mod", level: "info", message: "Cada jugador necesita Simple Voice Chat instalado en su Minecraft para hablar y oír." },
        ],
      };
      f.layers = [
        { key: "server", state: "operativo", detail: "Encendido y aceptando jugadores.", checked_at_unix_ms: 1_793_300_300_000 },
        { key: "voice", state: "degradado", detail: "Escuchando en el puerto UDP 24454.", checked_at_unix_ms: 1_793_300_300_000 },
      ];
    });
    const status = within(await w.findByRole("region", { name: "Estado de la conexión" }));
    expect(status.getByLabelText("Chat de voz: Degradado")).toBeInTheDocument();
    expect(status.getByLabelText("Servidor (Java): Operativo")).toBeInTheDocument();
    const card = within(w.getByRole("region", { name: "Chat de voz" }));
    expect(card.getByText("plugins/voicechat/voicechat-server.properties")).toBeInTheDocument();
    expect(card.getByText(/Cada jugador necesita Simple Voice Chat/)).toBeInTheDocument();
    // El túnel de voz no se confunde con un túnel de Minecraft hacia otro puerto.
    expect(w.queryByLabelText("Puerto distinto")).toBeNull();
    await user.click(card.getByRole("button", { name: "Usar 147.185.221.181:25732 para la voz" }));
    const dlg = within(screen.getByRole("dialog", { name: "Escribir la dirección de la voz" }));
    expect(dlg.getByText(/guarda antes una copia/)).toBeInTheDocument();
    expect(fake.voiceWrites).toEqual([]);
    await user.click(dlg.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(fake.voiceWrites).toEqual([{ voice_host: "147.185.221.181:25732", port: undefined }]));
    await waitFor(() => expect(card.queryByText("«voice_host» está vacío")).toBeNull());
  });

  it("chat de voz sin túnel: explica el túnel de tipo Simple Voice Chat hacia su puerto", async () => {
    const { fake, user, w } = await open((f) => {
      f.playit = { ...f.playit, enabled: true, installed: true, running: true };
      f.approveClaim();
      f.voice = {
        installed: true,
        enabled: true,
        jar: "mods/voicechat.jar",
        config: "config/voicechat/voicechat-server.properties",
        port_setting: 24460,
        port: 24460,
        voice_host: "",
        bind_address: "",
        tunnels: [],
        suggested_port: null,
        listening: null,
        restart_needed: true,
        problems: [
          { code: "restart_needed", level: "warning", message: "Reinícialo para aplicarla." },
          { code: "no_tunnel", level: "warning", message: "Falta el túnel de voz." },
        ],
      };
    });
    const card = within(await w.findByRole("region", { name: "Chat de voz" }));
    expect(card.getByText("24460", { selector: "code" })).toBeInTheDocument();
    expect(card.getByText("Simple Voice Chat", { selector: "strong" })).toBeInTheDocument();
    await user.click(card.getByRole("button", { name: /Reiniciar avisando/ }));
    await waitFor(() => expect(fake.calls).toContainEqual({ op: "restart_with_notice", id, seconds: 60 }));
  });
});
