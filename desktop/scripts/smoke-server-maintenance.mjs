// Windows + existing Ubuntu WSL toolchains. Real-service mode requires an explicit SSH target.
import {spawnSync} from "node:child_process";
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,rmSync} from "node:fs";
import path from "node:path";
import {fileURLToPath} from "node:url";
import {runRemoteFixture} from "./run-remote-service-fixture.mjs";
const arguments_=process.argv.slice(2);
if(arguments_.length&&!(arguments_.length===2&&arguments_[0]==="--ssh-target"))throw new Error("Only --ssh-target <alias-or-user@host> is supported.");
const sshTarget=arguments_[1];
const root=fileURLToPath(new URL("../../",import.meta.url));
const artifacts=path.join(root,"desktop/artifacts");mkdirSync(artifacts,{recursive:true});
const temp=mkdtempSync(path.join(artifacts,"TEST-only-server-maintenance-"));
const cli=path.join(root,"desktop/node_modules/@tauri-apps/cli/tauri.js");
const linux=path.resolve(temp).replaceAll("\\","/").replace(/^([A-Za-z]):/,(_,drive)=>`/mnt/${drive.toLowerCase()}`);
function run(command,args,options={}) {
  const result=spawnSync(command,args,{cwd:root,encoding:"utf8",timeout:600000,...options});
  if(result.status!==0)throw new Error(`Disposable server test command failed: ${result.stderr || result.error || result.status}`);
  return result.stdout;
}
function wsl(mode) {
  const script=path.join(root,"scripts/test-managed-server.py").replaceAll("\\","/").replace(/^([A-Za-z]):/,(_,drive)=>`/mnt/${drive.toLowerCase()}`);
  return run("wsl.exe",["-d","Ubuntu","--exec","python3",script,mode,linux,...(mode==="prepare"&&sshTarget?["--crashing-release"]:[])]);
}
function signer(args) {
  // signer generation can print a private key: never relay its output.
  const result=spawnSync(process.execPath,[cli,"signer",...args],{cwd:root,encoding:"utf8"});
  if(result.status!==0)throw new Error("Synthetic server signing failed; command output withheld.");
}
try {
  process.stdout.write("Building disposable headless versions with the existing WSL toolchain...\n");
  wsl("prepare");
  const key=path.join(temp,"synthetic.key");
  signer(["generate","--ci","--password","","--write-keys",key]);
  writeFileSync(path.join(temp,"trusted.pub"),readFileSync(`${key}.pub`));
  for(const version of ["0.3.0","0.3.1","0.3.2"]) {
    signer(["sign","--private-key-path",key,"--password","","--app-version",version,
      path.join(temp,`SSHClipboardServer-stable-linux-x86_64-${version}`)]);
  }
  rmSync(key); // Tests need only public trust/signatures.
  if(sshTarget)await runRemoteFixture(temp,sshTarget);
  else process.stdout.write(wsl("test"));
} finally {
  const resolved=path.resolve(temp);
  if(path.dirname(resolved)!==path.resolve(artifacts)||!path.basename(resolved).startsWith("TEST-only-server-maintenance-"))throw new Error("Refusing unexpected cleanup path.");
  rmSync(resolved,{recursive:true,force:true});
}
