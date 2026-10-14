import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import path from "node:path";

// Execute the actual workflow's draft handoff with inert filesystem/API adapters.
// No GitHub credentials, subprocess or real release is used by these probes.
const workflow=readFileSync(new URL("../../.github/workflows/server-release.yml",import.meta.url),"utf8").replace(/\r\n/g,"\n");
const handoff=workflow.split("- name: Create reviewable draft, never publish or overwrite")[1];
const body=handoff.split("node --input-type=module <<'JS'\n")[1].split("\n          JS")[0]
  .split("\n").map(line=>line.slice(10)).filter(line=>!line.startsWith("import ")).join("\n");
const execute=new Function("readFileSync","readdirSync","path","execFileSync","spawnSync","process",body);
function run({proof={},existing=false}={}) {
  const source="a".repeat(40),calls=[];
  const evidence={testOnly:false,profile:"release",platform:"linux-x86_64",version:"0.3.0",source,...proof};
  const fsRead=file=>file.endsWith("build-evidence.json")?JSON.stringify(evidence):Buffer.from("synthetic asset");
  const invoke=()=>execute(fsRead,()=>["server-0.3.0-test-stage"],path,
    (command,args)=>calls.push({command,args}),()=>({status:existing?0:1}),
    {env:{RELEASE_VERSION:"0.3.0",GITHUB_SHA:source,GITHUB_REPOSITORY:"owner/repository"}});
  return {invoke,calls,source};
}
test("server handoff stages an immutable-source draft without taking over the desktop latest feed",()=>{
  const probe=run();probe.invoke();
  assert.equal(probe.calls.length,1);
  const {command,args}=probe.calls[0];
  assert.equal(command,"gh");
  assert.deepEqual(args.slice(0,3),["release","create","server-v0.3.0"]);
  assert.ok(args.includes("--draft"));assert.ok(args.includes("--latest=false"));
  assert.equal(args[args.indexOf("--target")+1],probe.source);
  const assets=args.slice(3,args.indexOf("--repo"));
  assert.equal(assets.length,5);
  assert.ok(assets.some(file=>path.basename(file)==="SSHClipboardServer-stable-linux-x86_64-0.3.0.sig"));
  assert.ok(assets.every(file=>!file.endsWith(".key")&&!file.endsWith(".pub")));
});
test("existing signed releases cannot be overwritten",()=>{
  const probe=run({existing:true});
  assert.throws(probe.invoke,/Never overwrite/);assert.equal(probe.calls.length,0);
});
test("unaccepted source, version, target and test builds cannot create a draft",()=>{
  for(const proof of [{source:"b".repeat(40)},{testOnly:true},{profile:"debug"},{platform:"linux-aarch64"},{version:"0.3.1"}]) {
    const probe=run({proof});assert.throws(probe.invoke,/Unexpected server package identity/);
    assert.equal(probe.calls.length,0);
  }
});
