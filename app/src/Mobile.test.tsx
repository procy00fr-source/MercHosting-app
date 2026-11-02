import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { MercApi } from "./api/client";
import type { RemoteHost } from "./api/types";
import { MobileApp } from "./components/MobileApp";
import { ROLE_PERMS } from "./companions";
import { setPlatformForTests } from "./platform";
import { FakeApi, makeSummary } from "./test/fakeApi";

const casero = (servers: RemoteHost["servers"] = []): RemoteHost => ({
  host: "cd".repeat(32),
  name: "PC casera",
  fingerprint: "ABCD-EFGH-IJKL",
  servers,
  last_ok_unix_ms: 1,
  last_error: null,
  host_admin: true,
});

/** El celular: sin supervisor propio y sin traer ni subir archivos. */
function phone(f: FakeApi): MercApi {
  return Object.assign(Object.create(f), { canTransfer: false }) as MercApi;
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

describe("MercHosting en el celular", () => {
  it("sin PCs abre «Conectar», rechaza un código malo y se une con uno bueno", async () => {
    const fake = new FakeApi();
    const pc = new FakeApi();
    fake.friend = { fake: pc, host: casero() };
    const user = userEvent.setup();
    render(<MobileApp api={phone(fake)} identity={async () => "MNOP-QRST-UVWX"} />);
    const join = within(await screen.findByRole("region", { name: "Conectar" }));
    expect(join.getByText("merc compañeros invitar --control-total")).toBeInTheDocument();
    await user.type(join.getByLabelText(/Código de invitación/), "MH1-MALO");
    await user.type(join.getByLabelText(/Nombre de este celular/), "Celular de Amir");
    await user.click(join.getByRole("button", { name: "Conectar" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/no es válido/);
    await user.clear(join.getByLabelText(/Código de invitación/));
    await user.type(join.getByLabelText(/Código de invitación/), "MH1-AMIGO");
    await user.click(join.getByRole("button", { name: "Conectar" }));
    expect(fake.calls).toContainEqual({ op: "join_with_code", code: "MH1-AMIGO", name: "Celular de Amir" });
    const pcs = within(await screen.findByRole("region", { name: "Mis PCs" }));
    expect(pcs.getByText("PC casera")).toBeInTheDocument();
    expect(pcs.getByText("Control total de esa PC")).toBeInTheDocument();
    expect(localStorage.getItem("merchosting.mobileName")).toBe("Celular de Amir");

    // Navegación inferior.
    await user.click(screen.getByRole("button", { name: "Ayuda" }));
    expect(await screen.findByText(/Huella de este celular/)).toBeInTheDocument();
    expect(screen.getByText("MNOP-QRST-UVWX")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Mis PCs" }));
    expect(await screen.findByRole("region", { name: "Mis PCs" })).toBeInTheDocument();
  });

  it("crea un servidor Bedrock en la PC con control total, lo abre y activa Playit allí", async () => {
    const fake = new FakeApi();
    const pc = new FakeApi();
    fake.friend = { fake: pc, host: casero() };
    fake.friendHosts = [casero()];
    const user = userEvent.setup();
    render(<MobileApp api={phone(fake)} />);
    const pcs = within(await screen.findByRole("region", { name: "Mis PCs" }));
    await user.click(await pcs.findByRole("button", { name: /Crear servidor \(Java o Bedrock\)/ }));
    const w = within(screen.getByRole("dialog", { name: "Crear un servidor" }));
    // Se puede elegir Java o Bedrock.
    expect(w.getByRole("button", { name: /^Paper/ })).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: /^Bedrock/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: /^1\.21\.124\.2/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(w.queryByRole("button", { name: /Cambiar ubicación/ })).toBeNull();
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(w.getByRole("checkbox", { name: /acepto la EULA y la Política de privacidad/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    // La PC elige la carpeta: el celular nunca manda una ruta propia.
    fake.friendHosts = [casero([{ id: "id-creado", name: "Nuevo", perms: [...ROLE_PERMS.administrador] }])];
    await user.click(w.getByRole("button", { name: "Crear servidor" }));
    await waitFor(() => expect(pc.created).toMatchObject({ flavor: "bedrock", path: "/casero/Servidores/x", accept_eula: true }));
    await user.click(await w.findByRole("button", { name: /Ir al servidor/ }, { timeout: 8000 }));

    // Se abre el servidor nuevo, con el control total: Conexión muestra Playit de esa PC.
    await user.click(await screen.findByRole("tab", { name: "Conexión" }, { timeout: 8000 }));
    const card = within(await screen.findByRole("region", { name: "Amigos por Internet" }));
    await user.click(await card.findByRole("button", { name: /Activar Playit/ }));
    await waitFor(() =>
      expect(fake.calls.some((c) => c.op === "remote_call" && c.request.op === "playit_enable")).toBe(true),
    );
  }, 30_000);

  it("en el celular no aparecen botones para subir o descargar archivos", async () => {
    const fake = new FakeApi();
    const pc = new FakeApi();
    pc.servers.push(makeSummary({ id: "s1", name: "Survival", kind: "paper" }));
    const h = casero([{ id: "s1", name: "Survival", perms: [...ROLE_PERMS.administrador] }]);
    fake.friend = { fake: pc, host: h };
    fake.friendHosts = [h];
    const user = userEvent.setup();
    render(<MobileApp api={phone(fake)} />);
    await user.click(await screen.findByRole("button", { name: /Survival/ }));
    await user.click(await screen.findByRole("tab", { name: "Archivos" }, { timeout: 8000 }));
    const panel = await screen.findByRole("tabpanel", { name: "Archivos" });
    await waitFor(() => expect(within(panel).getByRole("button", { name: /Nueva carpeta/ })).toBeInTheDocument());
    expect(within(panel).queryByRole("button", { name: /Subir archivo/ })).toBeNull();
    expect(within(panel).queryByRole("button", { name: /^Descargar / })).toBeNull();
  }, 20_000);
});
