import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

beforeEach(() => localStorage.clear());

function setup(servers = 0, opts: { welcome?: boolean } = {}) {
  const fake = new FakeApi();
  for (let i = 0; i < servers; i++) fake.servers.push(makeSummary({ id: `s${i}`, name: `Server ${i}` }));
  const user = userEvent.setup();
  const r = render(<App api={fake} intro={false} welcome={opts.welcome ?? false} />);
  return { fake, user, ...r };
}

describe("Bienvenida", () => {
  it("la primera vez explica el hosting local, muestra la PC real y guarda el modo elegido", async () => {
    const { user, unmount } = setup(0, { welcome: true });
    const dlg = await screen.findByRole("dialog", { name: "Bienvenido a MercHosting" });
    expect(within(dlg).getByText(/Tu PC es el servidor/)).toBeInTheDocument();
    await user.click(within(dlg).getByRole("button", { name: "Siguiente" }));
    expect(within(dlg).getByText("16 GB")).toBeInTheDocument();
    expect(within(dlg).getByText("12")).toBeInTheDocument();
    expect(within(dlg).getByText("4 GB")).toBeInTheDocument();
    await user.click(within(dlg).getByRole("button", { name: "Siguiente" }));
    expect(within(dlg).getByText(/nunca la acepta por ti/)).toBeInTheDocument();
    await user.click(within(dlg).getByRole("button", { name: "Siguiente" }));
    await user.click(within(dlg).getByRole("radio", { name: /Principiante/ }));
    await user.click(within(dlg).getByRole("button", { name: "Empezar sin tutorial" }));
    expect(screen.queryByRole("dialog", { name: "Bienvenido a MercHosting" })).not.toBeInTheDocument();
    expect(localStorage.getItem("merchosting.level")).toBe("principiante");
    unmount();
    setup(0, { welcome: true });
    await screen.findByRole("tab", { name: "Compañeros" });
    expect(screen.queryByRole("dialog", { name: "Bienvenido a MercHosting" })).not.toBeInTheDocument();
  });

  it("«Saltar» la cierra en cualquier paso", async () => {
    const { user } = setup(0, { welcome: true });
    const dlg = await screen.findByRole("dialog", { name: "Bienvenido a MercHosting" });
    await user.click(within(dlg).getByRole("button", { name: "Saltar" }));
    expect(localStorage.getItem("merchosting.welcomeSeen")).toBe("1");
  });
});

describe("Tutoriales", () => {
  it("«Ver el tutorial» recorre los controles reales y se puede salir con Esc", async () => {
    const { user } = setup(0, { welcome: true });
    const dlg = await screen.findByRole("dialog", { name: "Bienvenido a MercHosting" });
    for (let i = 0; i < 3; i++) await user.click(within(dlg).getByRole("button", { name: "Siguiente" }));
    await user.click(within(dlg).getByRole("button", { name: "Ver el tutorial" }));
    const card = await screen.findByRole("dialog", { name: "Tutorial: Primeros pasos" });
    expect(within(card).getByText("Tus servidores")).toBeInTheDocument();
    expect(within(card).getByText(/1 de 6/)).toBeInTheDocument();
    await user.click(within(card).getByRole("button", { name: "Siguiente" }));
    expect(await within(card).findByText("Crear un servidor")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: /Tutorial/ })).not.toBeInTheDocument();
  });

  it("crear servidor: avanza al hacer clic de verdad en «Crear servidor»", async () => {
    const { user } = setup(0);
    await user.click(await screen.findByRole("tab", { name: "Ayuda y tutoriales" }));
    await user.click(screen.getByRole("button", { name: "Empezar: Crear tu primer servidor" }));
    const card = await screen.findByRole("dialog", { name: "Tutorial: Crear tu primer servidor" });
    await waitFor(() => expect(within(card).getByText(/Haz clic en el control resaltado/)).toBeInTheDocument());
    expect(within(card).queryByRole("button", { name: "Siguiente" })).not.toBeInTheDocument();
    const crear = document.querySelector<HTMLButtonElement>('[data-tour="crear-servidor"]')!;
    crear.click();
    expect(await within(card).findByText("Los pasos")).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Crear un servidor" })).toBeInTheDocument();
  });

  it("los tutoriales de un servidor llevan a su pestaña; sin servidor no se pueden empezar", async () => {
    const empty = setup(0);
    await empty.user.click(await screen.findByRole("tab", { name: "Ayuda y tutoriales" }));
    expect(screen.getByRole("button", { name: "Empezar: Copias de seguridad" })).toBeDisabled();
    empty.unmount();

    const { user } = setup(2);
    await user.click(await screen.findByRole("tab", { name: "Ayuda y tutoriales" }));
    await user.click(screen.getByRole("button", { name: "Empezar: Copias de seguridad" }));
    expect(await screen.findByRole("tabpanel", { name: "Copias de seguridad" })).toBeInTheDocument();
    const card = screen.getByRole("dialog", { name: "Tutorial: Copias de seguridad" });
    expect(within(card).getByText("Haz una copia")).toBeInTheDocument();
  });
});

describe("Modo principiante", () => {
  it("oculta las secciones avanzadas y se cambia desde Ayuda", async () => {
    localStorage.setItem("merchosting.level", "principiante");
    const { user } = setup(2);
    await user.click(await screen.findByText("Server 0"));
    await screen.findByRole("tab", { name: "Resumen" });
    expect(screen.queryByRole("tab", { name: "Archivos" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Tareas programadas" })).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Copias de seguridad" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Ayuda y tutoriales" }));
    const help = await screen.findByRole("region", { name: "Ayuda y tutoriales" });
    expect(within(help).getByText(/Ocultas ahora: Archivos/)).toBeInTheDocument();
    await user.click(within(help).getByRole("radio", { name: "Avanzado" }));
    await user.click(screen.getByRole("button", { name: /Servidores/ }));
    await user.click(await screen.findByText("Server 0"));
    expect(await screen.findByRole("tab", { name: "Archivos" })).toBeInTheDocument();
  });
});
