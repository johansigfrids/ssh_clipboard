// Stage reviewable packages only; this helper never installs, tags, pushes, or publishes.
import {spawnSync} from "node:child_process";
import {createHash,randomUUID} from "node:crypto";
import {copyFileSync,readFileSync,writeFileSync,mkdirSync,readdirSync,statSync,existsSync} from "node:fs";
import path from "node:path";
import {fileURLToPath} from "node:url";
import {nativeOrigin} from "./native-update-config.mjs";
import {releaseSource} from "./release-source.mjs";
import {verifyMacPackage} from "./verify-macos-package.mjs";
import {loadReleaseNotes} from "./release-notes.mjs";

const desktop=fileURLToPath(new URL("../",import.meta.url));
const root=path.dirname(desktop);
const cli=path.join(desktop,"node_modules/@tauri-apps/cli/tauri.js");
const values=new Map();const flags=new Set();
for(let i=2;i<process.argv.length;i++) {
  const key=process.argv[i];
  if(["--debug","--test-build"].includes(key)){flags.add(key);continue;}
  if(!["--version","--public-key-file","--transition-from-public-key-file","--notes-file","--native-test-origin"].includes(key) || !process.argv[i+1])throw new Error("Unknown or incomplete installed-build argument.");
  if(values.has(key))throw new Error("Duplicate installed-build argument.");values.set(key,process.argv[++i]);
}
if(process.env.SSH_CLIPBOARD_NATIVE_TEST_ORIGIN !== undefined)throw new Error("Never inherit a native test transport; pass --test-build --native-test-origin explicitly.");
const nativeTestOrigin=nativeOrigin(values.get("--native-test-origin"),flags.has("--test-build"),flags.has("--debug"));
const version=values.get("--version");
const coreVersion=readFileSync(path.join(root,"Cargo.toml"),"utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if(!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(version??"") || version!==coreVersion || version==="0.0.0")throw new Error("Pass a stable --version matching the shared core's Cargo.toml.");
if(flags.has("--debug")&&!flags.has("--test-build"))throw new Error("Debug desktop packages require --test-build.");
// Reject missing, stale or draft notes before compilation or signing inputs.
const notes=loadReleaseNotes({version,desktop,notesFile:values.get("--notes-file"),testOnly:flags.has("--test-build")});
const sourceProof=releaseSource(root,{testOnly:flags.has("--test-build"),expectedSource:process.env.GITHUB_SHA});
if(!values.get("--public-key-file"))throw new Error("An explicit public signing key file is required.");
if(!process.env.TAURI_SIGNING_PRIVATE_KEY || process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD===undefined)throw new Error("Set protected signing key/password environment variables before staging packages.");
if(!flags.has("--test-build") && existsSync(process.env.TAURI_SIGNING_PRIVATE_KEY)) {
  const privatePath=path.resolve(process.env.TAURI_SIGNING_PRIVATE_KEY);
  if(privatePath.startsWith(path.resolve(root)+path.sep))throw new Error("Keep production private signing keys outside the checkout.");
}
const publicKey=readFileSync(values.get("--public-key-file"),"utf8").trim();
const signingPublicKey=readFileSync(values.get("--transition-from-public-key-file")??values.get("--public-key-file"),"utf8").trim();
if(publicKey.length>4096 || !publicKey || !signingPublicKey)throw new Error("Invalid public key input.");
const keyId=key=>createHash("sha256").update(key).digest("hex").slice(0,16);
const feed=`https://github.com/johansigfrids/ssh_clipboard/releases/latest/download/desktop-stable-${keyId(publicKey)}.json`;
const stage=path.join(desktop,"artifacts",`${flags.has("--test-build")?"TEST-only-":""}installed-${version}-${randomUUID()}`);mkdirSync(stage,{recursive:true});
const targetDir=path.join(desktop,"src-tauri/target");
const env={...process.env,CARGO_TARGET_DIR:targetDir,SSH_CLIPBOARD_UPDATE_PUBLIC_KEY:publicKey,SSH_CLIPBOARD_UPDATE_FEED:feed};
if(nativeTestOrigin)env.SSH_CLIPBOARD_NATIVE_TEST_ORIGIN=nativeTestOrigin;
if(existsSync(env.TAURI_SIGNING_PRIVATE_KEY))env.TAURI_SIGNING_PRIVATE_KEY=path.resolve(env.TAURI_SIGNING_PRIVATE_KEY);
function run(command,args,cwd=desktop,taskEnv=env,capture=false) {
  const result=spawnSync(command,args,{cwd,env:taskEnv,stdio:capture?"pipe":"inherit",encoding:"utf8"});
  if(result.error)throw result.error;
  if(result.status!==0)throw new Error(`Installed build step failed (${result.status}); no package was published.`);
  return result.stdout??"";
}
const host=run("rustc",["-vV"],root,env,true).match(/^host:\s*(\S+)/m)?.[1];
const platforms={"x86_64-pc-windows-msvc":"windows-x86_64","aarch64-pc-windows-msvc":"windows-aarch64","x86_64-apple-darwin":"darwin-x86_64","aarch64-apple-darwin":"darwin-aarch64","x86_64-unknown-linux-gnu":"linux-x86_64","aarch64-unknown-linux-gnu":"linux-aarch64"};
const platform=platforms[host];if(!platform)throw new Error("Unsupported native package target.");
const profile=flags.has("--debug")?"debug":"release";
const extension=platform.startsWith("windows")?".exe":"";
const cliEnv={...process.env,CARGO_TARGET_DIR:path.join(root,"target")};
run("cargo",["build","--locked",...(!flags.has("--debug")?["--release"]:[]),"--no-default-features","--features","system-clipboard","--bin","ssh_clipboard"],root,cliEnv);
const cliBinary=path.join(root,"target",profile,`ssh_clipboard${extension}`);
const reported=run(cliBinary,["--version"],root,cliEnv,true).trim();
if(reported!==`ssh_clipboard ${version}`)throw new Error("Bundled CLI/core version mismatch.");
const sidecars=path.join(desktop,"src-tauri/binaries");mkdirSync(sidecars,{recursive:true});copyFileSync(cliBinary,path.join(sidecars,`ssh_clipboard-${host}${extension}`));
// Plain HTTP is used ONLY for a one-use loopback metadata handoff to the framework.
// The Rust downloader enforces remote HTTPS, bounded bodies, and exact-byte verification.
const overlay={version,plugins:{updater:{pubkey:publicKey,endpoints:[],dangerousInsecureTransportProtocol:true,requireSignedVersion:true,allowDowngrades:false,windows:{installMode:"passive"}}}};
if(flags.has("--test-build")){overlay.productName="SSH Clipboard Update Test";overlay.identifier="io.github.johansigfrids.sshclipboard.update-test";}
if(!platform.startsWith("windows"))overlay.bundle={targets:[platform.startsWith("darwin")?"app":"appimage"]};
// Updater signatures remain separate from Apple's publisher identity. The free
// Mac delivery uses an explicit ad-hoc code signature, including its sidecar.
if(platform.startsWith("darwin"))overlay.bundle.macOS={signingIdentity:"-"};
const overlayPath=path.join(stage,"build-config.json");writeFileSync(overlayPath,JSON.stringify(overlay,null,2));
const started=Date.now();
run(process.execPath,[cli,"build",...(flags.has("--debug")?["--debug"]:[]),"--features","custom-protocol,installed-desktop","--config","src-tauri/tauri.installed.conf.json","--config",overlayPath]);
const bundleDir=path.join(targetDir,profile,"bundle",platform.startsWith("windows")?"nsis":platform.startsWith("darwin")?"macos":"appimage");
const suffix=platform.startsWith("windows")?".exe":platform.startsWith("darwin")?".app.tar.gz":".AppImage";
const built=readdirSync(bundleDir).filter(file=>file.endsWith(suffix) && statSync(path.join(bundleDir,file)).mtimeMs>=started-2000);
if(built.length!==1)throw new Error("Expected exactly one newly built updater package.");
const name=`SSHClipboard-stable-${platform}-${version}${platform.startsWith("windows")?"-setup.exe":suffix}`;
const output=path.join(stage,name);copyFileSync(path.join(bundleDir,built[0]),output);
const signEnv={...env};delete signEnv.TAURI_SIGNING_PRIVATE_KEY_PATH;
if(existsSync(signEnv.TAURI_SIGNING_PRIVATE_KEY)) {signEnv.TAURI_SIGNING_PRIVATE_KEY_PATH=signEnv.TAURI_SIGNING_PRIVATE_KEY;delete signEnv.TAURI_SIGNING_PRIVATE_KEY;}
run(process.execPath,[cli,"signer","sign","--app-version",version,output],desktop,signEnv,true);
const candidate={version,platform,url:`https://github.com/johansigfrids/ssh_clipboard/releases/download/desktop-v${version}/${name}`,signature:readFileSync(`${output}.sig`,"utf8").trim()};
const candidatePath=path.join(stage,"candidate.json");writeFileSync(candidatePath,JSON.stringify(candidate,null,2));
const verifyKey=path.join(stage,"signing-public-key.pub");writeFileSync(verifyKey,signingPublicKey+"\n");
const verifyEnv={...env};delete verifyEnv.SSH_CLIPBOARD_NATIVE_TEST_ORIGIN;
run("cargo",["run","--locked","--manifest-path","src-tauri/Cargo.toml","--features","release-tools","--bin","verify_desktop_update","--",verifyKey,"0.0.0",candidatePath,output],desktop,verifyEnv);
const manifest={version,notes,pub_date:new Date().toISOString(),platforms:{[platform]:{url:candidate.url,signature:candidate.signature}}};
writeFileSync(path.join(stage,`desktop-stable-${keyId(signingPublicKey)}.json`),JSON.stringify(manifest,null,2)+"\n");
const sha256=createHash("sha256").update(readFileSync(output)).digest("hex");
writeFileSync(`${output}.sha256`,`${sha256}  ${name}\n`);
// Check again after building: a changed ref or dirty production source cannot
// receive successful package evidence even if compilation/signing completed.
const finalSource=releaseSource(root,{testOnly:flags.has("--test-build"),expectedSource:sourceProof.source});
writeFileSync(path.join(stage,"build-evidence.json"),JSON.stringify({...sourceProof,sourceDirty:sourceProof.sourceDirty||finalSource.sourceDirty,profile,bytes:statSync(output).size,sha256,testOnly:flags.has("--test-build"),nativeTestOrigin:nativeTestOrigin??null,version,platform,cliVersion:reported,embeddedPublicKeyId:keyId(publicKey),signingPublicKeyId:keyId(signingPublicKey),installed:false,published:false},null,2)+"\n");
if(platform.startsWith("darwin"))verifyMacPackage(stage,JSON.parse(readFileSync(path.join(stage,"build-evidence.json"),"utf8")));
process.stdout.write(`Staged verified ${flags.has("--test-build")?"TEST-ONLY ":""}desktop package: ${stage}\n`);
