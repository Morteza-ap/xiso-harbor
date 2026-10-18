import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = realpathSync(fileURLToPath(new URL("../", import.meta.url)));
const env = { ...process.env };
const target = resolve(root, env.CARGO_TARGET_DIR ?? "src-tauri/target");
const separator = "\x1f";
const flags = env.CARGO_ENCODED_RUSTFLAGS
  ? env.CARGO_ENCODED_RUSTFLAGS.split(separator)
  : (env.RUSTFLAGS ?? "").split(/\s+/).filter(Boolean);

// Cargo's encoded form preserves spaces in local directory names. These paths
// are resolved at build time and never recorded in the repository.
for (const [directory, replacement] of [
  [homedir(), "/build/home"],
  [env.CARGO_HOME, "/build/cargo"],
  [env.RUSTUP_HOME, "/build/rustup"],
  [env.TMPDIR, "/build/tmp"],
  [target, "/build/target"],
  [root, "/build/xiso-harbor"],
]) {
  if (directory) flags.push(`--remap-path-prefix=${directory}=${replacement}`);
}
env.CARGO_ENCODED_RUSTFLAGS = flags.join(separator);
delete env.RUSTFLAGS;

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

run("npm", [
  "run",
  "tauri",
  "--",
  "build",
  "--target",
  "aarch64-apple-darwin",
  "--bundles",
  "app",
]);
run("python3", [
  "scripts/check-privacy.py",
  "--artifact",
  resolve(target, "aarch64-apple-darwin/release/bundle/macos/Xiso Harbor.app"),
]);
