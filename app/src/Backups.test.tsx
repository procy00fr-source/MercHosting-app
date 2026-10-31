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
  await user.click(await screen.findByRole("tab", { name: "Copias de seguridad" }));
  const panel = screen.getByRole("tabpanel", { name: "Copias de seguridad" });
  return { fake, user, w: within(panel) };
}

const online = (f: FakeApi) => {
  f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
};

describe("Copias de seguridad", () => {
  it("hace una copia del mundo con nota y la restaura con copia previa", async () => {
    const { fake, user, w } = await open();
    expect(await w.findByText("Todavía no hay copias")).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: /Copia del mundo/ }));
    const dlg = screen.getByRole("dialog", { name: "Copia del mundo" });
    expect(dlg).toHaveTextContent("la copia es exacta");
    await user.type(within(dlg).getByLabelText("Nota (opcional)"), "Antes del evento");
    await user.click(within(dlg).getByRole("button", { name: "Hacer copia" }));
    expect(fake.calls).toContainEqual({ op: "create_backup", id, scope: "world", note: "Antes del evento" });
    expect(await w.findByText("Copia hecha y comprobada.")).toBeInTheDocument();
    expect(w.getByText(/Verificada/)).toBeInTheDocument();
    expect(w.getByText(/Antes del evento/)).toBeInTheDocument();

    const backup = fake.backupItems[0].id;
    await user.click(w.getByRole("button", { name: /^Restaurar copia/ }));
    const rd = screen.getByRole("dialog", { name: /Restaurar la copia/ });
    expect(rd).toHaveTextContent("Antes de restaurar");
    await user.click(within(rd).getByRole("button", { name: "Restaurar" }));
    expect(fake.calls).toContainEqual({ op: "restore_backup", id, backup });
    expect(await w.findByText(/Copia restaurada/)).toBeInTheDocument();
    expect(w.getAllByText("Antes de restaurar").length).toBeGreaterThan(0);

    await user.click(w.getByRole("button", { name: "Ver carpeta" }));
    expect(fake.openedBackupFolders).toEqual([id]);
  });

  it("encendido: copia con el guardado pausado, pero no deja restaurar", async () => {
    const { fake, user, w } = await open((f) => {
      online(f);
      f.backupItems = [];
    });
    expect(await w.findByText(/se pausa el guardado automático/)).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: /Copia completa/ }));
    await user.click(within(screen.getByRole("dialog", { name: "Copia completa" })).getByRole("button", { name: "Hacer copia" }));
    expect(fake.calls).toContainEqual({ op: "create_backup", id, scope: "full", note: null });
    expect(await w.findByText(/Hecha con el servidor encendido/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /^Restaurar copia/ })).toBeDisabled();
    expect(w.getByText(/detén antes el servidor/)).toBeInTheDocument();
  });

  it("una copia dañada se marca y no se puede restaurar; las protegidas no se borran", async () => {
    const { fake, user, w } = await open();
    await user.click(await w.findByRole("button", { name: /Copia del mundo/ }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Hacer copia" }));
    await w.findByText("Copia hecha y comprobada.");
    const b = fake.backupItems[0].id;
    fake.damagedBackups.add(b);
    await user.click(w.getByRole("button", { name: /^Verificar copia/ }));
    expect(fake.calls).toContainEqual({ op: "verify_backup", id, backup: b });
    expect(await w.findByText(/La copia está dañada/)).toBeInTheDocument();
    expect(w.getByText(/Dañada: «world\/level.dat»/)).toBeInTheDocument();
    expect(w.getByRole("button", { name: /^Restaurar copia/ })).toBeDisabled();

    await user.click(w.getByRole("button", { name: /^Proteger copia/ }));
    expect(fake.calls).toContainEqual({ op: "protect_backup", id, backup: b, protected: true });
    await waitFor(() => expect(w.getByRole("button", { name: /^Borrar copia/ })).toBeDisabled());
    await user.click(w.getByRole("button", { name: /^Quitar protección/ }));
    await waitFor(() => expect(w.getByRole("button", { name: /^Borrar copia/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /^Borrar copia/ }));
    const dd = screen.getByRole("dialog", { name: "Borrar copia" });
    expect(dd).toHaveTextContent("No se puede deshacer");
    await user.click(within(dd).getByRole("button", { name: "Borrar copia" }));
    expect(await w.findByText("Todavía no hay copias")).toBeInTheDocument();
  });

  it("ajuste de cuántas copias conservar", async () => {
    const { fake, user, w } = await open();
    const input = await w.findByLabelText(/Copias que se conservan/);
    await user.clear(input);
    await user.type(input, "0");
    expect(w.getByText("Un número entre 1 y 500.")).toBeInTheDocument();
    await user.clear(input);
    await user.type(input, "3");
    await user.click(within(w.getByRole("region", { name: "Ajustes de copias" })).getByRole("button", { name: "Guardar" }));
    expect(fake.calls).toContainEqual({ op: "set_backup_settings", id, keep: 3 });
    expect(await w.findByText("Se conservarán las 3 copias más nuevas.")).toBeInTheDocument();
    expect(w.getByText(/Espacio libre en el disco/)).toBeInTheDocument();
  });
});
