// Build a package with a disposable TEST key and isolated test application identity.
import {spawnSync} from "node:child_process";
import {mkdtempSync,mkdirSync,readFileSync,readdirSync,rmSync} from "node:fs";
import {createHash} from "node:crypto";
import path from "node:path";
import {fileURLToPath} from "node:url";
const release=process.argv.length===3&&process.argv[2]==="--release";
if(process.argv.length!==2&&!release)throw new Error("Only --release is supported; all smoke packages use disposable trust and a test application identity.");
const desktop=fileURLToPath(new URL("../",import.meta.url));
const artifacts=path.join(desktop,"artifacts");mkdirSync(artifacts,{recursive:true});
const temporary=mkdtempSync(path.join(artifacts,"synthetic-installer-key-"));
const cli=path.join(desktop,"node_modules/@tauri-apps/cli/tauri.js");
try {
  const key=path.join(temporary,"TEST-ONLY.key");
  const generated=spawnSync(process.execPath,[cli,"signer","generate","--ci","--password","","--write-keys",key],{cwd:desktop,stdio:"pipe"});
  if(generated.status!==0)throw new Error("Could not generate the disposable test signing key; output withheld.");
  const version=readFileSync(path.join(desktop,"../Cargo.toml"),"utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const before=new Set(readdirSync(artifacts));
  const built=spawnSync(process.execPath,[path.join(desktop,"scripts/build-installed.mjs"),...(!release?["--debug"]:[]),"--test-build","--version",version,"--public-key-file",`${key}.pub`],{cwd:desktop,stdio:"inherit",env:{...process.env,TAURI_SIGNING_PRIVATE_KEY:key,TAURI_SIGNING_PRIVATE_KEY_PASSWORD:""}});
  if(built.status!==0)throw new Error("Test-only package build failed; no installer was run.");
  const stages=readdirSync(artifacts).filter(name=>name.startsWith(`TEST-only-installed-${version}-`)&&!before.has(name));
  if(stages.length!==1)throw new Error("Expected one new test package stage.");
  const stage=path.join(artifacts,stages[0]);
  const evidence=JSON.parse(readFileSync(path.join(stage,"build-evidence.json"),"utf8"));
  const source=spawnSync("git",["rev-parse","HEAD"],{cwd:desktop,encoding:"utf8"});
  if(source.status!==0||evidence.source!==source.stdout.trim()||evidence.profile!==(release?"release":"debug")||evidence.testOnly!==true||typeof evidence.sourceDirty!=="boolean"||evidence.installed!==false||evidence.published!==false)throw new Error("Test package lacks accurate source/build evidence.");
  const suffix=evidence.platform.startsWith("windows")?"-setup.exe":evidence.platform.startsWith("darwin")?".app.tar.gz":".AppImage";
  const bytes=readFileSync(path.join(stage,`SSHClipboard-stable-${evidence.platform}-${version}${suffix}`));
  if(evidence.bytes!==bytes.length||evidence.sha256!==createHash("sha256").update(bytes).digest("hex"))throw new Error("Test package differs from recorded size/hash.");
  if(process.platform==="darwin") {
    const inspection=JSON.parse(readFileSync(path.join(stage,"macos-package-inspection.json"),"utf8"));
    if(inspection.source!==evidence.source||inspection.version!==version||inspection.platform!==evidence.platform||inspection.testOnly!==true||inspection.archiveInspected!==true||inspection.codeSignaturesVerified!==true||inspection.cliVersionVerified!==true||inspection.installed!==false)throw new Error("Mac package lacks native archive/signature inspection evidence.");
  }
  process.stdout.write(`PASS: synthetic installer evidence matches source, ${evidence.profile}/test identity and exact package bytes; nothing installed.\n`);
} finally {
  const resolved=path.resolve(temporary);
  if(path.dirname(resolved)!==path.resolve(artifacts) || !path.basename(resolved).startsWith("synthetic-installer-key-"))throw new Error("Refusing unexpected test-key cleanup path.");
  rmSync(resolved,{recursive:true,force:true});
}
