import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import type { LpSnapshot } from "./api/types";
import { FakeApi, makeSummary } from "./test/fakeApi";

const NOTCH = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
function sample(): LpSnapshot {
  return {
    generated_at: "2026-10-30 12:00:00 UTC",
    groups: [
      {
        name: "admin",
        weight: 10,
        prefix: "&c[Admin] ",
        parents: ["default"],
        nodes: [
          { kind: "permission", key: "essentials.fly", value: true, expiry: null, context: {} },
          { kind: "permission", key: "worldedit.*", value: true, expiry: null, context: { world: ["creativo"] } },
        ],
      },
      { name: "default", weight: null, prefix: null, parents: [], nodes: [] },
    ],
    users: [
      {
        uuid: NOTCH,
        username: "Notch",
        primary_group: "admin",
        parents: ["admin"],
        nodes: [{ kind: "inheritance", key: "group.vip", value: true, expiry: 1900000000, context: {} }],
      },
    ],
    tracks: [{ name: "staff", groups: ["default", "admin"] }],
  };
}

async function open(setup: (f: FakeApi) => void, online = true) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary());
  if (online) fake.servers[0].status = { ...fake.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
  setup(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Permisos (LuckPerms)" }));
  const panel = screen.getByRole("tabpanel", { name: "Permisos (LuckPerms)" });
  return { fake, user, w: within(panel) };
}

describe("Permisos con LuckPerms", () => {
  beforeEach(() => localStorage.clear());

  it("sin LuckPerms lo dice y no inventa grupos", async () => {
    const { w } = await open(() => {});
    expect(await w.findByText(/LuckPerms no está instalado/)).toBeInTheDocument();
    expect(w.queryByRole("list", { name: "Grupos" })).not.toBeInTheDocument();
  });

  it("apagado pide encender el servidor", async () => {
    const { w } = await open((f) => {
      f.lp = sample();
    }, false);
    expect(await w.findByText(/Enciende el servidor para ver y cambiar los permisos/)).toBeInTheDocument();
    expect(w.getByText("LuckPerms-Bukkit-5.5.17.jar")).toBeInTheDocument();
    expect(w.queryByRole("button", { name: /Crear grupo/ })).not.toBeInTheDocument();
  });

  it("muestra grupos con herencia, contextos y caducidades, y aplica cambios tras revisarlos", async () => {
    const { fake, user, w } = await open((f) => {
      f.lp = sample();
    });
    const admin = within(await w.findByRole("region", { name: "Grupo admin" }));
    expect(admin.getByText("&c[Admin]")).toBeInTheDocument();
    expect(admin.getByText("default")).toBeInTheDocument();
    expect(admin.getByText("world=creativo")).toBeInTheDocument();
    expect(w.getByText(/default → admin/)).toBeInTheDocument();

    // Añadir un permiso: primero la vista previa con el comando exacto.
    await user.type(admin.getByLabelText("Permiso nuevo"), "essentials.home");
    await user.click(admin.getByRole("button", { name: /Añadir permiso/ }));
    let dlg = within(await screen.findByRole("dialog", { name: "Revisar el cambio" }));
    expect(await dlg.findByText("luckperms group admin permission set essentials.home true")).toBeInTheDocument();
    expect(fake.lpApplied).toEqual([]);
    await user.click(dlg.getByRole("button", { name: "Aplicar" }));
    expect(await w.findByText(/Hecho y comprobado: Dar el permiso essentials.home/)).toBeInTheDocument();
    expect(within(w.getByRole("region", { name: "Grupo admin" })).getByText("essentials.home")).toBeInTheDocument();
    expect(w.getByText(/Hay 1 copia de los permisos/)).toBeInTheDocument();

    // Un nodo inválido ni se propone.
    await user.type(within(w.getByRole("region", { name: "Grupo admin" })).getByLabelText("Permiso nuevo"), "a b");
    expect(w.getByRole("button", { name: /Añadir permiso/ })).toBeDisabled();

    // Crear un grupo.
    await user.click(w.getByRole("button", { name: /Crear grupo/ }));
    dlg = within(screen.getByRole("dialog", { name: "Crear grupo" }));
    await user.type(dlg.getByLabelText("Nombre del grupo"), "admin");
    expect(dlg.getByRole("button", { name: "Continuar" })).toBeDisabled();
    await user.clear(dlg.getByLabelText("Nombre del grupo"));
    await user.type(dlg.getByLabelText("Nombre del grupo"), "vip");
    await user.click(dlg.getByRole("button", { name: "Continuar" }));
    dlg = within(await screen.findByRole("dialog", { name: "Revisar el cambio" }));
    await dlg.findByText("luckperms creategroup vip");
    await user.click(dlg.getByRole("button", { name: "Aplicar" }));
    expect(await w.findByRole("region", { name: "Grupo vip" })).toBeInTheDocument();
  });

  it("cambia los grupos de un usuario y avisa si LuckPerms no muestra el cambio", async () => {
    const { fake, user, w } = await open((f) => {
      f.lp = sample();
    });
    await user.click(await w.findByRole("button", { name: /Usuarios \(1\)/ }));
    const notch = within(w.getByRole("region", { name: "Usuario Notch" }));
    expect(notch.getByText(/caduca el/)).toBeInTheDocument();
    await user.click(notch.getByRole("button", { name: "Quitar el grupo admin" }));
    let dlg = within(await screen.findByRole("dialog", { name: "Revisar el cambio" }));
    expect(await dlg.findByText("Sacar a Notch del grupo «admin».")).toBeInTheDocument();
    await user.click(dlg.getByRole("button", { name: "Aplicar" }));
    await waitFor(() => expect(within(w.getByRole("region", { name: "Usuario Notch" })).queryByText("admin", { selector: ".chip" })).toBeNull());

    fake.lpApplies = false;
    await user.click(within(w.getByRole("region", { name: "Usuario Notch" })).getByRole("button", { name: /Añadir grupo/ }));
    dlg = within(await screen.findByRole("dialog", { name: "Revisar el cambio" }));
    await dlg.findByText(`luckperms user ${NOTCH} parent add admin`);
    await user.click(dlg.getByRole("button", { name: "Aplicar" }));
    expect(await w.findByText(/todavía no muestra el cambio/)).toBeInTheDocument();
    expect(fake.lpApplied).toHaveLength(2);
  });
});
