import test from "node:test";
import assert from "node:assert/strict";
import {mkdtempSync,readFileSync,writeFileSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {spawnSync} from "node:child_process";
import path from "node:path";
import {fileURLToPath} from "node:url";
import {loadReleaseNotes,validateReleaseNotes,maxNotesBytes} from "../scripts/release-notes.mjs";

const version="1.2.3",notes=`# ${version}\n\n- Share text and images.\n- Keep café text intact.\n`;

test("curated release notes preserve bullets and normalize platform newlines",()=>{
  assert.equal(validateReleaseNotes(version,notes),notes);
  assert.equal(validateReleaseNotes(version,"\uFEFF"+notes.replaceAll("\n","\r\n")),notes);
});

test("stable notes refuse missing content, other versions, changelogs and draft status",()=>{
  for(const text of ["",`# ${version}`,"# 9.9.9\n\n- Wrong version.","# Changelog\n\n## [Unreleased]\n\n- Old entry.",`# ${version}\n\nA paragraph instead of release bullets.`,...['Unreleased candidate','Release candidate only','Production signing and publication are pending','Not yet published','TODO: finish notes','TBD'].map(marker=>`# ${version}\n\n- ${marker}.`)])assert.throws(()=>validateReleaseNotes(version,text),/Release notes/);
  for(const value of [undefined,"0.0.0","1.2.3-beta.1","../secret","01.2.3"])assert.throws(()=>validateReleaseNotes(value,notes),/exact stable version/);
});

test("UTF-8 and canonical output both stay within the feed notes budget",()=>{
  assert.throws(()=>validateReleaseNotes(version,`# ${version}\n\n- ${"😀".repeat(maxNotesBytes/4)}`),/16 KiB/);
  const prefix=`# ${version}\n\n- `;
  const exact=prefix+"x".repeat(maxNotesBytes-Buffer.byteLength(prefix)-1)+"\n";
  assert.equal(Buffer.byteLength(validateReleaseNotes(version,exact)),maxNotesBytes);
  const tight=`# ${version}\n- `+"x".repeat(maxNotesBytes-Buffer.byteLength(`# ${version}\n- `));
  assert.throws(()=>validateReleaseNotes(version,tight),/16 KiB/);
});

test("loading selects only the requested version and reserves overrides for test builds",()=>{
  const desktop=path.resolve("checkout/desktop"),reads=[];
  const read=(file,encoding)=>{reads.push({file,encoding});return notes;};
  assert.equal(loadReleaseNotes({version,desktop},read),notes);
  assert.deepEqual(reads,[{file:path.join(desktop,"release-notes",`${version}.md`),encoding:"utf8"}]);
  assert.throws(()=>loadReleaseNotes({version,desktop},()=>{throw Error("missing");}),/missing or unreadable/);
  assert.throws(()=>loadReleaseNotes({version,desktop,notesFile:"CHANGELOG.md"},read),/only allowed.*test builds/);
  assert.equal(loadReleaseNotes({version,desktop,notesFile:"fixture-notes.md",testOnly:true},read),notes);
  assert.throws(()=>loadReleaseNotes({version,notesFile:"bad.md",testOnly:true},()=>"# 2.0.0\n\n- Wrong notes."),/start with/);
  assert.throws(()=>loadReleaseNotes({version:"../secret"},()=>{throw Error("must not read");}),/exact stable version/);
});

test("installed-build rejects invalid notes before credentials, compilation or staging",()=>{
  const desktop=fileURLToPath(new URL("../",import.meta.url));
  const current=readFileSync(path.join(desktop,"../Cargo.toml"),"utf8").match(/^version\s*=\s*"([^"]+)"/m)[1];
  const temporary=mkdtempSync(path.join(tmpdir(),"ssh-clipboard-notes-"));
  try {
    const file=path.join(temporary,"notes.md");writeFileSync(file,"# Wrong version\n\n- Must fail early.\n");
    const result=spawnSync(process.execPath,[path.join(desktop,"scripts/build-installed.mjs"),"--test-build","--version",current,"--notes-file",file],{cwd:desktop,encoding:"utf8",env:{...process.env,TAURI_SIGNING_PRIVATE_KEY:"",TAURI_SIGNING_PRIVATE_KEY_PASSWORD:""}});
    assert.notEqual(result.status,0);
    assert.match(result.stderr,/Release notes must start with/);
    assert.doesNotMatch(result.stderr,/explicit public signing key file is required/);
  } finally {
    const resolved=path.resolve(temporary);
    if(path.dirname(resolved)!==path.resolve(tmpdir())||!path.basename(resolved).startsWith("ssh-clipboard-notes-"))throw Error("Unexpected release-notes fixture cleanup path.");
    rmSync(resolved,{recursive:true,force:true});
  }
});
