import {readFileSync} from "node:fs";
import path from "node:path";
import {fileURLToPath} from "node:url";

export const maxNotesBytes=16*1024;
function validateVersion(version) {
  if(typeof version!=="string"||!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(version)||version==="0.0.0")throw Error("Release notes require an exact stable version.");
}

export function validateReleaseNotes(version,text) {
  validateVersion(version);
  if(typeof text!=="string"||Buffer.byteLength(text)>maxNotesBytes)throw Error("Release notes must be text of at most 16 KiB.");
  const normalized=text.replace(/^\uFEFF/,"").replace(/\r\n/g,"\n").trim();
  const [heading,...body]=normalized.split("\n");
  if(heading!==`# ${version}`)throw Error(`Release notes must start with # ${version}.`);
  const bullets=body.filter(line=>line.trim()!=="");
  if(!bullets.length||bullets.some(line=>!/^\- \S/.test(line)))throw Error("Release notes need plain user-facing bullets after the version heading.");
  if(/\b(?:unreleased|release candidate|candidate only|todo|tbd)\b|\b(?:publication|production signing)[^\n.!?]{0,80}\bpending\b|\bnot yet (?:published|released)\b/i.test(normalized))throw Error("Release notes for a stable version cannot contain draft or publication-pending wording.");
  const notes=`${heading}\n\n${bullets.join("\n")}\n`;
  if(Buffer.byteLength(notes)>maxNotesBytes)throw Error("Release notes must be text of at most 16 KiB.");
  return notes;
}

export function loadReleaseNotes({version,desktop=fileURLToPath(new URL("../",import.meta.url)),notesFile,testOnly=false},read=readFileSync) {
  validateVersion(version);
  if(notesFile&&!testOnly)throw Error("Release notes overrides are only allowed for explicit test builds.");
  const file=notesFile??path.join(desktop,"release-notes",`${version}.md`);
  let text;
  try {text=read(file,"utf8");}catch {throw Error(`Release notes for ${version} are missing or unreadable.`);}
  return validateReleaseNotes(version,text);
}

if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  if(process.argv.length!==3)throw Error("Pass the exact desktop version to validate its release notes.");
  loadReleaseNotes({version:process.argv[2]});
  process.stdout.write(`Validated desktop release notes for ${process.argv[2]}.\n`);
}
