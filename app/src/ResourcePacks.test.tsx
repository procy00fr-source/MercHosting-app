import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

describe("Navegador de paquetes de recursos", () => {
  it("busca en Modrinth, solo deja usar versiones del Minecraft del servidor y lo comprueba al ponerlo", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary());
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Ajustes" }));
    const card = within(await screen.findByRole("region", { name: "Paquete de recursos" }));
    await user.click(card.getByRole("button", { name: /Buscar en Modrinth/ }));

    const dlg = within(await screen.findByRole("dialog", { name: "Paquetes de recursos de Modrinth" }));
    expect(dlg.getByText(/para Minecraft 1.21.1/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({ op: "resource_pack_search", id, query: "", sort: "downloads", offset: 0 });
    await user.type(dlg.getByRole("searchbox", { name: "Buscar paquetes de recursos" }), "suaves{Enter}");
    expect(fake.calls).toContainEqual({ op: "resource_pack_search", id, query: "suaves", sort: "relevance", offset: 0 });
    await user.click(await dlg.findByRole("button", { name: /Texturas Suaves/ }));

    const d2 = within(await screen.findByRole("dialog", { name: "Texturas Suaves" }));
    expect(d2.getByRole("button", { name: "Usar Texturas Suaves 1.0" })).toBeDisabled();
    expect(d2.getByText("No es para Minecraft 1.21.1.")).toBeInTheDocument();
    await user.click(d2.getByRole("button", { name: "Ver en Modrinth" }));
    expect(fake.openedUrls).toContain("https://modrinth.com/project/texturas-suaves");
    await user.click(d2.getByRole("button", { name: "Usar Texturas Suaves 2.0" }));

    expect(await card.findByText(/Texturas Suaves 2.0: paquete comprobado y guardado con su huella SHA-1/)).toBeInTheDocument();
    expect(fake.calls).toContainEqual({
      op: "set_resource_pack",
      id,
      url: "https://cdn.modrinth.com/data/RP1/versions/rpv2/suaves-2.0.zip",
      required: false,
    });
    expect(card.getByLabelText("Enlace del paquete (.zip)")).toHaveValue("https://cdn.modrinth.com/data/RP1/versions/rpv2/suaves-2.0.zip");
  });
});
