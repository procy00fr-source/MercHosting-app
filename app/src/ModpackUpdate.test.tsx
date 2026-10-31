import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(setup: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "neo_forge" }));
  setup(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Actualizaciones" }));
  const card = within(await screen.findByRole("region", { name: "Modpack" }));
  return { fake, user, card };
}

const linked = (f: FakeApi) => {
  f.modpackState = {
    linked: {
      source: "modrinth",
      project_id: "PACK1",
      title: "Aventura Épica",
      version_id: "packv1",
      version_number: "1.0",
      mc_version: "1.21.1",
      loader: "neoforge",
      loader_version: "21.1.77",
      installed_at_unix: 1,
      files: 200,
    },
    online: true,
    error: null,
    versions: [
      { version_id: "packv3", version_number: "3.0", name: "", date_published: "2026-11-01", game_versions: ["1.21.4"], compatible: false, current: false, newer: true },
      { version_id: "packv2", version_number: "2.0", name: "", date_published: "2026-10-01", game_versions: ["1.21.1"], compatible: true, current: false, newer: true },
      { version_id: "packv1", version_number: "1.0", name: "", date_published: "2026-09-01", game_versions: ["1.21.1"], compatible: true, current: true, newer: false },
    ],
  };
  f.modpackPlan = {
    from_version: "1.0",
    to_version: "2.0",
    to_version_id: "packv2",
    mc_version: "1.21.1",
    loader: "neoforge",
    loader_from: "21.1.77",
    loader_to: "21.1.80",
    loader_mismatch: true,
    changes: [
      { path: "mods/a-2.jar", kind: "add", from_override: false },
      { path: "mods/a-1.jar", kind: "remove", from_override: false },
      { path: "config/create.toml", kind: "keep_modified", from_override: true },
      { path: "mods/minimapa.jar", kind: "skip_removed", from_override: false },
    ],
    download_bytes: 3_000_000,
    notes: ["La versión nueva está hecha para neoforge 21.1.80 y el servidor tiene 21.1.77."],
  };
};

describe("Actualizar modpack", () => {
  it("muestra qué cambia, pide aceptar el cambio de loader y actualiza con copia previa", async () => {
    const { fake, user, card } = await open(linked);
    expect(await card.findByText("Hay una versión más nueva: 2.0.")).toBeInTheDocument();
    const select = card.getByLabelText("Versión del modpack");
    expect(within(select).queryByRole("option", { name: /3.0/ })).not.toBeInTheDocument();
    expect(select).toHaveValue("packv2");
    await user.click(card.getByRole("button", { name: "Ver qué cambia" }));

    const dlg = within(await screen.findByRole("dialog", { name: "Actualizar a 2.0" }));
    expect(dlg.getByText(/se hace una copia completa/)).toBeInTheDocument();
    expect(dlg.getByText(/se conserva tu versión/)).toBeInTheDocument();
    expect(dlg.getByText(/no se vuelven a poner/)).toBeInTheDocument();
    const go = dlg.getByRole("button", { name: "Actualizar (con copia previa)" });
    expect(go).toBeDisabled();
    await user.click(dlg.getByRole("checkbox"));
    await user.click(go);

    expect(await card.findByText(/actualizado a 2.0/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "modpack_update_apply", id, version_id: "packv2", accept_loader_change: true });
  });

  it("si no se sabe el modpack, deja vincularlo eligiendo la versión instalada", async () => {
    const { fake, user, card } = await open(() => {});
    await user.click(await card.findByRole("button", { name: /Vincular un modpack/ }));
    const dlg = within(await screen.findByRole("dialog", { name: "Vincular un modpack" }));
    await user.type(dlg.getByRole("searchbox", { name: "Buscar el modpack" }), "aventura{Enter}");
    await user.click(await dlg.findByRole("button", { name: /Aventura Épica/ }));
    await user.click(await dlg.findByRole("button", { name: "Tengo la 2.0" }));
    expect(await card.findByText("Aventura Épica")).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "modpack_link", id, source: "modrinth", version_id: "packv2" });
  });
});
