import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { temurinFor } from "./components/JavaInstall";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

describe("Instalador de Java", () => {
  it("elige la versión LTS de Temurin que cumple lo pedido", () => {
    expect(temurinFor(16)).toBe(17);
    expect(temurinFor(21)).toBe(21);
    expect(temurinFor(8)).toBe(8);
    expect(temurinFor(22)).toBe(25);
  });

  it("sin Java compatible, lo descarga, aparece en la lista y se puede usar", async () => {
    const fake = new FakeApi();
    fake.servers.push(makeSummary({ kind: "paper" }));
    fake.java = { candidates: [{ path: "C:\\Java8\\bin\\java.exe", major: 8, description: "", error: null, jdk: false }], recommended: null, recommended_major: 21 };
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Ajustes" }));
    const card = within(await screen.findByRole("region", { name: "Java y memoria" }));
    await user.click(card.getByRole("button", { name: "Cambiar Java" }));
    const dlg = within(screen.getByRole("dialog", { name: "Elegir Java" }));
    expect(await dlg.findByText("No hay un Java compatible en esta PC.")).toBeInTheDocument();
    expect(dlg.getByText(/no se instala nada en el sistema/)).toBeInTheDocument();
    await user.click(dlg.getByRole("button", { name: "Descargar Java 21 (Eclipse Temurin)" }));
    expect(fake.calls).toContainEqual({ op: "install_java", major: 21 });
    const option = await dlg.findByRole("radio", { name: /Java 21/ }, { timeout: 5000 });
    await user.click(option);
    await user.click(dlg.getByRole("button", { name: "Usar este Java" }));
    await waitFor(() =>
      expect(fake.calls).toContainEqual({
        op: "set_launch",
        id,
        java: "C:\\Users\\ana\\AppData\\Local\\MercHosting\\java\\jdk-21-jre\\bin\\java.exe",
      }),
    );
  });
});
