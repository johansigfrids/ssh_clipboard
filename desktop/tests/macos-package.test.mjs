import test from "node:test";
import assert from "node:assert/strict";
import {macPackageIdentity,checkMacArchivePaths} from "../scripts/verify-macos-package.mjs";

test("Mac package identity distinguishes architectures and production/test applications",()=>{
  const production=macPackageIdentity({version:"0.4.1",platform:"darwin-aarch64",testOnly:false});
  assert.equal(production.arch,"arm64");
  assert.equal(production.application,"SSH Clipboard.app");
  assert.equal(production.identifier,"io.github.johansigfrids.sshclipboard");
  assert.equal(production.packageName,"SSHClipboard-stable-darwin-aarch64-0.4.1.app.tar.gz");
  const isolated=macPackageIdentity({version:"0.4.1",platform:"darwin-x86_64",testOnly:true});
  assert.equal(isolated.arch,"x86_64");
  assert.equal(isolated.identifier,"io.github.johansigfrids.sshclipboard.update-test");
  for(const change of [{platform:"windows-x86_64"},{version:"../0.4.1"},{testOnly:undefined}])assert.throws(()=>macPackageIdentity({version:"0.4.1",platform:"darwin-aarch64",testOnly:true,...change}));
});

test("Mac archive admits the expected app only and refuses traversal or multiple roots",()=>{
  const app="SSH Clipboard Update Test.app";
  checkMacArchivePaths(`${app}/\n./${app}/Contents/MacOS/ssh_clipboard\n`,app);
  for(const listing of ["",`${app}/../outside`,`${app}//Contents`,`${app}/./Contents`,`/${app}/Contents`,`other.app/Contents`,`${app}/Contents\nother.app/Contents`,`${app}/Contents\\outside`])assert.throws(()=>checkMacArchivePaths(listing,app));
});
