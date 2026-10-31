import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { highlight, langOf } from "./highlight";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "neo_forge" }));
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Archivos" }));
  const panel = within(await screen.findByRole("tabpanel", { name: "Archivos" }));
  await panel.findByRole("button", { name: /^server.properties$/ });
  return { fake, user, panel };
}

describe("Archivos: mover, copiar, extraer y colores", () => {
  it("colorea sin cambiar el texto", () => {
    const text = "# comentario\nmotd=Hola\nmax-players=20\npvp=true\n[seccion]\n";
    const toks = highlight(text, langOf("server.properties"));
    expect(toks.map((t) => t.t).join("")).toBe(text);
    expect(toks.find((t) => t.t === "motd")?.c).toBe("k");
    expect(toks.find((t) => t.t === "20")?.c).toBe("n");
    expect(toks.find((t) => t.t === "# comentario")?.c).toBe("c");
    const json = '{\n  "pack": {"pack_format": 48, "ok": true}\n}';
    const jt = highlight(json, langOf("pack.mcmeta"));
    expect(jt.map((t) => t.t).join("")).toBe(json);
    expect(jt.find((t) => t.t === '"pack_format"')?.c).toBe("k");
    const yaml = "settings:\n  debug: false # nada\n  - item\n";
    expect(highlight(yaml, "yaml").map((t) => t.t).join("")).toBe(yaml);
    expect(langOf("mods/a.jar")).toBeNull();
  });

  it("el editor sigue siendo un cuadro de texto editable", async () => {
    const { user, panel } = await open();
    await user.click(panel.getByRole("button", { name: /^server.properties$/ }));
    const box = await panel.findByRole("textbox", { name: "Contenido de server.properties" });
    await user.type(box, "\npvp=false");
    expect(panel.getByText("Cambios sin guardar")).toBeInTheDocument();
  });

  it("mueve a otra carpeta eligiéndola en el selector", async () => {
    const { fake, user, panel } = await open();
    await user.click(panel.getByRole("button", { name: "Mover server.properties a otra carpeta" }));
    const dlg = within(await screen.findByRole("dialog", { name: "Mover «server.properties»" }));
    expect(dlg.getByRole("button", { name: "Mover aquí" })).toBeDisabled();
    await user.click(await dlg.findByRole("button", { name: /^config$/ }));
    await user.click(dlg.getByRole("button", { name: "Mover aquí" }));
    expect(await panel.findByText("Movido a config/server.properties")).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "move_file", id, path: "server.properties", dest_dir: "config" });
  });

  it("no deja meter una carpeta dentro de sí misma y copia sin borrar el original", async () => {
    const { fake, user, panel } = await open();
    await user.click(panel.getByRole("button", { name: "Copiar config a otra carpeta" }));
    const dlg = within(await screen.findByRole("dialog", { name: "Copiar «config»" }));
    expect(await dlg.findByRole("button", { name: /^config$/ })).toBeDisabled();
    await user.click(dlg.getByRole("button", { name: /^world$/ }));
    await user.click(dlg.getByRole("button", { name: "Copiar aquí" }));
    expect(await panel.findByText("Copiado a world/config")).toBeInTheDocument();
    expect(fake.files.has("config/a.toml")).toBe(true);
    expect(fake.files.has("world/config/a.toml")).toBe(true);
  });

  it("extrae un zip en una carpeta nueva y cuenta lo omitido", async () => {
    const { fake, user, panel } = await open((f) => f.files.set("mapa.zip", "zip"));
    await user.click(await panel.findByRole("button", { name: "Extraer mapa.zip" }));
    const dlg = within(await screen.findByRole("dialog", { name: "Extraer «mapa.zip»" }));
    expect(dlg.getByText(/Se crea la carpeta «mapa»/)).toBeInTheDocument();
    await user.click(dlg.getByRole("button", { name: "Extraer" }));
    expect(await panel.findByText(/Extraído en mapa: 1 archivo.*Se omitieron 1 entrada/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "extract_zip", id, path: "mapa.zip" });
    expect(panel.queryByRole("button", { name: "Extraer server.properties" })).not.toBeInTheDocument();
  });
});
