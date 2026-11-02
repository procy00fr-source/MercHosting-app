import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { cleanDomain, srvTarget, validDomain } from "./components/DomainHelper";
import { pickShareAddress } from "./shareAddress";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

function setup(f: FakeApi) {
  f.servers.push(makeSummary({ kind: "paper" }));
  f.servers[0].status = { ...f.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
  f.playit = { ...f.playit, enabled: true, installed: true, running: true };
  f.approveClaim("dark-sea.joinmc.link");
}

beforeEach(() => localStorage.clear());

describe("Usar mi dominio", () => {
  it("valida y limpia el dominio", () => {
    expect(cleanDomain(" https://Play.MiDominio.com/inicio ")).toBe("play.midominio.com");
    expect(cleanDomain("play.midominio.com:25565")).toBe("play.midominio.com");
    expect(validDomain("play.midominio.com")).toBe(true);
    expect(validDomain("midominio")).toBe(false);
    expect(validDomain("mal_nombre.com")).toBe(false);
  });

  it("el destino sale de la prueba real o de la dirección con puerto", () => {
    expect(srvTarget("dark-sea.joinmc.link", undefined)).toBeNull();
    expect(srvTarget("a.gl.at.ply.gg:41234", undefined)).toEqual({ host: "a.gl.at.ply.gg", port: 41234 });
    const t = { address: "x", at_unix_ms: 0, error: null, result: { tested: "b.gl.at.ply.gg:5000", latency_ms: 1, version: "", players_online: 0, players_max: 0, motd: "" } };
    expect(srvTarget("dark-sea.joinmc.link", t)).toEqual({ host: "b.gl.at.ply.gg", port: 5000 });
  });

  it("arma el registro SRV tras probar el túnel y prueba el dominio de verdad", async () => {
    const fake = new FakeApi();
    setup(fake);
    fake.reachable.add("dark-sea.joinmc.link");
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    await user.click(await screen.findByRole("tab", { name: "Conexión" }));
    const panel = within(screen.getByRole("tabpanel", { name: "Conexión" }));
    await user.click(await panel.findByRole("button", { name: /Usar mi dominio/ }));
    const box = within(panel.getByLabelText("Usar mi dominio"));
    expect(box.getByText(/Primero prueba la dirección de Playit/)).toBeInTheDocument();

    await user.click(panel.getByRole("button", { name: "Probar dark-sea.joinmc.link" }));
    await user.type(box.getByLabelText("Tu dominio o subdominio"), "play.midominio.com");
    expect(await box.findByText("_minecraft._tcp.play.midominio.com")).toBeInTheDocument();
    expect(box.getByText("dark-sea.gl.at.ply.gg")).toBeInTheDocument();
    expect(box.getByText("41234")).toBeInTheDocument();

    // Todavía no existe el registro: no responde.
    await user.click(box.getByRole("button", { name: "Probar mi dominio" }));
    expect(fake.calls).toContainEqual({ op: "test_address", id, address: "play.midominio.com" });
    expect(await box.findByText(/Todavía no responde/)).toBeInTheDocument();

    // Responde pero va a otro lado.
    fake.reachable.add("play.midominio.com");
    fake.testedAs["play.midominio.com"] = "otro.host.com:25565";
    await user.click(box.getByRole("button", { name: "Probar mi dominio" }));
    expect(await box.findByText(/lleva a/)).toBeInTheDocument();

    // Bien hecho.
    delete fake.testedAs["play.midominio.com"];
    await user.click(box.getByRole("button", { name: "Probar mi dominio" }));
    expect(await box.findByText(/Ya puedes compartirlo/)).toBeInTheDocument();
    expect(localStorage.getItem("merchosting.customDomain")).toContain("play.midominio.com");
  }, 20_000);
});

describe("Dirección para compartir en el Resumen", () => {
  it("elige el dominio probado, si no el túnel, y marca si falta probar", () => {
    const tunnel = { display_address: "dark-sea.joinmc.link", destination: "127.0.0.1:25565", is_disabled: false, disabled_reason: null };
    const ok = (address: string) => ({ address, at_unix_ms: 0, error: null, result: { tested: "h:1", latency_ms: 1, version: "", players_online: 0, players_max: 0, motd: "" } });
    const base = { port: 25565, lan: [], tunnels: [tunnel], tests: [], ports_in_use: [], voice: null, layers: [], bedrock: null };
    expect(pickShareAddress(base, null)).toEqual({ address: "dark-sea.joinmc.link", tested: false, domain: false });
    expect(pickShareAddress({ ...base, tests: [ok("dark-sea.joinmc.link")] }, null)?.tested).toBe(true);
    expect(pickShareAddress({ ...base, tests: [ok("play.x.com")] }, "play.x.com")).toEqual({ address: "play.x.com", tested: true, domain: true });
    expect(pickShareAddress({ ...base, tunnels: [] }, null)).toBeNull();
  });

  it("el Resumen muestra la dirección de Internet y lleva a Conexión si falta probarla", async () => {
    const fake = new FakeApi();
    setup(fake);
    const user = userEvent.setup();
    render(<App api={fake} intro={false} />);
    const pending = await screen.findByRole("button", { name: /Pruébala en Conexión para compartirla/ });
    await user.click(pending);
    expect(await screen.findByRole("tabpanel", { name: "Conexión" })).toBeInTheDocument();

    fake.reachable.add("dark-sea.joinmc.link");
    const panel = within(screen.getByRole("tabpanel", { name: "Conexión" }));
    await user.click(await panel.findByRole("button", { name: "Probar dark-sea.joinmc.link" }));
    await panel.findByText(/Funciona/);
    await user.click(screen.getByRole("tab", { name: "Resumen" }));
    expect(await screen.findByRole("button", { name: "Copiar Para tus amigos (Internet): dark-sea.joinmc.link" })).toBeInTheDocument();
  }, 20_000);
});
