// Assemble one immutable multi-platform draft after each protected job has
// signed and independently verified its package. Never publish or overwrite.
import {createHash,randomUUID} from "node:crypto";
import {readFileSync,readdirSync,writeFileSync,mkdirSync,copyFileSync} from "node:fs";
import {execFileSync,spawnSync} from "node:child_process";
import path from "node:path";
import {fileURLToPath} from "node:url";

export const desktopPlatforms=["windows-x86_64","darwin-aarch64"];

export function admitDesktopStage({version,source,repository,keyId,platform},read) {
  if(!desktopPlatforms.includes(platform)||!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(version)||!/^[a-f0-9]{40}$/.test(source)||!/^[a-f0-9]{16}$/.test(keyId)||repository!=="johansigfrids/ssh_clipboard")throw Error("Unexpected desktop release request");
  const proof=JSON.parse(read("build-evidence.json","utf8"));
  if(proof.testOnly!==false||proof.sourceDirty!==false||proof.profile!=="release"||proof.nativeTestOrigin!==null||proof.installed!==false||proof.published!==false||proof.platform!==platform||proof.version!==version||proof.source!==source||proof.cliVersion!==`ssh_clipboard ${version}`||proof.signingPublicKeyId!==keyId||proof.embeddedPublicKeyId!==keyId)throw Error("Unexpected desktop package identity");
  const name=`SSHClipboard-stable-${platform}-${version}${platform.startsWith("windows")?"-setup.exe":".app.tar.gz"}`;
  const bytes=read(name),sha256=createHash("sha256").update(bytes).digest("hex");
  if(bytes.length===0||bytes.length>256*1024*1024||proof.bytes!==bytes.length||proof.sha256!==sha256||read(`${name}.sha256`,"utf8")!==`${sha256}  ${name}\n`)throw Error("Desktop package differs from verified evidence");
  const signature=read(`${name}.sig`,"utf8").trim();
  const candidate=JSON.parse(read("candidate.json","utf8"));
  const url=`https://github.com/${repository}/releases/download/desktop-v${version}/${name}`;
  if(!signature||signature.length>16384||candidate.version!==version||candidate.platform!==platform||candidate.url!==url||candidate.signature!==signature)throw Error("Desktop candidate differs from signed package identity");
  const feed=JSON.parse(read(`desktop-stable-${keyId}.json`,"utf8"));
  if(feed.version!==version||typeof feed.notes!=="string"||Buffer.byteLength(feed.notes)>16384||typeof feed.pub_date!=="string"||feed.pub_date.length>64||Object.keys(feed.platforms??{}).length!==1||feed.platforms[platform]?.url!==url||feed.platforms[platform]?.signature!==signature)throw Error("Desktop feed differs from admitted candidate");
  const assets=[{from:name,to:name},{from:`${name}.sig`,to:`${name}.sig`},{from:`${name}.sha256`,to:`${name}.sha256`},{from:"candidate.json",to:`candidate-${platform}.json`},{from:"build-evidence.json",to:`build-evidence-${platform}.json`}];
  if(platform.startsWith("darwin")) {
    const inspection=JSON.parse(read("macos-package-inspection.json","utf8"));
    if(inspection.source!==source||inspection.version!==version||inspection.platform!==platform||inspection.testOnly!==false||inspection.sha256!==sha256||inspection.bytes!==bytes.length||inspection.application!=="SSH Clipboard.app"||inspection.identifier!=="io.github.johansigfrids.sshclipboard"||inspection.architecture!=="arm64"||inspection.archiveInspected!==true||inspection.codeSignaturesVerified!==true||inspection.cliVersionVerified!==true||inspection.installed!==false)throw Error("Mac package lacks native archive inspection evidence");
    assets.push({from:"macos-package-inspection.json",to:"macos-package-inspection.json"});
  }
  // Windows Git checkouts may use CRLF while the same Mac source uses LF.
  // Compare the actual notes text without rejecting that platform difference.
  return {platform,version,keyId,notes:feed.notes.replace(/\r\n/g,"\n"),asset:{url,signature},assets};
}

export function combineDesktopFeed(stages) {
  if(stages.length!==desktopPlatforms.length||new Set(stages.map(item=>item.platform)).size!==desktopPlatforms.length||desktopPlatforms.some(platform=>!stages.some(item=>item.platform===platform)))throw Error("Every delivered desktop platform is required exactly once");
  const first=stages[0];
  if(stages.some(item=>item.version!==first.version||item.keyId!==first.keyId||item.notes!==first.notes))throw Error("Desktop platform feeds disagree on version, signing trust or notes");
  return {version:first.version,notes:first.notes,pub_date:new Date().toISOString(),platforms:Object.fromEntries(stages.map(item=>[item.platform,item.asset]))};
}

export function createDesktopDraft({root=fileURLToPath(new URL("../../",import.meta.url)),env=process.env,read=readFileSync,list=readdirSync,write=writeFileSync,mkdir=mkdirSync,copy=copyFileSync,run=execFileSync,probe=spawnSync}={}) {
  const {RELEASE_VERSION:version,GITHUB_SHA:source,GITHUB_REPOSITORY:repository}=env;
  const publicKey=read(path.join(root,"docs/update-signing-public-key.pub"),"utf8").trim();
  const keyId=createHash("sha256").update(publicKey).digest("hex").slice(0,16);
  const inputs=path.join(root,"desktop/artifacts/release-inputs");
  const admitted=desktopPlatforms.map(platform=>{
    const base=path.join(inputs,`desktop-package-${platform}`);
    const stages=list(base).filter(name=>name.startsWith(`installed-${version}-`));
    if(stages.length!==1)throw Error("Expected exactly one verified desktop stage per platform");
    const stage=path.join(base,stages[0]);
    return {...admitDesktopStage({version,source,repository,keyId,platform},(name,encoding)=>read(path.join(stage,name),encoding)),stage};
  });
  const feed=combineDesktopFeed(admitted);
  const tag=`desktop-v${version}`;
  if(probe("gh",["release","view",tag,"--repo",repository],{stdio:"ignore"}).status===0)throw Error("Never overwrite an existing signed release");
  const output=path.join(root,"desktop/artifacts",`desktop-draft-${version}-${randomUUID()}`);
  mkdir(output,{recursive:false});
  const assets=[];
  for(const stage of admitted)for(const asset of stage.assets) {
    const destination=path.join(output,asset.to);
    copy(path.join(stage.stage,asset.from),destination);assets.push(destination);
  }
  const manifest=path.join(output,`desktop-stable-${keyId}.json`);
  write(manifest,JSON.stringify(feed,null,2)+"\n");assets.push(manifest);
  run("gh",["release","create",tag,...assets,"--repo",repository,"--target",source,"--draft","--title",`SSH Clipboard ${version}`,"--notes-file",path.join(root,"CHANGELOG.md")],{stdio:"inherit"});
}

if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url))createDesktopDraft();
