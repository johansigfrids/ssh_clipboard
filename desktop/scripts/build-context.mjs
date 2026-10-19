import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const config = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));

// Tauri CLI sets this itself. Plain Cargo must use the same deployment target
// or switching between the CLI and Cargo rebuilds the native dependency graph.
export function desktopBuildEnv(env = process.env, platform = process.platform) {
  return platform === "darwin"
    ? { ...env, MACOSX_DEPLOYMENT_TARGET: config.bundle.macOS.minimumSystemVersion }
    : { ...env };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 2 || process.platform !== "darwin") throw new Error("Run only on the Mac CI host.");
  console.log(`MACOSX_DEPLOYMENT_TARGET=${desktopBuildEnv().MACOSX_DEPLOYMENT_TARGET}`);
}
