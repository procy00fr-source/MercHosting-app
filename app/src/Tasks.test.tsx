import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary());
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Tareas programadas" }));
  const panel = screen.getByRole("tabpanel", { name: "Tareas programadas" });
  return { fake, user, w: within(panel) };
}

describe("Tareas programadas", () => {
  it("crea una copia diaria desde la sugerencia, la pausa y la ejecuta a mano", async () => {
    const { fake, user, w } = await open();
    expect(await w.findByText("Todavía no hay tareas")).toBeInTheDocument();
    expect(w.getByText(/UTC−05:00/)).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: /Copia diaria a las 04:00/ }));
    const dlg = screen.getByRole("dialog", { name: "Nueva tarea" });
    expect(within(dlg).getByLabelText("Qué hacer")).toHaveValue("backup_world");
    // Solo de lunes a viernes.
    for (const d of ["lunes", "martes", "miércoles", "jueves", "viernes"]) await user.click(within(dlg).getByRole("button", { name: d }));
    await user.click(within(dlg).getByRole("button", { name: "Guardar tarea" }));
    expect(fake.calls).toContainEqual({
      op: "save_task",
      id,
      task: { id: null, enabled: true, action: { type: "backup", scope: "world" }, schedule: { type: "daily", times: ["04:00"], days: [1, 2, 3, 4, 5] } },
    });
    expect(await w.findByText("Copia del mundo")).toBeInTheDocument();
    expect(w.getByText(/De lunes a viernes a las 04:00 · Próxima:/)).toBeInTheDocument();

    await user.click(w.getByRole("checkbox", { name: "Pausar: Copia del mundo" }));
    expect(await w.findByText("Pausada")).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Ejecutar ahora: Copia del mundo" }));
    expect(fake.calls).toContainEqual({ op: "run_task_now", id, task_id: "tarea1" });
    expect(await w.findByText(/Hecha/)).toBeInTheDocument();
    expect(w.getByText(/\(a mano\)/)).toBeInTheDocument();
  });

  it("reinicio con aviso, política de jugadores y cada cierto tiempo; valida antes de guardar", async () => {
    const { fake, user, w } = await open();
    await user.click(await w.findByRole("button", { name: /Nueva tarea/ }));
    const dlg = screen.getByRole("dialog", { name: "Nueva tarea" });
    await user.selectOptions(within(dlg).getByLabelText("Qué hacer"), "restart");
    await user.selectOptions(within(dlg).getByLabelText(/Aviso en el chat/), "120");
    await user.click(within(dlg).getByRole("radio", { name: /Avisarles y hacerlo igual/ }));
    await user.click(within(dlg).getByRole("button", { name: "Cada cierto tiempo" }));
    const every = within(dlg).getByLabelText("Cada");
    await user.clear(every);
    await user.type(every, "0");
    await user.click(within(dlg).getByRole("button", { name: "Guardar tarea" }));
    expect(within(dlg).getByRole("alert")).toHaveTextContent("Entre 5 minutos y 7 días.");
    await user.clear(every);
    await user.type(every, "6");
    await user.click(within(dlg).getByRole("button", { name: "Guardar tarea" }));
    expect(fake.calls).toContainEqual({
      op: "save_task",
      id,
      task: {
        id: null,
        enabled: true,
        action: { type: "restart", notice_seconds: 120, if_players: "notify" },
        schedule: { type: "every", minutes: 360 },
      },
    });
    expect(await w.findByText("Reiniciar (aviso de 2 min, avisando a los jugadores)")).toBeInTheDocument();
    expect(w.getByText(/Cada 6 h/)).toBeInTheDocument();
    // Con el servidor apagado, ejecutarla ahora se omite.
    await user.click(w.getByRole("button", { name: /^Ejecutar ahora: Reiniciar/ }));
    expect(await w.findByText("Omitida")).toBeInTheDocument();
  });

  it("un comando «stop» se rechaza; editar y borrar", async () => {
    const { fake, user, w } = await open();
    await user.click(await w.findByRole("button", { name: /Nueva tarea/ }));
    let dlg = screen.getByRole("dialog", { name: "Nueva tarea" });
    await user.selectOptions(within(dlg).getByLabelText("Qué hacer"), "command");
    await user.type(within(dlg).getByLabelText(/Comando/), "stop");
    await user.click(within(dlg).getByRole("button", { name: "Guardar tarea" }));
    expect(await within(dlg).findByRole("alert")).toHaveTextContent("no se programa como comando");
    await user.clear(within(dlg).getByLabelText(/Comando/));
    await user.type(within(dlg).getByLabelText(/Comando/), "/weather clear");
    await user.click(within(dlg).getByRole("button", { name: "Guardar tarea" }));
    expect(await w.findByText("Comando: /weather clear")).toBeInTheDocument();

    await user.click(w.getByRole("button", { name: /^Editar: Comando/ }));
    dlg = screen.getByRole("dialog", { name: "Editar tarea" });
    await user.click(within(dlg).getByRole("button", { name: "Añadir hora" }));
    await user.click(within(dlg).getByRole("button", { name: "Guardar tarea" }));
    await waitFor(() => expect(fake.taskItems[0].schedule).toEqual({ type: "daily", times: ["04:00", "12:00"], days: [] }));
    expect(fake.calls.filter((c) => c.op === "save_task").at(-1)).toMatchObject({ task: { id: "tarea1" } });

    await user.click(w.getByRole("button", { name: /^Borrar: Comando/ }));
    await user.click(within(screen.getByRole("dialog", { name: "Borrar tarea" })).getByRole("button", { name: "Borrar tarea" }));
    expect(await w.findByText("Todavía no hay tareas")).toBeInTheDocument();
  });
});
