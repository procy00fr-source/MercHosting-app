import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(tab: string, setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "neo_forge" }));
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: tab }));
  const panel = within(await screen.findByRole("tabpanel", { name: tab }));
  return { fake, user, panel };
}

describe("Arrastrar y soltar", () => {
  it("en Mods copia los .jar a mods/, ignora lo demás y no sobrescribe", async () => {
    const { fake, panel } = await open("Mods", (f) => f.files.set("mods/repetido.jar", "jar"));
    await panel.findByText("Just Enough Items");
    act(() => fake.dropFiles({ type: "enter", paths: ["C:\\Descargas\\create.jar"] }));
    expect(panel.getByText(/Suelta para copiar el archivo .jar a/)).toBeInTheDocument();
    await act(async () =>
      fake.dropFiles({ type: "drop", paths: ["C:\\Descargas\\create.jar", "C:\\Descargas\\repetido.jar", "C:\\Descargas\\leeme.txt"] }),
    );
    const alert = await panel.findByRole("alert");
    expect(alert).toHaveTextContent("Copiado a mods: create.jar.");
    expect(alert).toHaveTextContent(/No se copiaron: repetido.jar \(ya existe/);
    expect(alert).toHaveTextContent("Ignorados por no ser del tipo correcto: leeme.txt.");
    expect(fake.calls).toContainEqual({ op: "upload_file", id, source: "C:\\Descargas\\create.jar", dest_dir: "mods" });
    expect(fake.calls.some((c) => c.op === "upload_file" && c.source.endsWith("leeme.txt"))).toBe(false);
    expect(panel.queryByText(/Suelta para copiar/)).not.toBeInTheDocument();
  });

  it("con el servidor encendido no copia nada", async () => {
    const { fake, panel } = await open("Mods", (f) => {
      f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 3 };
    });
    await panel.findByText(/detén el servidor primero/);
    act(() => fake.dropFiles({ type: "enter", paths: ["C:\\x.jar"] }));
    expect(panel.getByText("Detén el servidor para añadir mods.")).toBeInTheDocument();
    await act(async () => fake.dropFiles({ type: "drop", paths: ["C:\\x.jar"] }));
    expect(await panel.findByRole("alert")).toHaveTextContent(/Detén el servidor/);
    expect(fake.calls.some((c) => c.op === "upload_file")).toBe(false);
  });

  it("en Archivos copia a la carpeta abierta", async () => {
    const { fake, panel } = await open("Archivos");
    await panel.findByRole("button", { name: /Subir archivo/ });
    await act(async () => fake.dropFiles({ type: "drop", paths: ["C:\\mapa.png"] }));
    expect(await panel.findByText(/Copiado a la carpeta del servidor: mapa.png/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "upload_file", id, source: "C:\\mapa.png", dest_dir: "" });
  });
});
