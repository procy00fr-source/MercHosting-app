import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { PLAYIT_LINUX_GUIDE, setPlatformForTests } from "./platform";
import { FakeApi, makeSummary } from "./test/fakeApi";

describe("MercHosting en Linux", () => {
  beforeEach(() => {
    localStorage.clear();
    setPlatformForTests("linux");
  });
  afterEach(() => setPlatformForTests("windows"));

  it("en un Linux sin agente verificado (p. ej. ARM), lo dice y enlaza la guía oficial (sin botón de activar)", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ kind: "paper" }));
    fake.playit = { ...fake.playit, supported: false };
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Conexión" }));
    const card = within(await screen.findByRole("region", { name: "Amigos por Internet" }));
    expect(await card.findByText(/no tiene un agente de Playit verificado/)).toBeInTheDocument();
    expect(card.queryByRole("button", { name: /Activar Playit/ })).not.toBeInTheDocument();
    await user.click(card.getByRole("button", { name: /Guía oficial de Playit para Linux/ }));
    expect(fake.openedUrls).toContain(PLAYIT_LINUX_GUIDE);
  });

  it("explica el no suspender con systemd-inhibit y el arranque al iniciar sesión", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ id: "a", name: "Survival" }), makeSummary({ id: "b", name: "Creativo" }));
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("button", { name: "Abrir Survival" }));
    await user.click(await screen.findByRole("tab", { name: "Ajustes" }));
    const card = within(await screen.findByRole("region", { name: "Si se cae y al iniciar sesión" }));
    expect(card.getByText(/systemd-inhibit/)).toBeInTheDocument();
    expect(card.getByRole("checkbox", { name: /^Arrancar MercHosting al iniciar sesión/ })).toBeEnabled();
  });

  it("con el paquete .deb no ofrece instalar solo: manda a la página de releases", async () => {
    const fake = new FakeApi();
    fake.appUpdate = { enabled: true, current: "0.1.0", available: true, version: "0.2.0", notes: null, date: null, self_install: false };
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: /Ayuda y tutoriales/ }));
    const card = within(await screen.findByRole("region", { name: "Actualizar MercHosting" }));
    await user.click(card.getByRole("button", { name: /Buscar actualizaciones/ }));
    expect(await card.findByText(/paquete .deb/)).toBeInTheDocument();
    expect(card.queryByRole("button", { name: /Instalar la versión/ })).not.toBeInTheDocument();
    await user.click(card.getByRole("button", { name: "Abrir la página de releases" }));
    await waitFor(() => expect(fake.openedUrls).toContain("https://github.com/procy00fr-source/MercHosting-releases/releases"));
  });
});
