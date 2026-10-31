import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

async function openTab(tab: string, setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary());
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: tab }));
  const panel = screen.getByRole("tabpanel", { name: tab });
  return { fake, user, panel, w: within(panel) };
}

describe("MOTD e icono", () => {
  it("edita el MOTD con botones, lo previsualiza y lo guarda con «§» y dos líneas", async () => {
    const { fake, user, w } = await openTab("MOTD e icono");
    const l1 = await w.findByLabelText("Línea 1");
    await waitFor(() => expect(l1).toHaveValue("A Minecraft Server"));
    await user.clear(l1);
    await user.click(w.getByRole("button", { name: "Color Dorado" }));
    await user.click(w.getByRole("button", { name: "Negrita" }));
    await user.type(l1, "Mi Server");
    expect(l1).toHaveValue("&6&lMi Server");
    await user.type(w.getByLabelText("Línea 2"), "&7Entra ya");
    const line1 = w.getByTestId("mc-line-1");
    expect(line1).toHaveTextContent("Mi Server");
    expect(within(line1).getByText("Mi Server")).toHaveStyle({ color: "#FFAA00", fontWeight: "800" });
    expect(w.getByTestId("mc-line-2")).toHaveTextContent("Entra ya");

    await user.click(w.getByRole("button", { name: /Guardar MOTD/ }));
    expect(await w.findByText(/MOTD guardado/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "set_properties", id: fake.servers[0].instance.id, changes: { motd: "§6§lMi Server\n§7Entra ya" }, expected_modified: 1000 });
    expect(w.getByRole("button", { name: /Guardar MOTD/ })).toBeDisabled();
  });

  it("convierte una foto a 64×64, deja elegir cómo encajarla y la guarda", async () => {
    const { fake, user, w } = await openTab("MOTD e icono");
    await user.click(await w.findByRole("button", { name: /Elegir imagen/ }));
    expect(await w.findByText(/JPG 1920×1080 → 64×64/)).toBeInTheDocument();
    expect(w.getByAltText("Icono nuevo")).toHaveAttribute("src", "data:image/png;base64,PREVIEW-crop");
    await user.click(w.getByLabelText(/Imagen completa/));
    await waitFor(() => expect(w.getByAltText("Icono nuevo")).toHaveAttribute("src", "data:image/png;base64,PREVIEW-contain"));
    expect(fake.calls.some((c) => c.op === "set_server_icon")).toBe(false);
    await user.click(w.getByRole("button", { name: /Usar como icono/ }));
    expect(await w.findByText(/server-icon.png \(64×64\)/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "set_server_icon", id: fake.servers[0].instance.id, source: fake.iconSource, fit: "contain" });
    expect(w.getByAltText("Icono del servidor")).toHaveAttribute("src", "data:image/png;base64,SAVED-contain");
    await user.click(w.getByRole("button", { name: /Quitar icono/ }));
    expect(await w.findByText(/papelera de MercHosting/)).toBeInTheDocument();
  });

  it("muestra el error si el archivo no es una imagen", async () => {
    const { user, w } = await openTab("MOTD e icono", (f) => {
      f.iconSource = "C:\\notas.txt";
    });
    await user.click(await w.findByRole("button", { name: /Elegir imagen/ }));
    expect(await w.findByRole("alert")).toHaveTextContent(/No se reconoce la imagen/);
  });
});

describe("Actualizaciones", () => {
  it("actualiza el servidor con copia previa y ofrece volver atrás", async () => {
    const { fake, user, w } = await openTab("Actualizaciones");
    expect(await w.findByText(/hay una más nueva/)).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: "Actualizar a 130" }));
    const dlg = screen.getByRole("dialog");
    expect(within(dlg).getByText(/copia completa/)).toBeInTheDocument();
    await user.click(within(dlg).getByRole("button", { name: "Hacer copia y actualizar" }));
    expect(await w.findByText(/Actualizado a 130/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "apply_server_update", id: fake.servers[0].instance.id, loader_version: "130" });
    expect(fake.backupItems[0].reason).toBe("before_update");
    await waitFor(() => expect(w.getByText(/es la más nueva/)).toBeInTheDocument());
    await user.click(w.getByRole("button", { name: /Volver atrás/ }));
    expect(await screen.findByRole("tabpanel", { name: "Copias de seguridad" })).toBeInTheDocument();
    expect(await screen.findByText("Antes de actualizar")).toBeInTheDocument();
  });

  it("explica por qué no se puede actualizar ese tipo de servidor", async () => {
    const { w } = await openTab("Actualizaciones", (f) => {
      f.serverUpdate = { ...f.serverUpdate, supported: false, reason: "Este tipo de servidor se actualiza con su instalador oficial." };
    });
    expect(await w.findByText(/instalador oficial/)).toBeInTheDocument();
    expect(w.queryByRole("button", { name: /Actualizar a/ })).not.toBeInTheDocument();
  });

  it("actualiza todos los mods con copia completa antes", async () => {
    const { fake, user, w } = await openTab("Actualizaciones");
    expect(await w.findByText("Just Enough Items")).toBeInTheDocument();
    await user.click(w.getByRole("button", { name: /Actualizar todos \(1\)/ }));
    expect(await w.findByText(/Actualizados: Just Enough Items 20.0/)).toBeInTheDocument();
    const ops = fake.calls.map((c) => c.op);
    expect(ops.indexOf("create_backup")).toBeLessThan(ops.indexOf("content_install"));
    expect(fake.calls).toContainEqual({ op: "content_install", id: fake.servers[0].instance.id, kind: "mod", source: "modrinth", version_id: "jei20" });
  });
});
