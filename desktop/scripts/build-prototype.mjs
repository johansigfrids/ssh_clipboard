import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const desktop = fileURLToPath(new URL("../", import.meta.url));
const targetDir = path.join(desktop, "src-tauri", "target");
const env = { ...process.env, CARGO_TARGET_DIR: targetDir };

function run(command, args) {
  const result = spawnSync(command, args, { cwd: desktop, env, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// Debug optimization and development-server loading are separate choices.
run(process.execPath, [
  path.join(desktop, "node_modules", "@tauri-apps", "cli", "tauri.js"),
  "build", "--debug", "--no-bundle", "--features", "custom-protocol",
]);
run("cargo", [
  "test", "--locked", "--manifest-path", "src-tauri/Cargo.toml",
  "--features", "custom-protocol", "--test", "embedded_frontend",
]);

// Cargo integration tests can rebuild target/debug's executable in dev mode.
// Keep the user-facing copy outside that mutable build directory.
const extension = process.platform === "win32" ? ".exe" : "";
const outputDir = path.join(desktop, "artifacts");
// A timestamp lets a new build finish while a previous test copy is running.
const stamp = new Date().toISOString().replace(/[-:.]/g, "");
const output = path.join(outputDir, `SSHClipboardPrototype-${stamp}${extension}`);
mkdirSync(outputDir, { recursive: true });
copyFileSync(path.join(targetDir, "debug", `ssh_clipboard_desktop_prototype${extension}`), output);
console.log(`\nStandalone prototype: ${output}`);
