import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { startDomTranslator } from "./i18n/dom";
import { Translator } from "./i18n/translate";

const tr = new Translator({
  exact: { Servidores: "Servers", Iniciar: "Start", "Hay una versión más nueva": "A newer version is available", "Buscar mods": "Search mods", reiniciar: "restart" },
  patterns: [
    ["{0} servidor{1} · {2} en línea", "Servers: {0} · {2} online"],
    ["Hay {0} mods", "There are {0} mods"],
    ["Aviso antes de {0}", "Warning before you {0}"],
  ],
});

describe("Traductor", () => {
  it("traduce textos exactos y con huecos, y deja lo desconocido tal cual", () => {
    expect(tr.tr("Servidores")).toBe("Servers");
    expect(tr.tr("  Iniciar ")).toBe(" Start ");
    expect(tr.tr("3 servidores · 1 en línea")).toBe("Servers: 3 · 1 online");
    expect(tr.tr("Aviso antes de reiniciar")).toBe("Warning before you restart");
    expect(tr.tr("Survival de Ana")).toBe("Survival de Ana");
    expect(tr.tr("42")).toBe("42");
  });

  it("traduce lo que pinta React y sigue sus cambios, sin tocar lo marcado como no traducible", async () => {
    function View() {
      const [n, setN] = useState(3);
      return (
        <div>
          <h1>Servidores</h1>
          <p data-testid="count">Hay {n} mods</p>
          <button type="button" onClick={() => setN(n + 1)} title="Iniciar">
            Iniciar
          </button>
          <input placeholder="Buscar mods" aria-label="Buscar mods" />
          <span className="mono">Servidores</span>
          <span translate="no">Iniciar</span>
        </div>
      );
    }
    const host = document.createElement("div");
    document.body.appendChild(host);
    const stop = startDomTranslator(host, tr);
    render(<View />, { container: host });
    await act(async () => {});
    expect(screen.getByRole("heading")).toHaveTextContent("Servers");
    expect(screen.getByTestId("count")).toHaveTextContent("There are 3 mods");
    const btn = screen.getByRole("button", { name: "Start" });
    expect(btn).toHaveAttribute("title", "Start");
    expect(screen.getByRole("textbox")).toHaveAttribute("placeholder", "Search mods");
    expect(host.querySelector(".mono")).toHaveTextContent("Servidores");
    expect(host.querySelector('[translate="no"]')).toHaveTextContent("Iniciar");
    await userEvent.click(btn);
    await act(async () => {});
    expect(screen.getByTestId("count")).toHaveTextContent("There are 4 mods");
    await userEvent.click(btn);
    await act(async () => {});
    expect(screen.getByTestId("count")).toHaveTextContent("There are 5 mods");
    stop();
    host.remove();
  });
});

describe("Elegir idioma", () => {
  it("Apariencia guarda el idioma elegido", async () => {
    const { App } = await import("./App");
    const { FakeApi } = await import("./test/fakeApi");
    const fake = new FakeApi();
    render(<App api={fake} intro={false} />);
    await userEvent.click(await screen.findByRole("tab", { name: "Apariencia" }));
    const group = await screen.findByRole("radiogroup", { name: "Idioma" });
    expect(screen.getByRole("radio", { name: "Español" })).toHaveAttribute("aria-checked", "true");
    await userEvent.click(screen.getByRole("radio", { name: "English" }));
    expect(localStorage.getItem("merchosting.lang")).toBe("en");
    expect(group).toBeInTheDocument();
  });
});
