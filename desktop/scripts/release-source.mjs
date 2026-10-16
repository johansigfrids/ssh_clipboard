import {spawnSync} from "node:child_process";

// Read only public Git metadata; never pass signing credentials to subprocesses.
export function releaseSource(root,{testOnly=false,expectedSource}={}) {
  const env={...process.env};
  for(const key of ["TAURI_SIGNING_PRIVATE_KEY","TAURI_SIGNING_PRIVATE_KEY_PATH","TAURI_SIGNING_PRIVATE_KEY_PASSWORD"])delete env[key];
  function git(args) {
    const result=spawnSync("git",args,{cwd:root,env,encoding:"utf8",windowsHide:true});
    if(result.status!==0)throw new Error("Cannot verify release source from Git metadata.");
    return result.stdout.trim();
  }
  const source=git(["rev-parse","HEAD"]);
  if(!/^[a-f0-9]{40}$/.test(source)||expectedSource&&source!==expectedSource)throw new Error("Release source differs from the reviewed commit.");
  const sourceDirty=git(["status","--porcelain"])!=="";
  if(sourceDirty&&!testOnly)throw new Error("Production desktop staging requires a clean checkout.");
  return {source,sourceDirty};
}
