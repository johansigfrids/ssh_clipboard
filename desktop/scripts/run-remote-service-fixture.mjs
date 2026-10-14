// Real user-systemd acceptance on an explicitly selected SSH test host.
import {spawn,spawnSync} from "node:child_process";
import {readFileSync} from "node:fs";
import {createInterface} from "node:readline";
import path from "node:path";
import {fileURLToPath} from "node:url";

async function bounded(promise,milliseconds,message) {
  let timer;
  try {return await Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error(message)),milliseconds);})]);}
  finally {clearTimeout(timer);}
}
export async function runRemoteFixture(packages,target) {
  if(!/^[a-zA-Z0-9_][a-zA-Z0-9_.@:-]*$/.test(target))throw new Error("Use an SSH alias or user@host.");
  const repo=fileURLToPath(new URL("../../",import.meta.url));
  const source=readFileSync(path.join(repo,"scripts/live-managed-service-fixture.py"));
  const script=`python3 -u -c 'import base64;exec(base64.b64decode("${source.toString("base64")}"))'`;
  const options=["-o","BatchMode=yes","-o","ConnectTimeout=8"];
  const child=spawn("ssh",[...options,target,script],{stdio:["pipe","pipe","inherit"],windowsHide:true});
  child.stdin.on("error",()=>{}); // SSH failure is reported by the exit/reader path.
  const exited=new Promise((resolve,reject)=>{child.once("error",reject);child.once("close",code=>resolve(code));});
  const lines=createInterface({input:child.stdout,crlfDelay:Infinity});
  const iterator=lines[Symbol.asyncIterator]();
  let remoteRoot,passed=false,cleaned=false;
  try {
    const first=await bounded(iterator.next(),15000,"SSH fixture did not initialize.");
    if(first.done)throw new Error("SSH fixture ended before initialization.");
    remoteRoot=JSON.parse(first.value).root;
    if(!/^\/tmp\/ssh-clipboard-systemd-[a-zA-Z0-9_]+$/.test(remoteRoot))throw new Error("Unexpected temporary service path.");
    const names=["trusted.pub",...["0.3.0","0.3.1","0.3.2"].flatMap(version=>{
      const name=`SSHClipboardServer-stable-linux-x86_64-${version}`;return [name,`${name}.sig`];
    })];
    const copied=spawnSync("scp",["-q",...options,...names.map(name=>path.join(packages,name)),`${target}:${remoteRoot}/packages/`],
      {encoding:"utf8",timeout:90000,windowsHide:true});
    if(copied.status!==0)throw new Error(`Could not upload public test packages: ${copied.stderr??""}`);
    child.stdin.end('{"command":"start"}\n');
    while(true) {
      const item=await bounded(iterator.next(),150000,"Temporary service test timed out.");
      if(item.done)break;
      const result=JSON.parse(item.value);
      if(result.passed){passed=true;process.stdout.write(`PASS real user-systemd: ${result.checks.join(", ")}\n`);}
      if(result.cleaned&&result.legacy_unchanged&&result.ssh_route_unchanged)cleaned=true;
    }
    const code=await bounded(exited,10000,"SSH fixture did not exit after cleanup.");
    if(code!==0||!passed||!cleaned)throw new Error("Real service acceptance failed; inspect fixture diagnostics above.");
  } finally {
    child.stdin.end();
    try {await bounded(exited,15000,"SSH fixture cleanup did not finish.");}
    catch {child.kill();}
    lines.close();
    if(remoteRoot) {
      const removed=spawnSync("ssh",[...options,target,`test ! -e ${remoteRoot}`],{encoding:"utf8",timeout:15000,windowsHide:true});
      if(removed.status!==0)throw new Error(`Temporary service data was retained for repair: ${remoteRoot}`);
      process.stdout.write("Confirmed temporary directory removal; installed service identity and SSH route were preserved.\n");
    }
  }
}
