import test from "node:test";
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import path from "node:path";
import {admitDesktopStage,combineDesktopFeed,createDesktopDraft,desktopPlatforms} from "../scripts/desktop-draft.mjs";

const source="a".repeat(40),version="0.4.2",repository="johansigfrids/ssh_clipboard";
const publicKey="independently pinned synthetic public trust",keyId=createHash("sha256").update(publicKey).digest("hex").slice(0,16);
function fixture(platform="windows-x86_64") {
  const bytes=Buffer.from(`synthetic verified ${platform} package`);
  const sha256=createHash("sha256").update(bytes).digest("hex");
  const name=`SSHClipboard-stable-${platform}-${version}${platform.startsWith("windows")?"-setup.exe":".app.tar.gz"}`;
  const signature=`synthetic-${platform}-signature`,url=`https://github.com/${repository}/releases/download/desktop-v${version}/${name}`;
  const proof={source,sourceDirty:false,profile:"release",testOnly:false,nativeTestOrigin:null,installed:false,published:false,platform,version,cliVersion:`ssh_clipboard ${version}`,signingPublicKeyId:keyId,embeddedPublicKeyId:keyId,bytes:bytes.length,sha256};
  const feed={version,notes:"Reviewed release notes",pub_date:"2026-10-16T00:00:00Z",platforms:{[platform]:{url,signature}}};
  const inspection={source,version,platform,testOnly:false,sha256,bytes:bytes.length,application:"SSH Clipboard.app",identifier:"io.github.johansigfrids.sshclipboard",architecture:"arm64",archiveInspected:true,codeSignaturesVerified:true,cliVersionVerified:true,installed:false};
  const files=new Map([[name,bytes],[`${name}.sig`,signature],[`${name}.sha256`,`${sha256}  ${name}\n`],["build-evidence.json",JSON.stringify(proof)],["candidate.json",JSON.stringify({version,platform,url,signature})],[`desktop-stable-${keyId}.json`,JSON.stringify(feed)],["macos-package-inspection.json",JSON.stringify(inspection)]]);
  const read=(file)=>{if(!files.has(file))throw Error("Missing staged asset");return files.get(file);};
  const edit=(file,change)=>files.set(file,JSON.stringify({...JSON.parse(read(file)),...change}));
  const admit=()=>admitDesktopStage({version,source,repository,keyId,platform},read);
  return {files,read,edit,admit,name};
}

test("desktop admission accepts exact Windows and Apple Silicon package identities",()=>{
  for(const platform of desktopPlatforms) {
    const f=fixture(platform),admitted=f.admit();
    assert.equal(admitted.platform,platform);
    assert.ok(admitted.assets.some(asset=>asset.to===`candidate-${platform}.json`));
    assert.ok(admitted.assets.some(asset=>asset.to===`build-evidence-${platform}.json`));
  }
});

test("desktop admission refuses wrong, dirty, debug, test or incomplete identity for both platforms",()=>{
  for(const platform of desktopPlatforms)for(const proof of [{source:"b".repeat(40)},{source:undefined},{sourceDirty:true},{sourceDirty:undefined},{profile:"debug"},{testOnly:true},{testOnly:undefined},{nativeTestOrigin:"http://127.0.0.1:1234"},{installed:true},{published:true},{platform:"windows-aarch64"},{version:"0.3.1"},{cliVersion:"ssh_clipboard 0.2.0"},{signingPublicKeyId:"../../untrusted"},{embeddedPublicKeyId:"different-key"}]) {
    const f=fixture(platform);f.edit("build-evidence.json",proof);
    assert.throws(f.admit,/Unexpected desktop package identity/);
  }
});

test("desktop admission refuses altered bytes, checksums, candidates, feeds and missing assets",()=>{
  for(const platform of desktopPlatforms) {
    for(const missing of ["candidate.json","build-evidence.json",`${fixture(platform).name}.sig`]) {
      const f=fixture(platform);f.files.delete(missing);assert.throws(f.admit);
    }
    const changes=[
      f=>f.files.set(f.name,Buffer.from("unverified replacement")),
      f=>f.files.set(`${f.name}.sha256`,"changed checksum"),
      f=>f.edit("build-evidence.json",{bytes:1}),
      f=>f.edit("candidate.json",{url:"https://untrusted.invalid/package"}),
      f=>f.edit("candidate.json",{signature:"unverified-signature"}),
      f=>f.edit(`desktop-stable-${keyId}.json`,{platforms:{}}),
      f=>f.edit(`desktop-stable-${keyId}.json`,{version:"0.4.3"}),
    ];
    for(const change of changes){const f=fixture(platform);change(f);assert.throws(f.admit);}
  }
});

test("Mac admission requires archived production identity and native signature/CLI evidence",()=>{
  for(const change of [{source:"b".repeat(40)},{testOnly:true},{sha256:"0".repeat(64)},{bytes:1},{architecture:"x86_64"},{identifier:"io.github.johansigfrids.sshclipboard.update-test"},{application:"SSH Clipboard Update Test.app"},{archiveInspected:false},{codeSignaturesVerified:false},{cliVersionVerified:false},{installed:true}]) {
    const f=fixture("darwin-aarch64");f.edit("macos-package-inspection.json",change);assert.throws(f.admit,/Mac package lacks/);
  }
});

test("combined feed preserves both platforms and refuses missing, duplicate or inconsistent stages",()=>{
  const windows=fixture().admit(),mac=fixture("darwin-aarch64").admit();
  const feed=combineDesktopFeed([windows,mac]);
  assert.deepEqual(Object.keys(feed.platforms).sort(),desktopPlatforms.toSorted());
  for(const stages of [[mac],[windows,windows],[windows,{...mac,version:"0.4.3"}],[windows,{...mac,keyId:"another-key"}],[windows,{...mac,notes:"Different notes"}]])assert.throws(()=>combineDesktopFeed(stages));
});

test("same release notes from Windows CRLF and Mac LF checkouts combine without losing text",()=>{
  const windows=fixture(),mac=fixture("darwin-aarch64");
  windows.edit(`desktop-stable-${keyId}.json`,{notes:"Reviewed\r\nrelease notes\r\n"});
  mac.edit(`desktop-stable-${keyId}.json`,{notes:"Reviewed\nrelease notes\n"});
  assert.equal(combineDesktopFeed([windows.admit(),mac.admit()]).notes,"Reviewed\nrelease notes\n");
});

function handoff({existing=false,stages=[`installed-${version}-stage`],fault}={}) {
  const calls=[],writes=[],copies=[],fixtures=new Map(desktopPlatforms.map(platform=>[platform,fixture(platform)]));
  if(fault)fault(fixtures);
  const options={root:path.resolve("checkout"),env:{RELEASE_VERSION:version,GITHUB_SHA:source,GITHUB_REPOSITORY:repository},
    read(file){if(path.basename(file)==="update-signing-public-key.pub")return publicKey;const platform=desktopPlatforms.find(item=>file.includes(`desktop-package-${item}`));return fixtures.get(platform).read(path.basename(file));},
    list:()=>stages,mkdir:()=>{},copy:(from,to)=>copies.push({from,to}),write:(file,bytes)=>writes.push({file,bytes}),
    run:(command,args)=>calls.push({command,args}),probe:()=>({status:existing?0:1})};
  return {invoke:()=>createDesktopDraft(options),calls,writes,copies};
}

test("actual draft handoff creates one draft with the combined feed and platform-qualified evidence",()=>{
  const f=handoff();f.invoke();assert.equal(f.calls.length,1);
  const {command,args}=f.calls[0];assert.equal(command,"gh");
  assert.deepEqual(args.slice(0,3),["release","create",`desktop-v${version}`]);
  assert.ok(args.includes("--draft"));assert.ok(!args.includes("--clobber"));
  assert.equal(args[args.indexOf("--target")+1],source);
  assert.equal(args.slice(3,args.indexOf("--repo")).length,12);
  assert.equal(f.writes.length,1);
  assert.deepEqual(Object.keys(JSON.parse(f.writes[0].bytes).platforms).sort(),desktopPlatforms.toSorted());
  assert.ok(f.copies.every(copy=>!copy.to.endsWith(".key")&&!copy.to.endsWith(".pub")));
});

test("actual handoff cannot overwrite a release or create a partial-platform draft",()=>{
  for(const options of [{existing:true},{stages:[]},{stages:[`installed-${version}-one`,`installed-${version}-two`]},{fault:fixtures=>fixtures.get("darwin-aarch64").files.delete("candidate.json")}]) {
    const f=handoff(options);assert.throws(f.invoke);assert.equal(f.calls.length,0);assert.equal(f.writes.length,0);assert.equal(f.copies.length,0);
  }
});
