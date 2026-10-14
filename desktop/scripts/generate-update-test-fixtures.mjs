// Synthetic test keys only. Private material stays in a disposable ignored directory.
import {spawnSync} from "node:child_process";
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,rmSync} from "node:fs";
import path from "node:path";
import {fileURLToPath} from "node:url";
const desktop=fileURLToPath(new URL("../",import.meta.url));
const artifacts=path.join(desktop,"artifacts");mkdirSync(artifacts,{recursive:true});
const temp=mkdtempSync(path.join(artifacts,"synthetic-signing-"));
const cli=path.join(desktop,"node_modules/@tauri-apps/cli/tauri.js");
const server=process.argv.includes("--server");
function syntheticElf(machine=62) {
  const bytes=Buffer.alloc(96);bytes.set([0x7f,69,76,70,2,1,1]);bytes.writeUInt16LE(3,16);bytes.writeUInt16LE(machine,18);bytes.writeUInt16LE(64,52);bytes.write("SYNTHETIC NONEXECUTABLE FIXTURE",64);return bytes;
}
function run(args) {
  const result=spawnSync(process.execPath,[cli,...args],{cwd:desktop,encoding:"utf8"});
  if(result.status!==0) throw new Error(`Synthetic signing command failed (${result.status}); output withheld.`);
}
try {
  for(const key of ["a","b"])run(["signer","generate","--ci","--password","","--write-keys",path.join(temp,`${key}.key`)]);
  const key_a=readFileSync(path.join(temp,"a.key.pub"),"utf8").trim();
  const key_b=readFileSync(path.join(temp,"b.key.pub"),"utf8").trim();
  const fixture={purpose:"Synthetic public signature fixtures. No private release keys.",key_a,key_b,artifacts:{}};
  const specs=server ? [
    ["old","a","1.0.0","SSHClipboardServer-stable-linux-x86_64-1.0.0",syntheticElf()],
    ["release","a","2.0.0","SSHClipboardServer-stable-linux-x86_64-2.0.0",syntheticElf()],
    ["beta","a","2.0.0","SSHClipboardServer-beta-linux-x86_64-2.0.0",syntheticElf()],
    ["arm","a","2.0.0","SSHClipboardServer-stable-linux-aarch64-2.0.0",syntheticElf(183)],
    ["unversioned","a","2.0.0","SSHClipboardServer-stable-linux-x86_64-2.0.0",syntheticElf(),false],
    ["wrong_elf","a","2.0.0","SSHClipboardServer-stable-linux-x86_64-2.0.0",Buffer.alloc(96)],
  ] : [
    ["release","a","2.0.0","SSHClipboard-stable-windows-x86_64-2.0.0-setup.exe","synthetic release"],
    ["beta","a","2.0.0","SSHClipboard-beta-windows-x86_64-2.0.0-setup.exe","synthetic beta"],
    ["other_platform","a","2.0.0","SSHClipboard-stable-linux-x86_64-2.0.0.AppImage","synthetic Linux"],
    ["transition","a","2.0.0","SSHClipboard-stable-windows-x86_64-2.0.0-setup.exe",`synthetic transition\nnext public key: ${key_b}`],
    ["next","b","3.0.0","SSHClipboard-stable-windows-x86_64-3.0.0-setup.exe","synthetic next release"],
    ["unversioned_signature","a","2.0.0","SSHClipboard-stable-windows-x86_64-2.0.0-setup.exe","synthetic unversioned signature",false],
  ];
  for(const [name,key,version,file,content,bindVersion=true] of specs) {
    const input=path.join(temp,file);writeFileSync(input,content);
    run(["signer","sign","--private-key-path",path.join(temp,`${key}.key`),"--password","",...(bindVersion ? ["--app-version",version] : []),input]);
    fixture.artifacts[name]={version,file,bytes:Buffer.from(content).toString("base64"),signature:readFileSync(`${input}.sig`,"utf8").trim()};
  }
  const output=server ? path.join(desktop,"../tests/fixtures/server-signing.json") : path.join(desktop,"src-tauri/tests/fixtures/update-signing.json");mkdirSync(path.dirname(output),{recursive:true});
  writeFileSync(output,JSON.stringify(fixture,null,2)+"\n");
  process.stdout.write("Generated public synthetic update-signature fixtures.\n");
} finally {
  const resolved=path.resolve(temp);
  if(path.dirname(resolved)!==path.resolve(artifacts) || !path.basename(resolved).startsWith("synthetic-signing-")) throw new Error("Refusing unexpected cleanup path.");
  rmSync(resolved,{recursive:true,force:true});
}
