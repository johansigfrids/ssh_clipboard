import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { recordFrontend, prebuiltFrontendConfig } from "../scripts/frontend-proof.mjs";
import { desktopBuildEnv } from "../scripts/build-context.mjs";

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), "ssh-frontend-proof-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const directory of ["src", "tests/fixtures", "dist/assets"]) mkdirSync(path.join(root, directory), { recursive: true });
  for (const file of ["package.json", "package-lock.json", "tsconfig.json", "vite.config.ts", "index.html", "src/App.svelte", "tests/fixtures/ipc.ts", "dist/index.html", "dist/assets/app.js"]) writeFileSync(path.join(root, file), `fixture ${file}`);
  return root;
}

const reuse = { SSH_CLIPBOARD_PREBUILT_FRONTEND: "1" };

test("frontend reuse requires recorded inputs and exact output; normal local builds keep their hook", t => {
  const root = fixture(t);
  assert.deepEqual(prebuiltFrontendConfig(root, {}), {});
  assert.throws(() => prebuiltFrontendConfig(root, reuse), /requires proof/);
  recordFrontend(root, reuse);
  assert.deepEqual(prebuiltFrontendConfig(root, reuse), { build: { beforeBuildCommand: "" } });
  writeFileSync(path.join(root, "dist/assets/app.js"), "tampered generated code");
  assert.throws(() => prebuiltFrontendConfig(root, reuse), /output changed/);
});

test("source, configuration, dependency, fixture, environment and asset changes invalidate reuse", t => {
  const root = fixture(t);
  for (const file of ["src/App.svelte", "vite.config.ts", "tsconfig.json", "package-lock.json", "tests/fixtures/ipc.ts", ".env.production", "dist/assets/new.css"]) {
    recordFrontend(root, reuse);
    writeFileSync(path.join(root, file), `changed ${file}`);
    assert.throws(() => prebuiltFrontendConfig(root, reuse), /changed/, file);
  }
  recordFrontend(root, reuse);
  assert.throws(() => prebuiltFrontendConfig(root, { ...reuse, VITE_ENDPOINT: "changed" }), /changed/);
  rmSync(path.join(root, "dist/index.html"));
  assert.throws(() => prebuiltFrontendConfig(root, reuse), /Build the frontend/);
});

test("plain Cargo and Tauri use the declared Mac deployment target without leaking it to other hosts", () => {
  const config = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
  const original = { CARGO_TARGET_DIR: "isolated", MACOSX_DEPLOYMENT_TARGET: "different" };
  assert.equal(desktopBuildEnv(original, "darwin").MACOSX_DEPLOYMENT_TARGET, config.bundle.macOS.minimumSystemVersion);
  assert.equal(desktopBuildEnv(original, "darwin").CARGO_TARGET_DIR, "isolated");
  assert.deepEqual(desktopBuildEnv(original, "win32"), original);
  assert.equal(original.MACOSX_DEPLOYMENT_TARGET, "different");
});
