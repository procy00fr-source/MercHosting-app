import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

async function open(setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "paper" }));
  fake.servers.push(makeSummary({ id: "33333333-3333-4333-8333-333333333333", name: "Creativo", kind: "paper" }));
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Compañeros" }));
  const page = within(await screen.findByLabelText("Compañeros", { selector: "div" }));
  return { fake, user, page };
}

describe("Servidor casero", () => {
  it("el anfitrión crea un código de control total solo tras confirmarlo y lo ve en la lista", async () => {
    const { fake, user, page } = await open((f) => {
      f.companionsState = { ...f.companionsState, enabled: true, online: true };
    });
    const mine = within(page.getByRole("region", { name: "Tu PC" }));
    await user.click(await mine.findByRole("button", { name: /Control total \(servidor casero\)/ }));
    const dlg = within(screen.getByRole("dialog", { name: "Control total de esta PC" }));
    expect(dlg.getByText(/Nunca podrá ejecutar programas de esta PC/)).toBeInTheDocument();
    const create = dlg.getByRole("button", { name: "Crear código" });
    expect(create).toBeDisabled();
    await user.click(dlg.getByRole("checkbox", { name: /tendrá el control de todos mis servidores/ }));
    await user.click(create);
    expect(await dlg.findByLabelText("Código de control total")).toHaveTextContent("MH1-CASERO2ABCDEF");
    expect(fake.calls).toContainEqual({ op: "create_host_invite", minutes: 30 });
    await user.click(dlg.getByRole("button", { name: "Listo" }));
    expect(await mine.findByText("Control total de esta PC")).toBeInTheDocument();
  });

  it("un compañero con control total aparece marcado y sin permisos por servidor", async () => {
    const { page } = await open((f) => {
      f.companionsState = {
        ...f.companionsState,
        enabled: true,
        online: true,
        companions: [
          {
            companion: "ab".repeat(32),
            name: "Mi PC",
            fingerprint: "MNOP-QRST-UVWX",
            added_at_unix_ms: 1,
            last_seen_unix_ms: null,
            connected: true,
            servers: [],
            host_admin: true,
          },
        ],
      };
    });
    const list = within(await page.findByRole("list", { name: "Compañeros" }));
    expect(list.getByText(/Control total de esta PC: todos los servidores/)).toBeInTheDocument();
    expect(list.queryByRole("button", { name: "Cambiar permisos" })).toBeNull();
  });

  it("desde la PC principal: explica cómo conectarse y crea un servidor allí sin elegir carpeta", async () => {
    const { fake, user, page } = await open((f) => {
      const casero = new FakeApi();
      f.friend = {
        fake: casero,
        host: {
          host: "cd".repeat(32),
          name: "casero",
          fingerprint: "ABCD-EFGH-IJKL",
          servers: [],
          last_ok_unix_ms: 1,
          last_error: null,
          host_admin: true,
        },
      };
      f.friendHosts = [structuredClone(f.friend.host)];
    });
    const theirs = within(page.getByRole("region", { name: "Servidores de amigos" }));
    const help = within(theirs.getByRole("group", { name: "Conectar a mi servidor casero" }));
    expect(help.getByText("merc servicio instalar")).toBeInTheDocument();
    expect(help.getByText("merc compañeros invitar --control-total")).toBeInTheDocument();
    const pcs = within(await theirs.findByRole("list", { name: "PCs de amigos" }));
    expect(pcs.getByText(/Tu servidor casero: tienes el control total/)).toBeInTheDocument();
    expect(pcs.getByText("Todavía no hay servidores en esa PC.")).toBeInTheDocument();

    await user.click(pcs.getByRole("button", { name: /Crear servidor aquí/ }));
    const w = within(screen.getByRole("dialog", { name: "Crear un servidor" }));
    await user.click(w.getByRole("button", { name: /^Bedrock/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(await w.findByRole("button", { name: /^1\.21\.124\.2/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(await w.findByText(/Otra PC nunca decide dónde se escribe/)).toBeInTheDocument();
    expect(w.queryByRole("button", { name: /Cambiar ubicación/ })).toBeNull();
    await waitFor(() => expect(w.getByRole("button", { name: /Siguiente/ })).toBeEnabled());
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    await user.click(w.getByRole("checkbox", { name: /acepto la EULA y la Política de privacidad/ }));
    await user.click(w.getByRole("button", { name: /Siguiente/ }));
    expect(await w.findByText("La elige casero")).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Crear servidor" }));
    // La petición viaja a la otra PC, que pone su carpeta.
    await waitFor(() => expect(fake.friend!.fake.created).not.toBeNull());
    expect(fake.calls.some((c) => c.op === "remote_call" && c.request.op === "create_server")).toBe(true);
    expect(fake.friend!.fake.created).toMatchObject({ flavor: "bedrock", accept_eula: true, path: "/casero/Servidores/x" });
    expect(fake.calls.some((c) => c.op === "create_server")).toBe(false);
  }, 20_000);
});
