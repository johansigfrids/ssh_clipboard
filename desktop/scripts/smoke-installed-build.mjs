// Build a package with a disposable TEST key and isolated test application identity.
import {spawnSync} from "node:child_process";
import {mkdtempSync,mkdirSync,readFileSync,rmSync} from "node:fs";
import path from "node:path";
import {fileURLToPath} from "node:url";
const desktop=fileURLToPath(new URL("../",import.meta.url));
const artifacts=path.join(desktop,"artifacts");mkdirSync(artifacts,{recursive:true});
const temporary=mkdtempSync(path.join(artifacts,"synthetic-installer-key-"));
const cli=path.join(desktop,"node_modules/@tauri-apps/cli/tauri.js");
try {
  const key=path.join(temporary,"TEST-ONLY.key");
  const generated=spawnSync(process.execPath,[cli,"signer","generate","--ci","--password","","--write-keys",key],{cwd:desktop,stdio:"pipe"});
  if(generated.status!==0)throw new Error("Could not generate the disposable test signing key; output withheld.");
  const version=readFileSync(path.join(desktop,"../Cargo.toml"),"utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const built=spawnSync(process.execPath,[path.join(desktop,"scripts/build-installed.mjs"),"--debug","--test-build","--version",version,"--public-key-file",`${key}.pub`],{cwd:desktop,stdio:"inherit",env:{...process.env,TAURI_SIGNING_PRIVATE_KEY:key,TAURI_SIGNING_PRIVATE_KEY_PASSWORD:""}});
  if(built.status!==0)throw new Error("Test-only package build failed; no installer was run.");
} finally {
  const resolved=path.resolve(temporary);
  if(path.dirname(resolved)!==path.resolve(artifacts) || !path.basename(resolved).startsWith("synthetic-installer-key-"))throw new Error("Refusing unexpected test-key cleanup path.");
  rmSync(resolved,{recursive:true,force:true});
}
