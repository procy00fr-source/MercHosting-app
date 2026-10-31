import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import type { Alert } from "./api/types";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

function alert(over: Partial<Alert> = {}): Alert {
  return {
    id: "al1",
    at_unix_ms: Date.now() + 60_000,
    level: "critical",
    kind: "crash",
    instance_id: id,
    server_name: "Survival",
    title: "«Survival» se cerró inesperadamente",
    message: "Código de salida 1.",
    read: false,
    ...over,
  };
}

async function open(kind: "paper" | "vanilla" = "paper", setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind }));
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} alertsPollMs={100} />);
  await user.click(await screen.findByRole("tab", { name: "Salud y rendimiento" }));
  const panel = screen.getByRole("tabpanel", { name: "Salud y rendimiento" });
  return { fake, user, w: within(panel) };
}

const online = (f: FakeApi) => {
  f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
};

describe("Salud y rendimiento", () => {
  beforeEach(() => localStorage.clear());

  it("muestra lo medido de la PC y mide el TPS con el comando del servidor", async () => {
    const { fake, user, w } = await open("paper", online);
    expect(await w.findByText("24 %")).toBeInTheDocument();
    expect(w.getByRole("meter", { name: "Memoria usada de la PC" })).toBeInTheDocument();
    expect(w.getByRole("meter", { name: "Disco del servidor usado" })).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: /Medir TPS/ }));
    expect(fake.calls).toContainEqual({ op: "measure_tps", id });
    const res = await w.findByLabelText("Resultado del TPS");
    expect(res).toHaveTextContent("20.0 últ. 1m");
    expect(res).toHaveTextContent("17.2 últ. 15m");
    expect(res).toHaveTextContent("Medido con el comando «tps»");
  });

  it("vanilla no inventa TPS", async () => {
    const { w } = await open("vanilla");
    expect(await w.findByText(/no informa su TPS/)).toBeInTheDocument();
    expect(w.queryByRole("button", { name: /Medir TPS/ })).not.toBeInTheDocument();
  });

  it("modo de rendimiento con vista previa, aplicar y deshacer", async () => {
    const { fake, user, w } = await open();
    const card = within(await w.findByRole("region", { name: "Modo de rendimiento" }));
    await user.click(await card.findByRole("button", { name: "Ligero" }));
    const table = card.getByRole("table");
    expect(table).toHaveTextContent("Distancia de visión (chunks)106");
    await user.click(card.getByRole("button", { name: "Aplicar «Ligero»" }));
    const call = fake.calls.find((c) => c.op === "set_properties");
    expect(call).toMatchObject({ changes: { "view-distance": "6", "simulation-distance": "4", "entity-broadcast-range-percentage": "75" } });
    expect(await card.findByText(/Modo «Ligero» aplicado/)).toBeInTheDocument();
    expect(fake.props.find((p) => p.key === "view-distance")?.value).toBe("6");
    await user.click(card.getByRole("button", { name: /Deshacer el cambio/ }));
    await waitFor(() => expect(fake.props.find((p) => p.key === "view-distance")?.value).toBe("10"));
  });

  it("aviso de jugar y hostear en la misma PC si el servidor puede usar más de la mitad de la memoria", async () => {
    const { w } = await open("paper", (f) => {
      f.servers[0].instance.launch.max_memory_mb = 12 * 1024;
    });
    expect(await w.findByText(/El servidor puede usar hasta/)).toBeInTheDocument();
  });
});

describe("Alertas", () => {
  beforeEach(() => localStorage.clear());

  it("campana con sin leer, notificación de Windows de lo nuevo y ajustes", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary());
    const user = userEvent.setup();
    render(<App api={fake} intro={false} alertsPollMs={100} />);
    const bell = await screen.findByRole("button", { name: "Alertas" });
    // Llega una caída nueva: notificación y contador.
    fake.alertItems = [alert()];
    expect(await screen.findByRole("button", { name: "Alertas: 1 sin leer" })).toBeInTheDocument();
    await waitFor(() =>
      expect(fake.notifications).toEqual([{ title: "«Survival» se cerró inesperadamente", body: "Survival: Código de salida 1." }]),
    );
    // No se repite.
    await new Promise((r) => setTimeout(r, 300));
    expect(fake.notifications).toHaveLength(1);
    void bell;

    await user.click(screen.getByRole("button", { name: "Alertas: 1 sin leer" }));
    const dlg = screen.getByRole("dialog", { name: "Alertas" });
    expect(within(dlg).getByRole("list", { name: "Lista de alertas" })).toHaveTextContent("Grave");
    await user.click(within(dlg).getByRole("button", { name: /Marcar todo como leído/ }));
    expect(fake.calls).toContainEqual({ op: "mark_alerts_read", ids: [] });
    await within(dlg).findByText("Todo leído");

    await user.click(within(dlg).getByRole("checkbox", { name: "Mostrar notificaciones de Windows" }));
    await user.click(within(dlg).getByRole("checkbox", { name: "Hay una actualización del software de un servidor" }));
    expect(within(dlg).getByRole("checkbox", { name: /El túnel de Playit deja de funcionar/ })).toBeChecked();
    expect(within(dlg).getByRole("checkbox", { name: "Falla una copia de seguridad automática" })).toBeChecked();
    const gb = within(dlg).getByLabelText(/Avisar si quedan menos de/);
    await user.clear(gb);
    await user.type(gb, "10");
    await user.click(within(dlg).getByRole("button", { name: "Guardar ajustes" }));
    expect(fake.calls).toContainEqual({
      op: "set_alert_settings",
      settings: { notify: false, crash: true, disk: true, resources: true, tasks: true, disk_min_gb: 10, tunnel: true, backups: true, updates: false },
    });
    // Con las notificaciones apagadas, una alerta nueva no avisa a Windows.
    fake.alertItems = [alert({ id: "al2", at_unix_ms: Date.now() + 120_000, title: "Otra" }), ...fake.alertItems];
    await screen.findByRole("button", { name: "Alertas: 1 sin leer" });
    await new Promise((r) => setTimeout(r, 300));
    expect(fake.notifications).toHaveLength(1);
  });

  it("las alertas viejas no se notifican al abrir por primera vez", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary());
    fake.alertItems = [alert({ at_unix_ms: Date.now() - 3_600_000 })];
    render(<App api={fake} intro={false} alertsPollMs={100} />);
    expect(await screen.findByRole("button", { name: "Alertas: 1 sin leer" })).toBeInTheDocument();
    await new Promise((r) => setTimeout(r, 300));
    expect(fake.notifications).toHaveLength(0);
  });
});
