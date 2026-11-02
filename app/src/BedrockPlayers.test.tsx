import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import { validGamertag } from "./components/BedrockPlayersPanel";
import { FakeApi, makeSummary } from "./test/fakeApi";

const id = "11111111-1111-4111-8111-111111111111";

async function open(online: boolean, setup?: (f: FakeApi) => void) {
  const fake = new FakeApi();
  fake.servers.push(
    makeSummary({
      kind: "bedrock",
      mc_version: "1.21.124.2",
      loader_version: null,
      launch: { java: "", min_memory_mb: 0, max_memory_mb: 0, extra_jvm_args: [], target: { type: "native", exe: "bedrock_server.exe" } },
    }),
  );
  if (online) fake.servers[0].status = { ...fake.servers[0].status, state: "ONLINE", has_console: true, pid: 9 };
  setup?.(fake);
  const user = userEvent.setup();
  render(<App api={fake} intro={false} />);
  await user.click(await screen.findByRole("tab", { name: "Jugadores" }));
  const panel = within(await screen.findByRole("tabpanel", { name: "Jugadores" }));
  await panel.findByRole("region", { name: "Lista blanca" });
  return { fake, user, panel };
}

describe("Jugadores de un servidor Bedrock", () => {
  it("valida gamertags como el supervisor", () => {
    expect(validGamertag("Steve Craft")).toBe(true);
    expect(validGamertag("Gamer#1234")).toBe(true);
    expect(validGamertag('x"; op y')).toBe(false);
    expect(validGamertag("")).toBe(false);
    expect(validGamertag("x".repeat(33))).toBe(false);
  });

  it("lista blanca: activar, añadir con límite, cambiar y quitar", async () => {
    const { fake, user, panel } = await open(false);
    const card = within(panel.getByRole("region", { name: "Lista blanca" }));
    expect(panel.getByText(/valen al encender el servidor/)).toBeInTheDocument();
    expect(card.getByText("La lista está vacía.")).toBeInTheDocument();

    await user.click(card.getByRole("checkbox", { name: /Solo pueden entrar los de la lista/ }));
    expect(fake.calls).toContainEqual({ op: "bedrock_allowlist_enabled", id, enabled: true });

    const input = card.getByLabelText("Gamertag de Xbox");
    await user.type(input, 'x"; op');
    expect(card.getByRole("button", { name: "Añadir" })).toBeDisabled();
    await user.clear(input);
    await user.type(input, "Steve Craft");
    await user.click(card.getByRole("checkbox", { name: /aunque el servidor esté lleno/ }));
    await user.click(card.getByRole("button", { name: "Añadir" }));
    expect(fake.calls).toContainEqual({ op: "bedrock_allowlist", id, change: { type: "add", name: "Steve Craft", ignores_player_limit: true } });
    expect(await card.findByText("Steve Craft")).toBeInTheDocument();
    expect(card.getByText("todavía no ha entrado")).toBeInTheDocument();

    await user.click(card.getByRole("checkbox", { name: "Puede entrar aunque esté lleno: Steve Craft" }));
    expect(fake.calls).toContainEqual({ op: "bedrock_allowlist", id, change: { type: "set_ignores_limit", name: "Steve Craft", value: false } });
    await user.click(card.getByRole("button", { name: "Quitar de la lista: Steve Craft" }));
    await waitFor(() => expect(card.getByText("La lista está vacía.")).toBeInTheDocument());
  });

  it("permisos solo para XUIDs conocidos, sin pedirlos a mano", async () => {
    const { fake, user, panel } = await open(true, (f) => {
      f.bedrockPlayersState = {
        ...f.bedrockPlayersState,
        online: ["Steve Craft"],
        known: [{ name: "Steve Craft", xuid: "2535400000000001", online: true, last_seen_unix: 1_793_300_000 }],
        permissions: [{ xuid: "999", level: "visitor", name: null }],
        online_mode: false,
      };
    });
    expect(panel.getByText(/se aplican al momento/)).toBeInTheDocument();
    const now = within(panel.getByRole("region", { name: "Conectados ahora" }));
    expect(now.getByText("Steve Craft")).toBeInTheDocument();
    expect(panel.getByText(/online-mode está desactivado/)).toBeInTheDocument();
    const perms = within(panel.getByRole("region", { name: "Permisos" }));
    expect(perms.getByText(/pídele que entre una vez/)).toBeInTheDocument();
    await user.selectOptions(perms.getByRole("combobox", { name: "Permiso de Steve Craft" }), "operator");
    expect(fake.calls).toContainEqual({ op: "bedrock_permission", id, xuid: "2535400000000001", level: "operator" });
    await waitFor(() => expect(perms.getByRole("combobox", { name: "Permiso de Steve Craft" })).toHaveValue("operator"));
    // Un XUID que nunca se vio entrar se muestra tal cual y solo se puede quitar.
    expect(perms.getByText("999")).toBeInTheDocument();
    await user.click(perms.getByRole("button", { name: "Quitar permiso de 999" }));
    expect(fake.calls).toContainEqual({ op: "bedrock_permission", id, xuid: "999", level: null });
    await waitFor(() => expect(perms.queryByText("999")).not.toBeInTheDocument());
  });
});
