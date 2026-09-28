#!/usr/bin/env node
/**
 * Dead-command fitness check (rounds 16–18 methodology, mechanized).
 *
 * Every Tauri command registered in app_builder.rs must have at least one
 * caller: a TS invoke (apps/desktop/src) or a Rust-internal reference
 * (src-tauri/src, excluding the definition and the registration itself).
 * A registered command with zero callers is an IPC entry point with no
 * function — pure attack surface.
 *
 * Exceptions live in ALLOWLIST with a reason. An unreasoned exception fails
 * the check; the allowlist IS the documentation.
 *
 * Exit 1 on any unlisted dead command.
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const appBuilder = join(root, "apps/desktop/src-tauri/src/bootstrap/app_builder.rs");
const rustSrc = join(root, "apps/desktop/src-tauri/src");
const tsSrc = join(root, "apps/desktop/src");

const ALLOWLIST = new Map([
  ["health_check", "diagnostic endpoint: verifies IPC boundary + DB liveness"],
  ["move_folder", "product gap: folder reparenting UI not wired (STATUS round 12)"],
]);

function walk(dir, exts, out = []) {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    const st = statSync(p);
    if (st.isDirectory()) {
      if (entry === "node_modules" || entry === "dist" || entry === "target") continue;
      walk(p, exts, out);
    } else if (exts.some((e) => p.endsWith(e))) {
      out.push(p);
    }
  }
  return out;
}

const registered = [
  ...readFileSync(appBuilder, "utf8").matchAll(/commands::\w+::(\w+)\s*,/g),
].map((m) => m[1]);

if (registered.length === 0) {
  console.error("check-dead-commands: no registered commands parsed — parser broken?");
  process.exit(1);
}

const tsFiles = walk(tsSrc, [".ts", ".tsx"]).map((f) => readFileSync(f, "utf8"));
// Rust side: strip string literals first so tracing/log/format strings
// (e.g. event = "health_check") never count as callers — only real code
// references (fn paths, calls) do.
const rustFiles = walk(rustSrc, [".rs"]).map((f) =>
  readFileSync(f, "utf8").replace(/"[^"\n]*"/g, '""'),
);

const dead = [];
for (const name of registered) {
  const tsCaller = tsFiles.some((c) => c.includes(`"${name}"`));
  const rustCaller = rustFiles.some((c) =>
    new RegExp(`\\b${name}\\b`).test(c.replace(new RegExp(`pub async fn ${name}\\(`, "g"), "").replace(new RegExp(`commands::\\w+::${name}\\s*,`, "g"), "")),
  );
  if (!tsCaller && !rustCaller && !ALLOWLIST.has(name)) {
    dead.push(name);
  }
}

for (const [name, reason] of ALLOWLIST) {
  if (!registered.includes(name)) {
    console.error(`check-dead-commands: allowlisted "${name}" is no longer registered — remove it from the allowlist.`);
    process.exit(1);
  }
  console.log(`allowlisted: ${name} — ${reason}`);
}

if (dead.length > 0) {
  console.error(`check-dead-commands: ${dead.length} registered command(s) with zero callers:`);
  for (const name of dead) console.error(`  - ${name}`);
  console.error("Wire them up, delete them, or add them to ALLOWLIST with a reason.");
  process.exit(1);
}

console.log(`check-dead-commands: OK — ${registered.length} registered commands, all reachable or allowlisted.`);
