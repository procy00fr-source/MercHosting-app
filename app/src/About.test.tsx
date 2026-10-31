import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import thirdParty from "./about/terceros.json";
import { changelogVersions } from "./about/markdown";
import { FakeApi } from "./test/fakeApi";

describe("Acerca de", () => {
  it("muestra versión, licencia GPL, privacidad, novedades y terceros sin enlaces inventados", async () => {
    const fake = new FakeApi();
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Acerca de" }));
    const page = within(await screen.findByRole("region", { name: "Acerca de" }));
    expect(await page.findByText(/^Versión /)).toBeInTheDocument();
    expect(page.getByText("GPL-3.0-or-later")).toBeInTheDocument();
    // Sin repositorio configurado: botón desactivado con explicación.
    expect(page.getByRole("button", { name: /Código fuente/ })).toBeDisabled();
    expect(page.getByText(/el enlace aparecerá aquí/)).toBeInTheDocument();

    await user.click(page.getByRole("button", { name: "Ver la licencia" }));
    expect(await screen.findByText(/GNU GENERAL PUBLIC LICENSE/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cerrar" }));

    await user.click(page.getByRole("button", { name: "Leer la política de privacidad" }));
    const priv = within(await screen.findByRole("dialog", { name: "Política de privacidad" }));
    expect(priv.getByText(/no envía estadísticas de uso ni telemetría/)).toBeInTheDocument();
    await user.click(priv.getByRole("button", { name: "Cerrar" }));

    await user.click(page.getByRole("button", { name: "Ver la lista y sus licencias" }));
    const third = within(await screen.findByRole("dialog", { name: "Componentes de terceros" }));
    await user.type(third.getByRole("searchbox", { name: "Buscar componente" }), "tauri");
    expect(third.getAllByText(/^tauri/).length).toBeGreaterThan(0);
    await user.click(third.getByRole("button", { name: "Cerrar" }));

    await user.click(page.getByRole("button", { name: /Versiones publicadas/ }));
    expect(fake.openedUrls).toContain("https://github.com/procy00fr-source/MercHosting-releases/releases");
  });

  it("la lista de terceros y el historial tienen datos reales", () => {
    expect(thirdParty.length).toBeGreaterThan(100);
    expect(thirdParty.every((t) => t.license.length > 0)).toBe(true);
    expect(changelogVersions("## 0.2.4\n\n- Uno\n- Dos\n\n## 0.2.3\n\n- Tres")).toEqual([
      { version: "0.2.4", items: ["Uno", "Dos"] },
      { version: "0.2.3", items: ["Tres"] },
    ]);
  });
});
