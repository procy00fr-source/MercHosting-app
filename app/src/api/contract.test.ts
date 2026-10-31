// Comprueba que los tipos TypeScript coinciden con los ejemplos generados por Rust
// (docs/contratos/ipc-v1-ejemplos.json, prueba crates/merc-ipc/tests/contract.rs).

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { ERROR_CODES, FORCE_KILL_CONFIRMATION, PROTOCOL_VERSION, RUNTIME_STATES } from "./types";
import type { RequestBody, ResponseBody } from "./types";
import { KIND_LABEL, STATE_LABEL, STATE_TONE } from "../format";

// vitest se ejecuta desde app/; el contrato está en docs/ del repositorio.
const path = resolve(process.cwd(), "../docs/contratos/ipc-v1-ejemplos.json");
const contract = JSON.parse(readFileSync(path, "utf8"));

const REQUEST_KEYS: Record<RequestBody["op"], string[]> = {
  ping: [],
  detect: ["path"],
  find_java: [],
  import_instance: ["name", "path", "java", "min_memory_mb", "max_memory_mb"],
  list_instances: [],
  status: ["id"],
  start: ["id"],
  stop: ["id", "timeout_secs"],
  restart: ["id", "stop_timeout_secs"],
  force_kill: ["id", "confirm"],
  send_command: ["id", "command"],
  console: ["id", "after", "limit"],
  accept_eula: ["id", "accepted"],
  unregister: ["id", "delete_files"],
  metrics: ["id"],
  system_info: [],
  catalog: ["flavor"],
  resolve: ["flavor", "mc_version", "loader_version"],
  modpack_search: ["source", "query", "sort", "offset"],
  modpack_project: ["source", "project"],
  modpack_inspect: ["source", "version_id"],
  creation_defaults: [],
  check_target: ["path"],
  create_server: ["name", "flavor", "mc_version", "path", "java", "min_memory_mb", "max_memory_mb", "properties", "accept_eula", "modpack_version", "modpack_source", "loader_version", "game_rules"],
  job: ["job_id", "after"],
  cancel_job: ["job_id"],
  properties: ["id"],
  set_properties: ["id", "changes", "expected_modified"],
  set_resource_pack: ["id", "url", "required"],
  players: ["id"],
  player_action: ["id", "action", "name", "reason"],
  list_files: ["id", "path"],
  read_file: ["id", "path"],
  write_file: ["id", "path", "content", "expected_modified"],
  make_dir: ["id", "path"],
  rename_file: ["id", "path", "new_name"],
  modpack_status: ["id"],
  modpack_update_preview: ["id", "version_id"],
  modpack_update_apply: ["id", "version_id", "accept_loader_change"],
  modpack_link: ["id", "source", "version_id"],
  modpack_unlink: ["id"],
  resource_pack_search: ["id", "query", "sort", "offset"],
  resource_pack_project: ["id", "project"],
  move_file: ["id", "path", "dest_dir"],
  copy_file: ["id", "path", "dest_dir"],
  extract_zip: ["id", "path"],
  trash_file: ["id", "path"],
  upload_file: ["id", "source", "dest_dir"],
  download_file: ["id", "path", "dest"],
  export_console: ["id", "dest"],
  worlds: ["id"],
  duplicate_world: ["id", "name", "new_name"],
  export_world: ["id", "name", "dest"],
  import_world: ["id", "source", "new_name"],
  set_active_world: ["id", "name"],
  trash_world: ["id", "name"],
  saved_items: ["id"],
  restore_saved: ["id", "item_id"],
  purge_saved: ["id", "item_id"],
  game_rules: ["id"],
  set_game_rule: ["id", "key", "value"],
  activity: ["id", "limit"],
  restart_with_notice: ["id", "seconds"],
  admin_state: ["id"],
  set_maintenance: ["id", "on"],
  cancel_restart: ["id"],
  cancel_world_op: ["id"],
  backups: ["id"],
  create_backup: ["id", "scope", "note"],
  verify_backup: ["id", "backup"],
  restore_backup: ["id", "backup"],
  delete_backup: ["id", "backup"],
  protect_backup: ["id", "backup", "protected"],
  set_backup_settings: ["id", "keep"],
  tasks: ["id"],
  health: ["id"],
  playit_status: [],
  playit_enable: [],
  playit_disable: [],
  playit_link: [],
  playit_unlink: [],
  playit_login_url: [],
  connection: ["id"],
  test_address: ["id", "address"],
  set_voice_config: ["id", "voice_host", "port"],
  install_java: ["major"],
  diagnose: ["id"],
  server_update_check: ["id"],
  apply_server_update: ["id", "loader_version"],
  loader_versions: ["flavor", "mc_version"],
  clone_server: ["id", "name"],
  set_archived: ["id", "archived"],
  player_command: ["id", "name", "command"],
  player_history: ["id"],
  set_player_note: ["id", "name", "note"],
  clear_player_history: ["id"],
  mod_sides: ["id"],
  quarantine_mods: ["id", "files"],
  restore_mods: ["id", "files"],
  friend_pack: ["id"],
  export_friend_pack: ["id", "dest"],
  luck_perms: ["id"],
  luck_perms_preview: ["id", "change"],
  luck_perms_apply: ["id", "change"],
  automation: ["id"],
  set_automation: ["id", "auto_restart", "autostart"],
  set_startup: ["enabled"],
  supporter: [],
  activate_supporter: ["license_key"],
  refresh_supporter: [],
  forget_supporter: [],
  server_icon: ["id"],
  preview_server_icon: ["source", "fit"],
  set_server_icon: ["id", "source", "fit"],
  remove_server_icon: ["id"],
  security_report: ["id"],
  set_launch: ["id", "java", "min_memory_mb", "max_memory_mb"],
  companions: [],
  set_companions_enabled: ["enabled"],
  create_invite: ["id", "perms", "minutes"],
  cancel_invite: ["invite_id"],
  set_companion_perms: ["companion", "id", "perms"],
  remove_companion: ["companion"],
  join_with_code: ["code", "name"],
  remote_hosts: [],
  forget_host: ["host"],
  refresh_host: ["host"],
  remote_call: ["host", "request"],
  remote_download: ["host", "item", "dest"],
  remote_upload: ["host", "id", "source", "dest_dir"],
  download_backup: ["id", "backup_id", "dest"],
  measure_tps: ["id"],
  alerts: [],
  mark_alerts_read: ["ids"],
  set_alert_settings: ["settings"],
  save_task: ["id", "task"],
  delete_task: ["id", "task_id"],
  run_task_now: ["id", "task_id"],
  content_platforms: ["id"],
  content_search: ["id", "kind", "source", "query", "sort", "offset"],
  content_project: ["id", "kind", "source", "project"],
  content_plan: ["id", "kind", "source", "version_id"],
  content_install: ["id", "kind", "source", "version_id"],
  content_installed: ["id", "kind"],
  content_toggle: ["id", "kind", "file_name", "enabled"],
  content_remove: ["id", "kind", "file_name"],
  shutdown: [],
};

const RESPONSE_KINDS: ResponseBody["kind"][] = [
  "pong",
  "detected",
  "java_candidates",
  "instance",
  "instances",
  "status",
  "console",
  "metrics",
  "system",
  "catalog",
  "resolved",
  "creation_defaults",
  "job",
  "properties",
  "players",
  "files",
  "text_file",
  "file_done",
  "worlds",
  "saved_items",
  "game_rules",
  "activity",
  "admin_state",
  "content_platforms",
  "backups",
  "tasks",
  "health",
  "playit",
  "playit_login_url",
  "connection",
  "companions",
  "diagnosis",
  "server_update",
  "server_icon",
  "supporter",
  "loader_versions",
  "automation",
  "player_history",
  "mod_sides",
  "zip_extracted",
  "modpack_status",
  "modpack_update_plan",
  "friend_pack",
  "luck_perms",
  "luck_perms_preview",
  "luck_perms_applied",
  "icon_preview",
  "security_report",
  "invite",
  "remote_hosts",
  "tps",
  "alerts",
  "content_search",
  "content_details",
  "content_plan",
  "content_list",
  "modpack_summary",
  "ok",
];

const INSTANCE_KEYS = ["id", "name", "root", "kind", "mc_version", "loader_version", "launch", "imported", "created_at_unix"];
const STATUS_KEYS = [
  "state",
  "has_console",
  "pid",
  "port",
  "started_at_unix_ms",
  "online_at_unix_ms",
  "last_exit",
  "hint",
  "log_file",
];

const sorted = (a: string[]) => [...a].sort();
const FLAVORS = ["vanilla", "paper", "purpur", "spigot", "fabric", "quilt", "forge", "neo_forge", "arclight"];

describe("contrato del protocolo v1 con Rust", () => {
  it("usa la misma versión y textos fijos", () => {
    expect(contract.protocol).toBe(PROTOCOL_VERSION);
    expect(contract.force_kill_confirmation).toBe(FORCE_KILL_CONFIRMATION);
  });

  it("conoce todos los estados y códigos de error, con texto para cada estado", () => {
    expect(contract.all_states).toEqual([...RUNTIME_STATES]);
    expect(contract.all_error_codes).toEqual([...ERROR_CODES]);
    for (const s of RUNTIME_STATES) {
      expect(STATE_LABEL[s]).toBeTruthy();
      expect(STATE_TONE[s]).toBeTruthy();
    }
  });

  it("las peticiones tienen exactamente los campos que espera Rust", () => {
    const ops = new Set<string>();
    for (const r of contract.requests) {
      const op = r.op as RequestBody["op"];
      ops.add(op);
      expect(REQUEST_KEYS[op], `operación desconocida en TS: ${op}`).toBeDefined();
      expect(sorted(Object.keys(r).filter((k) => k !== "op"))).toEqual(sorted(REQUEST_KEYS[op]));
    }
    expect(sorted([...ops])).toEqual(sorted(Object.keys(REQUEST_KEYS)));
  });

  it("las respuestas usan los mismos tipos y campos", () => {
    const kinds = new Set<string>();
    for (const r of contract.responses) {
      kinds.add(r.kind);
      expect(RESPONSE_KINDS).toContain(r.kind);
      if (r.kind === "instances") {
        for (const s of r.data) {
          expect(sorted(Object.keys(s))).toEqual(sorted(["instance", "status", "eula_accepted", "configured_port", "archived"]));
          expect(sorted(Object.keys(s.instance))).toEqual(sorted(INSTANCE_KEYS));
          expect(sorted(Object.keys(s.status))).toEqual(sorted(STATUS_KEYS));
          expect(KIND_LABEL[s.instance.kind as keyof typeof KIND_LABEL]).toBeTruthy();
          expect(["jar", "args_file"]).toContain(s.instance.launch.target.type);
        }
      }
      if (r.kind === "status") expect(sorted(Object.keys(r.data))).toEqual(sorted(STATUS_KEYS));
      if (r.kind === "console") {
        expect(sorted(Object.keys(r.data))).toEqual(["lines", "next_after"]);
        for (const l of r.data.lines) {
          expect(sorted(Object.keys(l))).toEqual(["at_unix_ms", "seq", "stream", "text"]);
          expect(["stdout", "stderr", "system"]).toContain(l.stream);
        }
      }
      if (r.kind === "metrics") {
        expect(sorted(Object.keys(r.data))).toEqual(["max_players", "players", "samples"]);
        for (const m of r.data.samples) expect(sorted(Object.keys(m))).toEqual(["at_unix_ms", "cpu_percent", "memory_bytes"]);
      }
      if (r.kind === "java_candidates")
        for (const j of r.data) expect(sorted(Object.keys(j))).toEqual(["description", "error", "jdk", "major", "path"]);
      if (r.kind === "catalog") for (const c of r.data) expect(sorted(Object.keys(c))).toEqual(["java_major", "latest", "mc_version"]);
      if (r.kind === "resolved") {
        expect(sorted(Object.keys(r.data))).toEqual(
          sorted([
            "flavor",
            "mc_version",
            "loader_version",
            "java_major",
            "experimental",
            "artifact",
            "installer_args",
            "installer_in_cache",
            "needs_jdk",
            "notes",
          ]),
        );
        expect(FLAVORS).toContain(r.data.flavor);
        expect(["md5", "sha1", "sha256", "none"]).toContain(r.data.artifact.checksum.algorithm);
      }
      if (r.kind === "playit") {
        expect(sorted(Object.keys(r.data))).toEqual(
          sorted(["supported", "enabled", "installed", "agent_version", "running", "agent", "agent_error", "claim"]),
        );
        if (r.data.agent)
          expect(sorted(Object.keys(r.data.agent))).toEqual(
            sorted(["phase", "message", "version", "account_status", "tunnels", "pending", "notices"]),
          );
        if (r.data.claim) expect(sorted(Object.keys(r.data.claim))).toEqual(["done", "error", "state", "url"]);
      }
      if (r.kind === "connection") {
        expect(sorted(Object.keys(r.data))).toEqual(["lan", "layers", "port", "ports_in_use", "tests", "tunnels", "voice"]);
        for (const l of r.data.layers) {
          expect(sorted(Object.keys(l))).toEqual(["checked_at_unix_ms", "detail", "key", "state"]);
          expect(["operativo", "degradado", "caido", "desconocido"]).toContain(l.state);
        }
        if (r.data.voice) {
          expect(sorted(Object.keys(r.data.voice))).toEqual(
            sorted(["installed", "enabled", "jar", "config", "port_setting", "port", "voice_host", "bind_address", "tunnels", "suggested_port", "listening", "restart_needed", "problems"]),
          );
          for (const p of r.data.voice.problems) expect(sorted(Object.keys(p))).toEqual(["code", "level", "message"]);
        }
        for (const t of r.data.tunnels) expect(sorted(Object.keys(t))).toEqual(["destination", "disabled_reason", "display_address", "is_disabled"]);
        for (const t of r.data.tests) {
          expect(sorted(Object.keys(t))).toEqual(["address", "at_unix_ms", "error", "result"]);
          if (t.result)
            expect(sorted(Object.keys(t.result))).toEqual(["latency_ms", "motd", "players_max", "players_online", "tested", "version"]);
        }
      }
      if (r.kind === "security_report") {
        expect(sorted(Object.keys(r.data))).toEqual(["checked_at_unix_ms", "checks", "public_addresses"]);
        for (const c of r.data.checks) {
          expect(sorted(Object.keys(c))).toEqual(["action", "code", "detail", "level", "title"]);
          expect(["peligro", "aviso", "info", "bien"]).toContain(c.level);
        }
      }
      if (r.kind === "diagnosis") {
        expect(sorted(Object.keys(r.data))).toEqual(sorted(["checked_at_unix_ms", "sources", "mods_scanned", "plugins_scanned", "findings"]));
        for (const f of r.data.findings) {
          expect(sorted(Object.keys(f))).toEqual(sorted(["code", "title", "confidence", "explanation", "evidence", "source", "actions"]));
          for (const a of f.actions)
            expect(["disable_content", "set_port", "set_memory", "choose_java", "search_content", "open_tab"]).toContain(a.type);
        }
      }
      if (r.kind === "companions") {
        expect(sorted(Object.keys(r.data))).toEqual(sorted(["enabled", "online", "fingerprint", "error", "invites", "companions"]));
        for (const c of r.data.companions)
          expect(sorted(Object.keys(c))).toEqual(
            sorted(["companion", "name", "fingerprint", "added_at_unix_ms", "last_seen_unix_ms", "connected", "servers"]),
          );
      }
      if (r.kind === "remote_hosts") {
        for (const h of r.data)
          expect(sorted(Object.keys(h))).toEqual(sorted(["host", "name", "fingerprint", "servers", "last_ok_unix_ms", "last_error"]));
      }
      if (r.kind === "health") {
        expect(sorted(Object.keys(r.data))).toEqual(
          sorted(["pc_cpu_percent", "pc_memory_used_bytes", "pc_memory_total_bytes", "disks", "max_memory_mb", "tps_command"]),
        );
        for (const d of r.data.disks) expect(sorted(Object.keys(d))).toEqual(["free_bytes", "path", "total_bytes", "what"]);
      }
      if (r.kind === "tps") {
        expect(sorted(Object.keys(r.data))).toEqual(["command", "mspt", "raw", "tps"]);
        for (const t of r.data.tps) expect(typeof t[0] === "string" && typeof t[1] === "number").toBe(true);
      }
      if (r.kind === "alerts") {
        expect(sorted(Object.keys(r.data))).toEqual(["items", "settings", "unread"]);
        expect(sorted(Object.keys(r.data.settings))).toEqual(["backups", "crash", "disk", "disk_min_gb", "notify", "resources", "tasks", "tunnel", "updates"]);
        for (const a of r.data.items)
          expect(sorted(Object.keys(a))).toEqual(
            sorted(["id", "at_unix_ms", "level", "kind", "instance_id", "server_name", "title", "message", "read"]),
          );
      }
      if (r.kind === "tasks") {
        expect(sorted(Object.keys(r.data))).toEqual(["items", "now_unix_ms", "utc_offset_minutes"]);
        for (const t of r.data.items) {
          expect(sorted(Object.keys(t))).toEqual(
            sorted(["id", "enabled", "action", "schedule", "created_unix_ms", "next_run_unix_ms", "last_run", "running"]),
          );
          expect(["backup", "restart", "stop", "start", "command", "announce"]).toContain(t.action.type);
          expect(["daily", "every"]).toContain(t.schedule.type);
          if (t.last_run) expect(sorted(Object.keys(t.last_run))).toEqual(["at_unix_ms", "manual", "message", "outcome"]);
        }
      }
      if (r.kind === "backups") {
        expect(sorted(Object.keys(r.data))).toEqual(["folder", "free_bytes", "items", "keep", "restored"]);
        for (const b of r.data.items) {
          expect(sorted(Object.keys(b))).toEqual(
            sorted(["id", "created_unix_ms", "scope", "reason", "world", "mc_version", "size_bytes", "content_bytes", "files", "hot", "consistent", "verified", "verified_unix_ms", "problems", "protected", "note"]),
          );
          expect(["world", "full"]).toContain(b.scope);
          expect(["manual", "before_restore", "scheduled", "before_update"]).toContain(b.reason);
        }
      }
      if (r.kind === "admin_state") {
        expect(sorted(Object.keys(r.data))).toEqual(
          sorted(["maintenance", "whitelist_on", "enforce_whitelist", "restart_in_seconds", "world_op"]),
        );
        if (r.data.maintenance) expect(sorted(Object.keys(r.data.maintenance))).toEqual(["previous_whitelist", "since_unix"]);
        if (r.data.world_op) {
          expect(sorted(Object.keys(r.data.world_op))).toEqual(["done_bytes", "kind", "total_bytes", "world"]);
          expect(["duplicate", "export", "import", "install", "backup", "verify", "restore"]).toContain(r.data.world_op.kind);
        }
      }
      if (r.kind === "content_platforms")
        for (const p of r.data) expect(sorted(Object.keys(p))).toEqual(["experimental", "folder", "kind", "loaders", "sources"]);
      if (r.kind === "content_search")
        for (const h of r.data.hits)
          expect(sorted(Object.keys(h))).toEqual(
            sorted(["project_id", "slug", "title", "author", "description", "icon_url", "downloads", "categories", "client_side", "server_side", "date_modified", "license"]),
          );
      if (r.kind === "content_details") {
        expect(sorted(Object.keys(r.data.project))).toEqual(
          sorted(["id", "slug", "title", "description", "body", "icon_url", "client_side", "server_side", "license", "downloads", "categories", "source_url", "issues_url", "wiki_url", "page_url", "distribution_allowed"]),
        );
        for (const v of r.data.versions) {
          expect(sorted(Object.keys(v))).toEqual(
            sorted(["id", "project_id", "name", "version_number", "version_type", "game_versions", "loaders", "date_published", "downloads", "changelog", "dependencies", "files", "server_side"]),
          );
          for (const f of v.files) expect(sorted(Object.keys(f))).toEqual(["filename", "primary", "sha1", "sha256", "sha512", "size", "url"]);
        }
      }
      if (r.kind === "content_plan") {
        expect(sorted(Object.keys(r.data))).toEqual(sorted(["items", "already_installed", "optional", "warnings", "blockers", "download_bytes"]));
        for (const i of r.data.items)
          expect(sorted(Object.keys(i))).toEqual(
            sorted(["project_id", "title", "icon_url", "version_id", "version_number", "file_name", "url", "sha1", "sha256", "size", "reason", "replaces"]),
          );
      }
      if (r.kind === "content_list") {
        expect(sorted(Object.keys(r.data))).toEqual(["items", "offline_reason", "online", "platform"]);
        for (const i of r.data.items)
          expect(sorted(Object.keys(i))).toEqual(
            sorted(["file_name", "enabled", "size", "sha1", "project_id", "version_id", "title", "version_number", "icon_url", "server_side", "client_side", "page_url", "update", "source", "other_ids"]),
          );
      }
      if (r.kind === "modpack_summary")
        expect(sorted(Object.keys(r.data))).toEqual(
          sorted(["source", "flavor", "name", "pack_version", "mc_version", "loader", "loader_version", "java_major", "server_files", "client_only_files", "download_bytes", "warnings"]),
        );
      if (r.kind === "creation_defaults") expect(sorted(Object.keys(r.data))).toEqual(["curseforge", "servers_dir", "suggested_port"]);
      if (r.kind === "job") {
        expect(sorted(Object.keys(r.data))).toEqual(
          sorted(["job_id", "name", "state", "step", "message", "download", "log", "next_after", "instance_id", "error"]),
        );
        expect(["running", "succeeded", "failed", "cancelled"]).toContain(r.data.state);
        expect(["preparing", "downloading", "installing", "configuring", "finishing"]).toContain(r.data.step);
      }
      if (r.kind === "properties") {
        expect(sorted(Object.keys(r.data))).toEqual(["entries", "exists", "modified_unix_ms"]);
        for (const e of r.data.entries) expect(sorted(Object.keys(e))).toEqual(["key", "value"]);
      }
      if (r.kind === "players") {
        expect(sorted(Object.keys(r.data))).toEqual(["can_act", "lists", "online"]);
        expect(sorted(Object.keys(r.data.lists))).toEqual(["banned", "banned_ips", "ops", "warnings", "whitelist"]);
        for (const p of [...r.data.lists.whitelist, ...r.data.lists.ops, ...r.data.lists.banned])
          expect(sorted(Object.keys(p))).toEqual(["expires", "level", "name", "reason", "uuid"]);
      }
      if (r.kind === "files")
        for (const f of r.data.entries) expect(sorted(Object.keys(f))).toEqual(["is_dir", "link", "modified_unix_ms", "name", "size"]);
      if (r.kind === "text_file") expect(sorted(Object.keys(r.data))).toEqual(["content", "modified_unix_ms", "path", "size"]);
      if (r.kind === "file_done") expect(Object.keys(r.data)).toEqual(["path"]);
      if (r.kind === "worlds") {
        expect(sorted(Object.keys(r.data))).toEqual(["level_name", "worlds"]);
        for (const w of r.data.worlds)
          expect(sorted(Object.keys(w))).toEqual(["active", "companions", "modified_unix_ms", "name", "size_bytes"]);
      }
      if (r.kind === "saved_items")
        for (const i of r.data) {
          expect(sorted(Object.keys(i))).toEqual(["id", "is_dir", "kind", "path", "saved_unix_ms", "size_bytes"]);
          expect(["trash", "copy"]).toContain(i.kind);
        }
      if (r.kind === "game_rules") {
        expect(sorted(Object.keys(r.data))).toEqual(["new_names", "rules"]);
        for (const g of r.data.rules) {
          expect(sorted(Object.keys(g))).toEqual(["key", "kind", "name", "value"]);
          expect(["bool", "int"]).toContain(g.kind.type);
        }
      }
      if (r.kind === "activity")
        for (const a of r.data) expect(sorted(Object.keys(a))).toEqual(["action", "actor", "at_unix", "detail"]);
      if (r.kind === "system") expect(sorted(Object.keys(r.data))).toEqual(["lan_ipv4", "logical_cpus", "total_memory_bytes"]);
    }
    expect(sorted([...kinds])).toEqual(sorted(RESPONSE_KINDS));
  });

  it("los tipos de servidor que se piden existen en la interfaz", () => {
    for (const r of contract.requests.filter((x: { op: string }) => x.op === "catalog")) expect(FLAVORS).toContain(r.flavor);
  });

  it("las acciones de jugador son las mismas", () => {
    const actions = contract.requests.filter((r: { op: string }) => r.op === "player_action").map((r: { action: string }) => r.action);
    for (const a of actions) expect(["whitelist_add", "whitelist_remove", "op", "deop", "kick", "ban", "pardon"]).toContain(a);
  });

  it("los errores traen código y mensaje", () => {
    for (const e of contract.error_responses) {
      expect(ERROR_CODES).toContain(e.err.code);
      expect(typeof e.err.message).toBe("string");
    }
  });
});
