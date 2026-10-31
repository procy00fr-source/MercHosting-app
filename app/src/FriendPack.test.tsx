import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

async function open(setup: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(makeSummary({ kind: "neo_forge" }));
  setup(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Invitar jugadores" }));
  const panel = within(await screen.findByRole("tabpanel", { name: "Invitar jugadores" }));
  const card = within(panel.getByRole("region", { name: "Pack para amigos" }));
  return { fake, user, card };
}

describe("Pack para amigos", () => {
  it("muestra qué va en el pack, qué va a mano y lo guarda como .mrpack", async () => {
    const { fake, user, card } = await open((f) => {
      f.savePath = "C:\\Users\\ana\\Desktop\\Survival (para jugar).mrpack";
      f.friendPackPlan = {
        mc_version: "1.21.1",
        loader: ["neoforge", "21.1.259"],
        included: [
          { title: "Create", file_name: "create.jar", optional: false, page_url: "https://modrinth.com/mod/create", size: 1 },
          { title: "JourneyMap", file_name: "jm.jar", optional: true, page_url: null, size: 1 },
        ],
        manual: [
          {
            title: "JEI",
            file_name: "jei.jar",
            page_url: "https://www.curseforge.com/minecraft/mc-mods/jei",
            reason: "Es de CurseForge: el formato .mrpack no admite sus descargas. Bájalo de su página.",
          },
        ],
        server_only: ["spark"],
        blocked: null,
      };
    });
    expect(card.getByText(/no se copia ningún mod dentro/)).toBeInTheDocument();
    await user.click(card.getByRole("button", { name: "Preparar el pack" }));
    expect(await card.findByText("Minecraft 1.21.1 con NeoForge 21.1.259.")).toBeInTheDocument();
    const inPack = within(card.getByRole("list", { name: "Mods en el pack" }));
    expect(inPack.getByText("Create")).toBeInTheDocument();
    expect(inPack.getByText("(opcional)")).toBeInTheDocument();
    const manual = within(card.getByRole("list", { name: "Mods a mano" }));
    expect(manual.getByText(/Es de CurseForge/)).toBeInTheDocument();
    await user.click(manual.getByRole("button", { name: /Página oficial/ }));
    expect(fake.openedUrls).toContain("https://www.curseforge.com/minecraft/mc-mods/jei");
    expect(card.getByText(/Solo del servidor.*spark/)).toBeInTheDocument();

    await user.click(card.getByRole("button", { name: /Guardar el pack/ }));
    expect(await card.findByText(/Guardado en C:\\Users\\ana\\Desktop\\Survival \(para jugar\)\.mrpack/)).toBeInTheDocument();
    expect(fake.exportedPacks).toHaveLength(1);
  });

  it("si no se puede armar, explica por qué y no ofrece guardar", async () => {
    const { user, card } = await open(() => {});
    await user.click(card.getByRole("button", { name: "Preparar el pack" }));
    expect(await card.findByText(/Ningún mod de este servidor se puede incluir/)).toBeInTheDocument();
    expect(card.queryByRole("button", { name: /Guardar el pack/ })).not.toBeInTheDocument();
  });
});
