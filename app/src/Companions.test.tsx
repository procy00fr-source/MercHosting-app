import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FILES_WARNING } from "./companions";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";
const friendServer = "22222222-2222-4222-8222-222222222222";

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

describe("Compañeros", () => {
  it("activar, invitar con advertencia obligatoria para archivos y mostrar el código una vez", async () => {
    const { fake, user, page } = await open();
    const mine = within(page.getByRole("region", { name: "Tu PC" }));
    expect(mine.getByText(/relés públicos gratuitos de n0/)).toBeInTheDocument();
    await user.click(await mine.findByRole("button", { name: /Permitir compañeros/ }));
    expect(fake.calls).toContainEqual({ op: "set_companions_enabled", enabled: true });
    await user.click(await mine.findByRole("button", { name: /Invitar a un compañero/ }));

    const dlg = within(screen.getByRole("dialog", { name: "Invitar a un compañero" }));
    // Observador por defecto: sin advertencia.
    expect(dlg.queryByText(FILES_WARNING)).toBeNull();
    await user.click(dlg.getByRole("radio", { name: /Administrador/ }));
    // Texto literal de la especificación y confirmación consciente.
    expect(dlg.getByText(FILES_WARNING)).toBeInTheDocument();
    expect(dlg.getByText(/archivos JAR que Java ejecuta en tu PC/)).toBeInTheDocument();
    const create = dlg.getByRole("button", { name: "Crear código" });
    expect(create).toBeDisabled();
    await user.click(dlg.getByRole("checkbox", { name: /Entiendo los riesgos/ }));
    expect(create).toBeEnabled();
    await user.click(create);
    const call = fake.calls.find((c) => c.op === "create_invite");
    expect(call).toMatchObject({ op: "create_invite", id, minutes: 60 });
    expect(call && "perms" in call && call.perms).toContain("console_command");

    const shown = within(await screen.findByRole("dialog", { name: "Código de invitación" }));
    expect(shown.getByLabelText("Código de invitación")).toHaveTextContent("MH1-PRUEBA2ABCDEF");
    expect(shown.getByText(/Solo se muestra ahora/)).toBeInTheDocument();
    await user.click(shown.getByRole("button", { name: "Listo" }));
    expect(await mine.findByRole("list", { name: "Invitaciones sin usar" })).toHaveTextContent("Survival");
  });

  it("ampliar a lectura de archivos pide la advertencia; quitar pide confirmación", async () => {
    const { fake, user, page } = await open((f) => {
      f.companionsState = {
        ...f.companionsState,
        enabled: true,
        online: true,
        companions: [
          {
            companion: "ab".repeat(32),
            name: "Shandé",
            fingerprint: "MNOP-QRST-UVWX",
            added_at_unix_ms: 1,
            last_seen_unix_ms: null,
            connected: true,
            servers: [{ id, name: "Survival", perms: ["console_read", "status"] }],
            host_admin: false,
          },
        ],
      };
    });
    const list = within(await page.findByRole("list", { name: "Compañeros" }));
    expect(list.getByText("Shandé")).toBeInTheDocument();
    expect(list.getByText(/Observador/)).toBeInTheDocument();
    await user.click(list.getByRole("button", { name: "Cambiar permisos" }));
    const dlg = within(screen.getByRole("dialog", { name: /Permisos de Shandé/ }));
    expect(dlg.queryByText(FILES_WARNING)).toBeNull();
    await user.click(dlg.getByRole("checkbox", { name: "Leer y descargar archivos" }));
    expect(dlg.getByText(FILES_WARNING)).toBeInTheDocument();
    expect(dlg.getByRole("button", { name: "Guardar" })).toBeDisabled();
    await user.click(dlg.getByRole("checkbox", { name: /Entiendo los riesgos/ }));
    await user.click(dlg.getByRole("button", { name: "Guardar" }));
    expect(fake.calls).toContainEqual({
      op: "set_companion_perms",
      companion: "ab".repeat(32),
      id,
      perms: ["status", "console_read", "files_read"],
    });

    await user.click(list.getByRole("button", { name: "Quitar a Shandé" }));
    await user.click(within(screen.getByRole("dialog", { name: "Quitar a Shandé" })).getByRole("button", { name: "Quitar" }));
    expect(fake.calls).toContainEqual({ op: "remove_companion", companion: "ab".repeat(32) });
    await waitFor(() => expect(page.queryByRole("list", { name: "Compañeros" })).toBeNull());
  });

  it("compañero: unirse con un código y abrir el servidor del amigo solo con sus permisos", async () => {
    const { fake, user, page } = await open((f) => {
      const friend = new FakeApi();
      friend.servers.push(makeSummary({ id: friendServer, name: "Server de Amir", kind: "paper" }));
      f.friend = {
        fake: friend,
        host: {
          host: "cd".repeat(32),
          name: "PC-DE-AMIR",
          fingerprint: "ABCD-EFGH-IJKL",
          servers: [{ id: friendServer, name: "Server de Amir", perms: ["status", "console_read", "stop"] }],
          last_ok_unix_ms: 1,
          last_error: null,
          host_admin: false,
        },
      };
    });
    const theirs = within(page.getByRole("region", { name: "Servidores de amigos" }));
    await user.type(theirs.getByRole("textbox", { name: /Código que te dio tu amigo/ }), "MH1-MAL");
    await user.type(theirs.getByRole("textbox", { name: /Tu nombre/ }), "Shandé");
    await user.click(theirs.getByRole("button", { name: "Unirme" }));
    expect(await page.findByText(/El código no es válido/)).toBeInTheDocument();
    const code = theirs.getByRole("textbox", { name: /Código que te dio tu amigo/ });
    await user.clear(code);
    await user.type(code, "MH1-AMIGO");
    await user.click(theirs.getByRole("button", { name: "Unirme" }));
    const pcs = within(await theirs.findByRole("list", { name: "PCs de amigos" }));
    expect(pcs.getByText("PC-DE-AMIR")).toBeInTheDocument();
    await user.click(pcs.getByRole("button", { name: "Abrir" }));

    expect(await screen.findByRole("heading", { name: "Server de Amir" }, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.getByText(/PC de PC-DE-AMIR/)).toBeInTheDocument();
    // Solo las secciones permitidas: consola sí; archivos, ajustes y conexión no.
    expect(screen.getByRole("tab", { name: "Consola" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Archivos" })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Ajustes" })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Conexión" })).toBeNull();
    // Sin permiso de iniciar no hay botón Iniciar; las peticiones van por la PC del amigo.
    expect(screen.queryByRole("button", { name: "Iniciar" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Quitar de la lista/ })).toBeNull();
    expect(fake.calls).toContainEqual({ op: "remote_call", host: "cd".repeat(32), request: { op: "list_instances" } });
    expect(fake.calls.some((c) => c.op === "status" || c.op === "start")).toBe(false);
  }, 20_000);

  it("compañero: descargar un archivo del amigo a esta PC; sin permiso de edición no hay «Subir»", async () => {
    const { fake, user, page } = await open((f) => {
      const friend = new FakeApi();
      friend.servers.push(makeSummary({ id: friendServer, name: "Server de Amir", kind: "paper" }));
      f.friend = {
        fake: friend,
        host: {
          host: "cd".repeat(32),
          name: "PC-DE-AMIR",
          fingerprint: "ABCD-EFGH-IJKL",
          servers: [{ id: friendServer, name: "Server de Amir", perms: ["status", "files_list", "files_read"] }],
          last_ok_unix_ms: 1,
          last_error: null,
          host_admin: false,
        },
      };
      f.friendHosts = [structuredClone(f.friend.host)];
    });
    const pcs = within(await page.findByRole("list", { name: "PCs de amigos" }));
    await user.click(pcs.getByRole("button", { name: "Abrir" }));
    await user.click(await screen.findByRole("tab", { name: "Archivos" }, { timeout: 5000 }));
    await user.click(await screen.findByRole("button", { name: "Descargar server.properties" }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({
        op: "remote_download",
        host: "cd".repeat(32),
        item: { type: "file", id: friendServer, path: "server.properties" },
        dest: "C:\\Users\\ana\\Desktop\\copia.txt",
      }),
    );
    expect(screen.queryByRole("button", { name: /Subir archivo/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Ver carpeta del servidor/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "Mover server.properties a la papelera" })).toBeNull();
  }, 20_000);
});
