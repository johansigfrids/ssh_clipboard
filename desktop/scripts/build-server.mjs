// Stage a raw signed Linux server package. Never installs, restarts, tags or publishes.
import {spawnSync} from "node:child_process";
import {createHash,randomUUID} from "node:crypto";
import {copyFileSync,readFileSync,writeFileSync,mkdirSync,statSync,existsSync} from "node:fs";
import path from "node:path";
import {fileURLToPath} from "node:url";

const root=fileURLToPath(new URL("../../",import.meta.url));
const cli=path.join(root,"desktop/node_modules/@tauri-apps/cli/tauri.js");
const values=new Map(), flags=new Set();
for(let i=2;i<process.argv.length;i++) {
  const argument=process.argv[i];
  if(["--debug","--test-build"].includes(argument)){flags.add(argument);continue;}
  if(!["--version","--public-key-file"].includes(argument)||!process.argv[i+1]||values.has(argument))throw new Error("Unknown, duplicate or incomplete server-build argument.");
  values.set(argument,process.argv[++i]);
}
const version=values.get("--version");
const core=readFileSync(path.join(root,"Cargo.toml"),"utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if(!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(version??"")||version!==core||version==="0.0.0")throw new Error("Pass a canonical stable version matching the shared core.");
if(flags.has("--debug")&&!flags.has("--test-build"))throw new Error("Debug server packages require --test-build.");
if(!values.has("--public-key-file"))throw new Error("An independently trusted public-key file is required.");
const publicPath=path.resolve(values.get("--public-key-file"));
if(!statSync(publicPath).isFile()||statSync(publicPath).size>4096)throw new Error("Public key must be a bounded regular file.");
const publicKey=readFileSync(publicPath,"utf8").trim();
if(!publicKey||!process.env.TAURI_SIGNING_PRIVATE_KEY||process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD===undefined)throw new Error("Protected signing key/password inputs are missing.");
const signingEnv={...process.env};
if(existsSync(signingEnv.TAURI_SIGNING_PRIVATE_KEY)) {
  const keyPath=path.resolve(signingEnv.TAURI_SIGNING_PRIVATE_KEY);
  if(!flags.has("--test-build")&&keyPath.startsWith(path.resolve(root)+path.sep))throw new Error("Keep production private signing keys outside the checkout.");
  signingEnv.TAURI_SIGNING_PRIVATE_KEY_PATH=keyPath;delete signingEnv.TAURI_SIGNING_PRIVATE_KEY;
} else {delete signingEnv.TAURI_SIGNING_PRIVATE_KEY_PATH;}
const buildEnv={...process.env};
for(const key of ["TAURI_SIGNING_PRIVATE_KEY","TAURI_SIGNING_PRIVATE_KEY_PATH","TAURI_SIGNING_PRIVATE_KEY_PASSWORD"])delete buildEnv[key];
function run(command,args,env=buildEnv,{quiet=false}={}) {
  const result=spawnSync(command,args,{cwd:root,env,encoding:"utf8",stdio:"pipe",maxBuffer:8*1024*1024});
  if(result.status!==0) {
    if(!quiet&&result.stderr)process.stderr.write(result.stderr);
    throw new Error("Server package step failed; nothing was published. Signing output is withheld.");
  }
  return result.stdout;
}
const windows=process.platform==="win32";
if(!windows&&process.platform!=="linux")throw new Error("Build natively on Linux, or with the existing Ubuntu WSL toolchain on Windows.");
function linuxPath(file) {
  return windows?path.resolve(file).replaceAll("\\","/").replace(/^([A-Za-z]):/,(_,drive)=>`/mnt/${drive.toLowerCase()}`):path.resolve(file);
}
const linuxHome=windows?run("wsl.exe",["-d","Ubuntu","--exec","/usr/bin/printenv","HOME"]).trim():null;
function linux(tool,args) {
  if(!windows)return run(tool,args);
  return run("wsl.exe",["-d","Ubuntu","--cd",linuxPath(root),"--exec",
    tool==="cargo"||tool==="rustc"?`${linuxHome}/.cargo/bin/${tool}`:tool,...args]);
}
const source=run("git",["rev-parse","HEAD"]).trim();
if(process.env.GITHUB_SHA&&source!==process.env.GITHUB_SHA)throw new Error("Release source differs from the reviewed commit.");
if(!flags.has("--test-build")&&run("git",["status","--porcelain"]).trim())throw new Error("Production server staging requires a clean checkout.");
const host=linux("rustc",["-vV"]).match(/^host:\s*(\S+)/m)?.[1];
const platform={"x86_64-unknown-linux-gnu":"linux-x86_64","aarch64-unknown-linux-gnu":"linux-aarch64"}[host];
if(!platform)throw new Error("Unsupported native Linux server target.");
const target=path.join(root,"target",windows?"linux-wsl":"server-package");
const profile=flags.has("--debug")?"debug":"release";
process.stdout.write(`Building headless ${platform} server ${version}...\n`);
linux("cargo",["build","--locked","--bin","ssh_clipboard","--target-dir",linuxPath(target),...(!flags.has("--debug")?["--release"]:[])]);
const binary=path.join(target,profile,"ssh_clipboard");
if(linux(linuxPath(binary),["--version"]).trim()!==`ssh_clipboard ${version}`)throw new Error("Server binary/core version mismatch.");
const stage=path.join(root,"desktop/artifacts",`${flags.has("--test-build")?"TEST-only-":""}server-${version}-${randomUUID()}`);
mkdirSync(stage,{recursive:true});
const name=`SSHClipboardServer-stable-${platform}-${version}`;
const output=path.join(stage,name);
const verifyKey=path.join(stage,"signing-public-key.pub");
writeFileSync(verifyKey,publicKey+"\n");
copyFileSync(binary,output);
const size=statSync(output).size;
if(size===0||size>256*1024*1024)throw new Error("Server package exceeds its policy limit.");
run(process.execPath,[cli,"signer","sign","--app-version",version,output],signingEnv,{quiet:true});
linux(linuxPath(binary),["server-maintenance","verify","--version",version,
  "--public-key",linuxPath(verifyKey),"--signature",linuxPath(`${output}.sig`),"--package",linuxPath(output)]);
const sha256=createHash("sha256").update(readFileSync(output)).digest("hex");
writeFileSync(`${output}.sha256`,`${sha256}  ${name}\n`);
writeFileSync(path.join(stage,"candidate.json"),JSON.stringify({version,platform,
  url:`https://github.com/johansigfrids/ssh_clipboard/releases/download/server-v${version}/${name}`,
  signature:readFileSync(`${output}.sig`,"utf8").trim()},null,2)+"\n");
writeFileSync(path.join(stage,"build-evidence.json"),JSON.stringify({version,platform,source,profile,
  publicKeyId:createHash("sha256").update(publicKey).digest("hex").slice(0,16),bytes:size,sha256,
  testOnly:flags.has("--test-build"),installed:false,published:false},null,2)+"\n");
process.stdout.write(`Staged verified ${flags.has("--test-build")?"TEST-ONLY ":""}server package: ${stage}\n`);
