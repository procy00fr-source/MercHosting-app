import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

describe("Asistente: versión del loader y keepInventory", () => {
  it("elige una versión concreta de Fabric y conserva el inventario", async () => {
    const fake = new FakeApi();
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click((await screen.findAllByRole("button", { name: "Crear servidor" }))[0]);
    const w = within(screen.getByRole("dialog", { name: "Crear un servidor" }));
    await user.click(w.getByRole("button", { name: /^Fabric/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: /^1\.21\.11/ }));
    const sel = await w.findByLabelText("Versión del loader");
    expect(within(sel).getByRole("option", { name: "Recomendada (0.16.10)" })).toBeInTheDocument();
    expect(within(sel).getByRole("option", { name: "0.17.0-beta.1 (beta)" })).toBeInTheDocument();
    await user.selectOptions(sel, "0.16.9");
    await w.findByLabelText("Java que usará el servidor");
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("checkbox", { name: /Conservar el inventario/ }));
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(w.getByRole("checkbox", { name: /He leído y acepto la EULA/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(await w.findByText(/versión 0\.16\.9/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "resolve", flavor: "fabric", mc_version: "1.21.11", loader_version: "0.16.9" });
    await user.click(w.getByRole("button", { name: "Crear servidor" }));
    await waitFor(() => expect(fake.created).not.toBeNull());
    expect(fake.created).toMatchObject({ flavor: "fabric", loader_version: "0.16.9", game_rules: { keep_inventory: "true" } });
  });
});

describe("Clonar y archivar", () => {
  function two() {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ id: "a", name: "Survival" }), makeSummary({ id: "b", name: "Creativo" }));
    return fake;
  }

  it("clona un servidor detenido y abre la copia", async () => {
    const fake = two();
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("button", { name: "Abrir Survival" }));
    await user.click(await screen.findByRole("button", { name: /Clonar/ }));
    const dlg = screen.getByRole("dialog", { name: "Clonar «Survival»" });
    expect(within(dlg).getByLabelText("Nombre de la copia")).toHaveValue("Survival (copia)");
    expect(within(dlg).getByText(/otro puerto/)).toBeInTheDocument();
    await user.click(within(dlg).getByRole("button", { name: "Clonar" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Survival (copia)" }, { timeout: 3000 })).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "clone_server", id: "a", name: "Survival (copia)" });
  });

  it("archivado: no se enciende, va aparte en la lista y se desarchiva", async () => {
    const fake = two();
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("button", { name: "Abrir Survival" }));
    await user.click(await screen.findByRole("button", { name: /^Archivar/ }));
    await waitFor(() => expect(screen.getByRole("button", { name: /Iniciar/ })).toBeDisabled());
    expect(screen.getByText(/Archivado: no se puede encender/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Servidores/ }));
    const archivedGroup = await screen.findByText("Archivados (1)");
    const group = archivedGroup.closest("details")!;
    expect(within(group).getByRole("button", { name: "Abrir Survival" })).toBeInTheDocument();
    expect(within(group).getByText("Archivado")).toBeInTheDocument();

    await user.click(within(group).getByRole("button", { name: "Abrir Survival" }));
    await user.click(await screen.findByRole("button", { name: /Desarchivar/ }));
    await waitFor(() => expect(screen.getByRole("button", { name: /Iniciar/ })).toBeEnabled());
  });
});

describe("Automatización", () => {
  it("reinicio tras caídas y arrancar con Windows, desde Ajustes", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ id: "a", name: "Survival" }), makeSummary({ id: "b", name: "Creativo" }));
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("button", { name: "Abrir Survival" }));
    await user.click(await screen.findByRole("tab", { name: "Ajustes" }));
    const card = within(await screen.findByRole("region", { name: "Si se cae y al iniciar sesión" }));
    const restart = card.getByRole("checkbox", { name: /Reiniciar solo si se cae/ });
    expect(restart).not.toBeChecked();
    await user.click(restart);
    await waitFor(() => expect(restart).toBeChecked());
    expect(fake.calls).toContainEqual({ op: "set_automation", id: "a", auto_restart: true, autostart: null });

    expect(card.getByText(/Necesita «Arrancar MercHosting al iniciar sesión»/)).toBeInTheDocument();
    await user.click(card.getByRole("checkbox", { name: /^Arrancar MercHosting al iniciar sesión/ }));
    await waitFor(() => expect(card.getByText(/Se enciende solo al iniciar sesión/)).toBeInTheDocument());
    await user.click(card.getByRole("checkbox", { name: /Encender este servidor al iniciar sesión/ }));
    await waitFor(() => expect(card.getByRole("checkbox", { name: /Encender este servidor/ })).toBeChecked());
    expect(fake.calls).toContainEqual({ op: "set_startup", enabled: true });
  });

  it("«Activar modo 24/7» enciende las cuatro cosas y se puede quitar el no suspender", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ id: "a", name: "Survival" }), makeSummary({ id: "b", name: "Creativo" }));
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("button", { name: "Abrir Survival" }));
    await user.click(await screen.findByRole("tab", { name: "Ajustes" }));
    const card = within(await screen.findByRole("region", { name: "Si se cae y al iniciar sesión" }));
    expect(card.getByText(/si apagas la PC, se va la luz o se corta el Internet/)).toBeInTheDocument();
    await user.click(card.getByRole("button", { name: "Activar modo 24/7" }));
    expect(await card.findByText("Está activado para este servidor.")).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "set_startup", enabled: true });
    expect(fake.calls).toContainEqual({ op: "set_keep_awake", enabled: true });
    expect(fake.calls).toContainEqual({ op: "set_automation", id: "a", auto_restart: true, autostart: true });
    const awake = card.getByRole("checkbox", { name: /No suspender la PC/ });
    expect(awake).toBeChecked();
    expect(card.getByText(/Se activa cuando enciendes un servidor/)).toBeInTheDocument();
    await user.click(awake);
    await waitFor(() => expect(awake).not.toBeChecked());
    expect(fake.calls).toContainEqual({ op: "set_keep_awake", enabled: false });
    expect(card.getByRole("button", { name: "Activar modo 24/7" })).toBeInTheDocument();
  });

  it("avisa si dejó de reintentar y fuera de Windows lo explica", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ id: "a", name: "Survival" }), makeSummary({ id: "b", name: "Creativo" }));
    fake.automationState = { ...fake.automationState, auto_restart: true, gave_up: true, startup_available: false };
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("button", { name: "Abrir Survival" }));
    await user.click(await screen.findByRole("tab", { name: "Ajustes" }));
    const card = within(await screen.findByRole("region", { name: "Si se cae y al iniciar sesión" }));
    expect(card.getByText(/ya no se reinicia solo hasta que lo enciendas tú/)).toBeInTheDocument();
    expect(card.getByRole("checkbox", { name: /^Arrancar MercHosting al iniciar sesión/ })).toBeDisabled();
    expect(card.getByText("No está disponible en este sistema.")).toBeInTheDocument();
  });
});
