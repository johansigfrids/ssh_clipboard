// Inspect the actual archived app without launching its GUI or installing it.
import {execFileSync} from "node:child_process";
import {mkdtempSync,readFileSync,rmSync,writeFileSync} from "node:fs";
import {tmpdir} from "node:os";
import path from "node:path";

export function macPackageIdentity(evidence) {
  const arch={"darwin-aarch64":"arm64","darwin-x86_64":"x86_64"}[evidence.platform];
  if(!arch||!/^\d+\.\d+\.\d+$/.test(evidence.version)||typeof evidence.testOnly!=="boolean")throw new Error("Invalid Mac package identity.");
  return {
    arch,
    application:evidence.testOnly?"SSH Clipboard Update Test.app":"SSH Clipboard.app",
    identifier:"io.github.johansigfrids.sshclipboard"+(evidence.testOnly?".update-test":""),
    packageName:`SSHClipboard-stable-${evidence.platform}-${evidence.version}.app.tar.gz`,
  };
}

export function checkMacArchivePaths(listing,application) {
  const entries=listing.trim().split(/\r?\n/);
  if(!listing.trim()||entries.length>10000)throw new Error("Unexpected Mac archive contents.");
  for(const entry of entries) {
    const relative=entry.replace(/^\.\//,"");
    const parts=relative.replace(/\/$/,"").split("/");
    if(parts[0]!==application||parts.some(part=>!part||part==="."||part==="..")||relative.includes("\\"))throw new Error("Mac archive must contain exactly the expected app.");
  }
}

export function verifyMacPackage(stage,evidence) {
  if(process.platform!=="darwin")throw new Error("Mac application inspection requires native macOS tools.");
  const identity=macPackageIdentity(evidence);
  const archive=path.resolve(stage,identity.packageName);
  const run=(command,args)=>execFileSync(command,args,{encoding:"utf8",stdio:["ignore","pipe","pipe"],timeout:30000,maxBuffer:2*1024*1024}).trim();
  checkMacArchivePaths(run("/usr/bin/tar",["-tzf",archive]),identity.application);
  const temporary=mkdtempSync(path.join(tmpdir(),"ssh-clipboard-mac-package-"));
  try {
    run("/usr/bin/tar",["-xzf",archive,"-C",temporary]);
    const app=path.join(temporary,identity.application);
    const info=JSON.parse(run("/usr/bin/plutil",["-convert","json","-o","-",path.join(app,"Contents/Info.plist")]));
    if(info.CFBundleIdentifier!==identity.identifier||info.CFBundleShortVersionString!==evidence.version||info.CFBundleExecutable!=="ssh_clipboard_desktop")throw new Error("Archived Mac bundle identity/version differs from admitted package.");
    const main=path.join(app,"Contents/MacOS",info.CFBundleExecutable);
    const cli=path.join(app,"Contents/MacOS/ssh_clipboard");
    for(const binary of [main,cli]) {
      if(run("/usr/bin/lipo",["-archs",binary])!==identity.arch)throw new Error("Archived Mac binary has an unexpected architecture.");
      run("/usr/bin/codesign",["--verify","--strict",binary]);
    }
    run("/usr/bin/codesign",["--verify","--deep","--strict",app]);
    if(run(cli,["--version"])!==`ssh_clipboard ${evidence.version}`)throw new Error("Archived Mac CLI version differs from admitted package.");
    // Re-read the stage evidence to ensure it remains parseable without making
    // native installation, GUI, login or update acceptance claims.
    const source=JSON.parse(readFileSync(path.join(stage,"build-evidence.json"),"utf8")).source;
    if(source!==evidence.source)throw new Error("Mac package source evidence changed during inspection.");
    writeFileSync(path.join(stage,"macos-package-inspection.json"),JSON.stringify({source,version:evidence.version,platform:evidence.platform,testOnly:evidence.testOnly,application:identity.application,identifier:identity.identifier,architecture:identity.arch,archiveInspected:true,codeSignaturesVerified:true,cliVersionVerified:true,installed:false,guiTested:false,loginTested:false,builtinUpdateTested:false},null,2)+"\n");
  } finally {
    rmSync(temporary,{recursive:true,force:true});
  }
}
