import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { MercApi } from "./api/client";
import type { CreateRequest, LocalServer, LocalStatus, PhoneApi, PhoneInfo, PhonePlayit } from "./api/phone";
import { PHONE_WARNINGS, phoneApiFrom } from "./api/phone";
import { MobileApp } from "./components/MobileApp";
import { setPlatformForTests } from "./platform";
import { FakeApi } from "./test/fakeApi";

const INFO: PhoneInfo = {
  arch: "aarch64",
  supported: true,
  reason: null,
  total_ram_mb: 8192,
  heap: { suggested_mb: 2048, max_mb: 3584 },
  jre_installed: [],
  playit_available: true,
  warnings: [...PHONE_WARNINGS],
};

class FakePhone implements PhoneApi {
  info_: PhoneInfo = INFO;
  list: LocalServer[] = [];
  created: CreateRequest | null = null;
  state: LocalStatus["state"] = "stopped";
  eula = true;
  started: string[] = [];
  commands: string[] = [];
  asked = 0;
  linked = false;
  async info() {
    return this.info_;
  }
  async versions() {
    return [
      { mc_version: "1.21.1", java_major: 21, latest: true },
      { mc_version: "1.20.1", java_major: 17, latest: false },
    ];
  }
  async create(req: CreateRequest) {
    this.created = req;
    const s: LocalServer = {
      id: "loc1",
      name: req.name,
      flavor: req.flavor,
      mc_version: req.mc_version,
      java_major: 21,
      heap_mb: req.heap_mb,
      jar: "paper.jar",
      created_unix_ms: 1,
      playit: false,
    };
    this.list.push(s);
    return s;
  }
  async progress() {
    return { running: false, step: "", done: 0, total: null, error: null };
  }
  async cancel() {}
  async servers() {
    return this.list;
  }
  async status(): Promise<LocalStatus> {
    return { state: this.state, pid: this.state === "stopped" ? null : 42, eula_accepted: this.eula, port: 25565, problem: null };
  }
  async console() {
    return this.state === "online" ? ["[Server thread/INFO]: Done (3.1s)!"] : [];
  }
  async start(id: string) {
    this.started.push(id);
    this.state = "online";
  }
  async stop() {
    this.state = "stopped";
  }
  async forceStop() {
    this.state = "stopped";
  }
  async command(_id: string, text: string) {
    this.commands.push(text);
  }
  async acceptEula() {
    this.eula = true;
  }
  async setPlayit(id: string, on: boolean) {
    const s = this.list.find((x) => x.id === id)!;
    s.playit = on;
    return { ...s };
  }
  async setHeap(id: string, heap_mb: number) {
    const s = this.list.find((x) => x.id === id)!;
    s.heap_mb = heap_mb;
    return { ...s };
  }
  async backup() {
    return {} as never;
  }
  async backups() {
    return [];
  }
  async remove(id: string) {
    this.list = this.list.filter((s) => s.id !== id);
  }
  async playit(): Promise<PhonePlayit> {
    const running = this.state === "online";
    return {
      available: this.info_.playit_available,
      running,
      linked: this.linked,
      agent: null,
      claim: null,
      address: this.linked ? "mio.joinmc.link" : null,
      error: null,
    };
  }
  async playitLink() {
    return { url: "https://playit.gg/claim/abcdef0123", state: "waiting_for_visit" as const, done: false, error: null };
  }
  async playitUnlink() {}
  async askNotifications() {
    this.asked += 1;
    return true;
  }
}

function app(fake: FakeApi): MercApi {
  return Object.assign(Object.create(fake), { canTransfer: false }) as MercApi;
}

beforeEach(() => {
  localStorage.clear();
  setPlatformForTests("android");
  document.documentElement.dataset.mobile = "true";
});
afterEach(() => {
  setPlatformForTests("windows");
  delete document.documentElement.dataset.mobile;
});

describe("Servidor dentro del celular", () => {
  it("no hay pestaña «En este celular» si la app no la ofrece (iOS)", async () => {
    render(<MobileApp api={app(new FakeApi())} />);
    await screen.findByRole("region", { name: "Conectar" });
    expect(screen.queryByRole("button", { name: "En este celular" })).toBeNull();
  });

  it("obliga a aceptar todos los avisos y la EULA, y crea un Paper con la memoria elegida", async () => {
    const phone = new FakePhone();
    const user = userEvent.setup();
    render(<MobileApp api={app(new FakeApi())} phone={phone} />);
    await user.click(await screen.findByRole("button", { name: "En este celular" }));
    const here = within(await screen.findByRole("region", { name: "En este celular" }));
    expect(here.getByText(/Bedrock no se puede dentro del celular/)).toBeInTheDocument();
    await user.click(await here.findByRole("button", { name: /Crear servidor Java en este celular/ }));
    const d = within(screen.getByRole("dialog", { name: "Crear un servidor en este celular" }));
    const go = d.getByRole("button", { name: /Entiendo los riesgos/ });
    const boxes = within(d.getByRole("group", { name: "Avisos de riesgo" })).getAllByRole("checkbox");
    expect(boxes).toHaveLength(7);
    for (const b of boxes.slice(0, 6)) await user.click(b);
    expect(go).toBeDisabled();
    await user.click(boxes[6]);
    expect(go).toBeEnabled();
    await user.click(go);
    // Bedrock no aparece entre los tipos.
    const tipo = d.getByLabelText("Tipo");
    expect(within(tipo).queryByRole("option", { name: /Bedrock/ })).toBeNull();
    expect(within(tipo).queryByRole("option", { name: /Forge/ })).toBeNull();
    await waitFor(() => expect(d.getByLabelText(/^Versión de Minecraft/)).toHaveValue("1.21.1"));
    const crear = d.getByRole("button", { name: "Crear servidor" });
    expect(crear).toBeDisabled();
    await user.click(d.getByRole("checkbox", { name: /acepto la EULA/ }));
    await user.click(crear);
    await waitFor(() =>
      expect(phone.created).toMatchObject({ flavor: "paper", mc_version: "1.21.1", heap_mb: 2048, accept_eula: true }),
    );
    expect([...phone.created!.warnings_accepted].sort()).toEqual([...PHONE_WARNINGS].sort());
    // Se abre el servidor recién creado.
    expect(await screen.findByRole("region", { name: "Mi servidor" })).toBeInTheDocument();
  });

  it("un celular que no sirve dice por qué y no ofrece crear", async () => {
    const phone = new FakePhone();
    phone.info_ = { ...INFO, supported: false, reason: "Este celular es armv7l: solo arm64.", heap: null };
    const user = userEvent.setup();
    render(<MobileApp api={app(new FakeApi())} phone={phone} />);
    await user.click(await screen.findByRole("button", { name: "En este celular" }));
    expect(await screen.findByText(/solo arm64/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Crear servidor Java/ })).toBeNull();
  });

  it("enciende (pidiendo el permiso de notificación), manda comandos, vincula Playit y detiene", async () => {
    const phone = new FakePhone();
    phone.list.push({
      id: "s1",
      name: "Survival",
      flavor: "paper",
      mc_version: "1.21.1",
      java_major: 21,
      heap_mb: 2048,
      jar: "paper.jar",
      created_unix_ms: 1,
      playit: false,
    });
    const fake = new FakeApi();
    const opened: string[] = [];
    const api = Object.assign(app(fake), { openUrl: async (u: string) => void opened.push(u) });
    const user = userEvent.setup();
    render(<MobileApp api={api} phone={phone} />);
    await user.click(await screen.findByRole("button", { name: "En este celular" }));
    await user.click(await screen.findByRole("button", { name: /Survival/ }));
    const view = within(await screen.findByRole("region", { name: "Survival" }));
    await user.click(view.getByRole("checkbox", { name: /Encender Playit junto con el servidor/ }));
    await waitFor(() => expect(phone.list[0].playit).toBe(true));
    await user.click(view.getByRole("button", { name: /Encender/ }));
    await waitFor(() => expect(phone.started).toEqual(["s1"]));
    expect(phone.asked).toBe(1);
    expect(await view.findByText("En línea")).toBeInTheDocument();
    await user.type(view.getByLabelText("Comando"), "say hola");
    await user.click(view.getByRole("button", { name: /Enviar/ }));
    await waitFor(() => expect(phone.commands).toEqual(["say hola"]));
    await user.click(await view.findByRole("button", { name: /Vincular con playit.gg/ }));
    await waitFor(() => expect(opened).toContain("https://playit.gg/claim/abcdef0123"));
    phone.linked = true;
    expect(await view.findByText("mio.joinmc.link", {}, { timeout: 4000 })).toBeInTheDocument();
    await user.click(view.getByRole("button", { name: /Detener/ }));
    expect(await view.findByText("Apagado")).toBeInTheDocument();
  }, 20_000);

  it("las peticiones salen con la forma que entiende merc_phone::api", async () => {
    const sent: unknown[] = [];
    const p = phoneApiFrom(
      async (op) => {
        sent.push(op);
        return null;
      },
      async () => true,
    );
    await p.setHeap("a", 2048);
    await p.command("a", "say x");
    await p.playit(null);
    expect(sent).toEqual([
      { op: "set_heap", id: "a", heap_mb: 2048 },
      { op: "command", id: "a", text: "say x" },
      { op: "playit", id: null },
    ]);
  });
});
