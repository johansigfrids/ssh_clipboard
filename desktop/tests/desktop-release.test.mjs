import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createHash} from "node:crypto";
import path from "node:path";

// Execute the workflow handoff itself with inert filesystem/API adapters.
const workflow=readFileSync(new URL("../../.github/workflows/desktop-release.yml",import.meta.url),"utf8").replace(/\r\n/g,"\n");
const handoff=workflow.split("- name: Create reviewable draft, never publish or overwrite")[1];
const body=handoff.split("@'\n")[1].split("\n          '@ | node")[0]
  .split("\n").map(line=>line.slice(10)).filter(line=>!line.startsWith("import ")).join("\n");
const execute=new Function("readFileSync","readdirSync","path","createHash","execFileSync","spawnSync","process",body);
function run({proof={},existing=false,stages,missing,alteredPackage=false,alteredChecksum=false}={}) {
  const source="a".repeat(40),calls=[],bytes=Buffer.from("synthetic verified package");
  const sha256=createHash("sha256").update(bytes).digest("hex");
  const name="SSHClipboard-stable-windows-x86_64-0.3.0-setup.exe",key="0123456789abcdef";
  const evidence={source,sourceDirty:false,profile:"release",testOnly:false,nativeTestOrigin:null,installed:false,published:false,platform:"windows-x86_64",version:"0.3.0",cliVersion:"ssh_clipboard 0.3.0",signingPublicKeyId:key,bytes:bytes.length,sha256,...proof};
  function read(file,encoding) {
    const asset=path.basename(file);
    if(asset===missing)throw Error("Missing staged asset");
    if(asset==="build-evidence.json")return JSON.stringify(evidence);
    if(asset===name)return alteredPackage?Buffer.from("unverified replacement"):bytes;
    if(asset===`${name}.sha256`)return alteredChecksum?"wrong checksum":`${sha256}  ${name}\n`;
    return encoding?"synthetic metadata":Buffer.from("synthetic asset");
  }
  const invoke=()=>execute(read,()=>stages??["installed-0.3.0-stage"],path,createHash,
    (command,args)=>calls.push({command,args}),()=>({status:existing?0:1}),
    {env:{RELEASE_VERSION:"0.3.0",GITHUB_SHA:source,GITHUB_REPOSITORY:"owner/repository"}});
  return {invoke,calls,source,name,key};
}

test("desktop handoff creates only a draft for the exact reviewed source and canonical assets",()=>{
  const probe=run();probe.invoke();
  assert.equal(probe.calls.length,1);
  const {command,args}=probe.calls[0];assert.equal(command,"gh");
  assert.deepEqual(args.slice(0,3),["release","create","desktop-v0.3.0"]);
  assert.ok(args.includes("--draft"));assert.ok(!args.includes("--clobber"));
  assert.equal(args[args.indexOf("--target")+1],probe.source);
  assert.deepEqual(args.slice(3,args.indexOf("--repo")).map(file=>path.basename(file)),[
    probe.name,`${probe.name}.sig`,`${probe.name}.sha256`,`desktop-stable-${probe.key}.json`,"candidate.json","build-evidence.json"]);
});

test("desktop handoff refuses mismatched, dirty, debug, test or incomplete identity evidence",()=>{
  for(const proof of [{source:"b".repeat(40)},{source:undefined},{sourceDirty:true},{sourceDirty:undefined},{profile:"debug"},{testOnly:true},{testOnly:undefined},{nativeTestOrigin:"http://127.0.0.1:1234"},{installed:true},{published:true},{platform:"windows-aarch64"},{version:"0.3.1"},{cliVersion:"ssh_clipboard 0.2.0"},{signingPublicKeyId:"../../untrusted"}]) {
    const probe=run({proof});assert.throws(probe.invoke,/Unexpected desktop package identity/);assert.equal(probe.calls.length,0);
  }
});

test("desktop handoff refuses changed bytes, checksum drift, missing assets and ambiguous stages",()=>{
  for(const options of [{alteredPackage:true},{alteredChecksum:true},{proof:{bytes:1}},{proof:{sha256:"0".repeat(64)}},{missing:"candidate.json"},{missing:"SSHClipboard-stable-windows-x86_64-0.3.0-setup.exe.sig"},{stages:[]},{stages:["installed-0.3.0-one","installed-0.3.0-two"]}]) {
    const probe=run(options);assert.throws(probe.invoke);assert.equal(probe.calls.length,0);
  }
});

test("desktop handoff cannot overwrite an existing signed release",()=>{
  const probe=run({existing:true});assert.throws(probe.invoke,/Never overwrite/);assert.equal(probe.calls.length,0);
});
