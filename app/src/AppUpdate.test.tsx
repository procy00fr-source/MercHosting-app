import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { FEEDBACK_FORM, prefilledUrl, validForm, type FeedbackForm } from "./feedback";
import { FakeApi, makeSummary } from "./test/fakeApi";

const FORM: FeedbackForm = {
  viewUrl: "https://docs.google.com/forms/d/e/1FAIpQLSe_prueba/viewform",
  fields: { kind: "entry.111", message: "entry.222", contact: "entry.333", version: "entry.444" },
  kinds: ["Idea", "Error", "Otro"],
};

async function openHelp(fake: FakeApi) {
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Ayuda y tutoriales" }));
  return { user, card: within(await screen.findByRole("region", { name: "Actualizar MercHosting" })) };
}

describe("Actualizar MercHosting", () => {
  beforeEach(() => localStorage.clear());

  it("en una compilación de desarrollo lo explica y no ofrece instalar", async () => {
    const fake = new FakeApi();
    const { user, card } = await openHelp(fake);
    await user.click(card.getByRole("button", { name: /Buscar actualizaciones/ }));
    expect(await card.findByText(/compilación de desarrollo/)).toBeInTheDocument();
    expect(card.queryByRole("button", { name: /Instalar/ })).not.toBeInTheDocument();
  });

  it("avisa al abrir, muestra novedades y detiene los servidores antes de instalar", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary());
    fake.servers[0].status = { ...fake.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
    fake.appUpdate = { enabled: true, current: "0.1.0", available: true, version: "0.2.0", notes: "- Pack para amigos\n- Alertas", date: null };
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    const help = await screen.findByRole("button", { name: "Ayuda y tutoriales (MercHosting 0.2.0 disponible)" }, { timeout: 5000 });
    await user.click(help);
    const card = within(await screen.findByRole("region", { name: "Actualizar MercHosting" }));
    expect(card.getByText(/- Pack para amigos/)).toBeInTheDocument();
    await user.click(card.getByRole("button", { name: "Instalar la versión 0.2.0" }));
    const dlg = within(screen.getByRole("dialog", { name: /Instalar MercHosting 0.2.0/ }));
    expect(dlg.getByText("Survival")).toBeInTheDocument();
    expect(fake.appUpdateInstalled).toBe(false);
    await user.click(dlg.getByRole("button", { name: "Detener y actualizar" }));
    await waitFor(() => expect(fake.appUpdateInstalled).toBe(true));
    expect(fake.calls.some((c) => c.op === "stop")).toBe(true);
    expect(await card.findByText(/Abriendo el instalador/)).toBeInTheDocument();
  });
});

describe("Comentarios", () => {
  it("con el formulario oficial aparece en el menú y manda la opción tal cual está en el formulario", async () => {
    const fake = new FakeApi();
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Comentarios" }));
    const page = within(screen.getByRole("region", { name: "Comentarios" }));
    await user.selectOptions(page.getByLabelText("Tipo"), "Error");
    await user.type(page.getByLabelText("Mensaje"), "hola");
    await user.click(page.getByRole("button", { name: /Abrir el formulario/ }));
    const url = new URL(fake.openedUrls.at(-1)!);
    expect(url.searchParams.get("entry.1692104274")).toBe("error");
    expect(url.searchParams.get("entry.75650578")).toBe("hola");
  });

  it("sin formulario configurado no aparece la sección", async () => {
    render(<App api={new FakeApi()} intro={false} feedbackForm={null} />);
    await screen.findByRole("tab", { name: "Ayuda y tutoriales" });
    expect(screen.queryByRole("tab", { name: "Comentarios" })).not.toBeInTheDocument();
  });

  it("abre el formulario oficial ya rellenado y solo con lo que se ve", async () => {
    const fake = new FakeApi();
    const user = userEvent.setup();
    render(<App api={fake} intro={false} feedbackForm={FORM} />);
    await user.click(await screen.findByRole("tab", { name: "Comentarios" }));
    const page = within(screen.getByRole("region", { name: "Comentarios" }));
    expect(page.getByRole("button", { name: /Abrir el formulario/ })).toBeDisabled();
    await user.selectOptions(page.getByLabelText("Tipo"), "Error");
    await user.type(page.getByLabelText("Mensaje"), "Se cierra al crear & guardar");
    await user.type(page.getByLabelText(/Cómo responderte/), "ana@correo.com");
    await user.click(page.getByRole("button", { name: /Abrir el formulario/ }));
    expect(await page.findByText(/Formulario abierto/)).toBeInTheDocument();
    const url = new URL(fake.openedUrls.at(-1)!);
    expect(url.origin + url.pathname).toBe(FORM.viewUrl);
    expect(url.searchParams.get("entry.111")).toBe("Error");
    expect(url.searchParams.get("entry.222")).toBe("Se cierra al crear & guardar");
    expect(url.searchParams.get("entry.333")).toBe("ana@correo.com");
    expect(url.searchParams.has("entry.444")).toBe(true);
  });

  it("valida la configuración y recorta mensajes largos", () => {
    expect(validForm(FORM)).toBe(true);
    expect(validForm(FEEDBACK_FORM)).toBe(true);
    expect(validForm({ ...FORM, viewUrl: "https://evil.example/forms/d/e/x/viewform" })).toBe(false);
    expect(validForm({ ...FORM, fields: { ...FORM.fields, kind: "nombre" } })).toBe(false);
    const u = new URL(prefilledUrl(FORM, { kind: "Idea", message: "x".repeat(5000), contact: " ", version: null }));
    expect(u.searchParams.get("entry.222")).toHaveLength(1500);
    expect(u.searchParams.has("entry.333")).toBe(false);
  });
});
