import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

const KEY = "ABCD1234-EFGH5678-IJKL9012-MNOP3456";

beforeEach(() => {
  localStorage.clear();
  delete document.documentElement.dataset.theme;
});

async function openPage(name: string, setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name }));
  const page = await screen.findByRole("region", { name });
  return { fake, user, w: within(page) };
}

describe("Apoyar MercHosting", () => {
  it("compara honestamente, abre la página oficial y activa con la clave", async () => {
    const { fake, user, w } = await openPage("Apoyar MercHosting");
    expect(w.getByText(/único pago de US\$8/)).toBeInTheDocument();
    const row = w.getByRole("row", { name: /Más RAM, servidores/ });
    expect(within(row).getAllByLabelText("No")).toHaveLength(2);
    await user.click(w.getByRole("button", { name: "Apoyar por US$8" }));
    expect(fake.openedUrls).toContain("https://example.gumroad.com/l/merchosting");

    await user.type(w.getByLabelText("Clave de licencia"), "FAKE0000-FAKE0000");
    await user.click(w.getByRole("button", { name: "Activar" }));
    expect(await w.findByRole("alert")).toHaveTextContent(/does not exist/);

    await user.clear(w.getByLabelText("Clave de licencia"));
    await user.type(w.getByLabelText("Clave de licencia"), KEY);
    await user.click(w.getByRole("button", { name: "Activar" }));
    expect(await w.findByText(/Eres Supporter/)).toBeInTheDocument();
    expect(w.getByText(/a•••@gmail.com/)).toBeInTheDocument();
    expect(screen.getByText("Colaborador")).toBeInTheDocument();

    // Redes reales del creador.
    await user.click(w.getByRole("button", { name: /TikTok @amercfr/ }));
    await user.click(w.getByRole("button", { name: /YouTube @Amercson/ }));
    expect(fake.openedUrls).toEqual(expect.arrayContaining(["https://www.tiktok.com/@amercfr", "https://www.youtube.com/@Amercson"]));
  });

  it("sin tienda abierta: «Próximamente» y nada que activar", async () => {
    const { w } = await openPage("Apoyar MercHosting", (f) => {
      f.supporterState = { ...f.supporterState, available: false, checkout_url: null };
    });
    expect(await w.findByRole("button", { name: "Próximamente" })).toBeDisabled();
    expect(w.getByLabelText("Clave de licencia")).toBeDisabled();
  });

  it("un reembolso quita los extras y el tema vuelve al gratuito", async () => {
    const { fake, user, w } = await openPage("Apoyar MercHosting");
    await user.type(await w.findByLabelText("Clave de licencia"), KEY);
    await user.click(w.getByRole("button", { name: "Activar" }));
    await w.findByText(/Eres Supporter/);

    await user.click(screen.getByRole("tab", { name: "Apariencia" }));
    const look = within(await screen.findByRole("region", { name: "Apariencia" }));
    await user.click(within(look.getByRole("radiogroup", { name: "Tema" })).getByRole("radio", { name: /Nether/ }));
    expect(document.documentElement.dataset.theme).toBe("nether");

    fake.gumroadKeys[KEY].refunded = true;
    await user.click(screen.getByRole("tab", { name: "Apoyar MercHosting" }));
    const page = within(await screen.findByRole("region", { name: "Apoyar MercHosting" }));
    await user.click(await page.findByRole("button", { name: /Comprobar ahora/ }));
    expect(await page.findByText(/se desactivaron: La compra se reembolsó/)).toBeInTheDocument();
    await waitFor(() => expect(document.documentElement.dataset.theme).toBeUndefined());
    expect(screen.queryByText("Colaborador")).not.toBeInTheDocument();
  });
});

describe("Apariencia", () => {
  it("sin Supporter: fondos gratis disponibles y los exclusivos con candado", async () => {
    const { user, w } = await openPage("Apariencia");
    const nether = within(w.getByRole("radiogroup", { name: "Tema" })).getByRole("radio", { name: /Nether/ });
    expect(nether).toHaveAttribute("aria-disabled", "true");
    await user.click(nether);
    expect(document.documentElement.dataset.theme).toBeUndefined();
    expect(w.getByRole("checkbox", { name: /insignia/ })).toBeDisabled();

    expect(document.querySelector("canvas.particles")).not.toBeNull();
    await user.click(w.getByRole("radio", { name: /Sencillo/ }));
    expect(w.getByRole("radio", { name: /Sencillo/ })).toHaveAttribute("aria-checked", "true");
    expect(document.querySelector("canvas.particles")).toBeNull();
    expect(JSON.parse(localStorage.getItem("merchosting.appearance")!).background).toBe("sencillo");
  });

  it("celebra cuando un servidor termina de encender (Supporter) y se puede apagar", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ id: "a" , name: "Survival" }), makeSummary({ id: "b", name: "Creativo" }));
    fake.supporterState = { ...fake.supporterState, active: true, key_hint: "…3456" };
    render(<App api={fake} intro={false} />);
    await screen.findByText("Colaborador");
    fake.servers[0].status = { ...fake.servers[0].status, state: "ONLINE" };
    expect(await screen.findByTestId("celebration", {}, { timeout: 3000 })).toHaveTextContent("¡Survival está en línea!");
  });
});

describe("Fase 10 · parte 2", () => {
  it("modo claro para todos y se recuerda", async () => {
    delete document.documentElement.dataset.mode;
    const { user, w } = await openPage("Apariencia");
    await user.click(w.getByRole("radio", { name: "Claro" }));
    expect(document.documentElement.dataset.mode).toBe("light");
    expect(JSON.parse(localStorage.getItem("merchosting.appearance")!).mode).toBe("claro");
    await user.click(w.getByRole("radio", { name: "Oscuro" }));
    expect(document.documentElement.dataset.mode).toBeUndefined();
  });

  it("iconos alternativos: con candado sin Supporter", async () => {
    const free = await openPage("Apariencia");
    const iconGroup = within(free.w.getByRole("radiogroup", { name: "Icono de la app" }));
    expect(iconGroup.getByRole("radio", { name: /Oro/ })).toHaveAttribute("aria-disabled", "true");
    await free.user.click(iconGroup.getByRole("radio", { name: /Oro/ }));
    expect(free.fake.windowIcons).toEqual([]);
  });

  it("con Supporter el icono cambia de verdad y vuelve al oficial al perderlo", async () => {
    const { fake, user, w } = await openPage("Apoyar MercHosting");
    await user.type(await w.findByLabelText("Clave de licencia"), KEY);
    await user.click(w.getByRole("button", { name: "Activar" }));
    await w.findByText(/Eres Supporter/);
    await user.click(screen.getByRole("tab", { name: "Apariencia" }));
    const icons = within(within(await screen.findByRole("region", { name: "Apariencia" })).getByRole("radiogroup", { name: "Icono de la app" }));
    await user.click(icons.getByRole("radio", { name: /Nether/ }));
    await waitFor(() => expect(fake.windowIcons.at(-1)).toContain("#e8431c"));

    fake.gumroadKeys[KEY].refunded = true;
    await user.click(screen.getByRole("tab", { name: "Apoyar MercHosting" }));
    const page = within(await screen.findByRole("region", { name: "Apoyar MercHosting" }));
    await user.click(await page.findByRole("button", { name: /Comprobar ahora/ }));
    await waitFor(() => expect(fake.windowIcons.at(-1)).toBeNull());
  });

  it("los créditos solo muestran a quien lo pidió (vacío por ahora)", async () => {
    const { w } = await openPage("Apoyar MercHosting");
    const g = within(await w.findByRole("region", { name: "Gracias a" }));
    expect(g.getByText("Todavía no hay nombres aquí.")).toBeInTheDocument();
    expect(g.getByText(/Solo aparece quien lo pide/)).toBeInTheDocument();
  });
});
