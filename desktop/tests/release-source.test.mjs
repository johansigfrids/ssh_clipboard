import test from "node:test";
import assert from "node:assert/strict";
import {spawnSync} from "node:child_process";
import {mkdtempSync,writeFileSync,rmSync} from "node:fs";
import os from "node:os";
import path from "node:path";
import {releaseSource} from "../scripts/release-source.mjs";

test("desktop source admission requires clean reviewed bytes and records explicit test deviations",()=>{
  const root=mkdtempSync(path.join(os.tmpdir(),"ssh-clipboard-release-source-"));
  function git(...args) {
    const result=spawnSync("git",args,{cwd:root,encoding:"utf8",windowsHide:true});
    assert.equal(result.status,0,result.stderr);return result.stdout.trim();
  }
  try {
    git("init");writeFileSync(path.join(root,"source.txt"),"reviewed\n");git("add","source.txt");
    git("-c","user.name=Fixture","-c","user.email=fixture@example.invalid","-c","commit.gpgsign=false","commit","-m","Fixture source");
    const source=git("rev-parse","HEAD");
    assert.deepEqual(releaseSource(root,{expectedSource:source}),{source,sourceDirty:false});
    assert.throws(()=>releaseSource(root,{expectedSource:"b".repeat(40)}),/reviewed commit/);
    writeFileSync(path.join(root,"untracked.txt"),"new code\n");
    assert.throws(()=>releaseSource(root),/clean checkout/);
    assert.deepEqual(releaseSource(root,{testOnly:true}),{source,sourceDirty:true});
    rmSync(path.join(root,"untracked.txt"));writeFileSync(path.join(root,"source.txt"),"changed\n");
    assert.throws(()=>releaseSource(root),/clean checkout/);
    git("add","source.txt");
    assert.throws(()=>releaseSource(root),/clean checkout/);
    git("-c","user.name=Fixture","-c","user.email=fixture@example.invalid","-c","commit.gpgsign=false","commit","-m","Changed fixture");
    assert.throws(()=>releaseSource(root,{expectedSource:source}),/reviewed commit/);
  } finally {
    if(path.dirname(root)!==path.resolve(os.tmpdir())||!path.basename(root).startsWith("ssh-clipboard-release-source-"))throw Error("Unexpected fixture cleanup path");
    rmSync(root,{recursive:true,force:true});
  }
});
