// Test the real package helper with disposable synthetic trust. Never installs a server.
import {spawnSync} from "node:child_process";
import {mkdtempSync,mkdirSync,readFileSync,readdirSync,existsSync,rmSync} from "node:fs";
import path from "node:path";
import {fileURLToPath} from "node:url";
const root=fileURLToPath(new URL("../../",import.meta.url));
const artifacts=path.join(root,"desktop/artifacts");mkdirSync(artifacts,{recursive:true});
const temp=mkdtempSync(path.join(artifacts,"synthetic-server-package-"));
const key=path.join(temp,"test.key");
try {
  const generate=spawnSync(process.execPath,[path.join(root,"desktop/node_modules/@tauri-apps/cli/tauri.js"),"signer","generate","--ci","--password","","--write-keys",key],{cwd:root,encoding:"utf8"});
  if(generate.status!==0)throw new Error("Synthetic key creation failed; output withheld.");
  const version=readFileSync(path.join(root,"Cargo.toml"),"utf8").match(/^version\s*=\s*"([^"]+)"/m)[1];
  const env={...process.env,TAURI_SIGNING_PRIVATE_KEY:key,TAURI_SIGNING_PRIVATE_KEY_PASSWORD:""};
  const invoke=publicKey=>spawnSync(process.execPath,[path.join(root,"desktop/scripts/build-server.mjs"),"--version",version,"--public-key-file",publicKey,"--debug","--test-build"],
    {cwd:root,env,encoding:"utf8",maxBuffer:8*1024*1024});
  const result=invoke(`${key}.pub`);
  if(result.status!==0)throw new Error("Synthetic server packaging failed.");
  process.stdout.write(result.stdout);
  const second=path.join(temp,"wrong.key");
  const wrong=spawnSync(process.execPath,[path.join(root,"desktop/node_modules/@tauri-apps/cli/tauri.js"),"signer","generate","--ci","--password","","--write-keys",second],{cwd:root,encoding:"utf8"});
  if(wrong.status!==0)throw new Error("Second synthetic key creation failed; output withheld.");
  const before=new Set(readdirSync(artifacts));
  const rejected=invoke(`${second}.pub`);
  if(rejected.status===0)throw new Error("Wrong public trust was accepted.");
  const failed=readdirSync(artifacts).filter(name=>name.startsWith(`TEST-only-server-${version}-`)&&!before.has(name));
  if(failed.length!==1||existsSync(path.join(artifacts,failed[0],"build-evidence.json"))||existsSync(path.join(artifacts,failed[0],"candidate.json")))throw new Error("Rejected package received successful release evidence.");
  process.stdout.write("PASS: a mismatched signing/public key is rejected before candidate or successful build evidence is written.\n");
} finally {
  const resolved=path.resolve(temp);
  if(path.dirname(resolved)!==path.resolve(artifacts)||!path.basename(resolved).startsWith("synthetic-server-package-"))throw new Error("Refusing unexpected cleanup path.");
  rmSync(resolved,{recursive:true,force:true});
}
