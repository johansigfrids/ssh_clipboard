// Disposable Mac replacement/restart rehearsal. Never installs or publishes.
import {execFileSync,spawnSync} from 'node:child_process';
import {randomBytes,createHash} from 'node:crypto';
import {readFileSync,writeFileSync,mkdtempSync,mkdirSync,readdirSync,rmSync} from 'node:fs';
import path from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';

if(process.platform!=='darwin'||process.argv.length!==2)throw Error('Run without arguments on a disposable native Mac builder.');
const root=fileURLToPath(new URL('../../',import.meta.url)),desktop=path.join(root,'desktop');
const cli=path.join(desktop,'node_modules/@tauri-apps/cli/tauri.js');
const original=new Map(['Cargo.toml','Cargo.lock','desktop/src-tauri/Cargo.lock'].map(name=>[name,readFileSync(path.join(root,name),'utf8')]));
const core=original.get('Cargo.toml').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if(core!=='0.4.2')throw Error('This rehearsal expects the reviewed 0.4.2 baseline.');
const source=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim();
if(execFileSync('git',['status','--porcelain'],{cwd:root,encoding:'utf8'}).trim())throw Error('Start from a clean source checkout.');
const privateRoot=mkdtempSync(path.join(tmpdir(),'ssh-clipboard-mac-test-key-'));
const key=path.join(privateRoot,'DISPOSABLE.key');
const token=randomBytes(16).toString('hex'),port=48173,origin=`http://127.0.0.1:${port}/${token}`;
const artifacts=path.join(desktop,'artifacts');mkdirSync(artifacts,{recursive:true});
const packages=[];
try {
  const generated=spawnSync(process.execPath,[cli,'signer','generate','--ci','--password','','--write-keys',key],{cwd:desktop,stdio:'pipe'});
  if(generated.status!==0)throw Error('Disposable signer generation failed; output withheld.');
  for(const version of ['0.4.2','0.4.3']){
    writeFileSync(path.join(root,'Cargo.toml'),original.get('Cargo.toml').replace(/^version\s*=\s*"([^"]+)"/m,`version = "${version}"`));
    for(const name of ['Cargo.lock','desktop/src-tauri/Cargo.lock']){
      const expression=/name = "ssh_clipboard"\r?\nversion = "0\.4\.2"/;
      if(!expression.test(original.get(name)))throw Error('Missing core lock anchor.');
      writeFileSync(path.join(root,name),original.get(name).replace(expression,`name = "ssh_clipboard"\nversion = "${version}"`));
    }
    const notes=path.join(privateRoot,'notes.txt');writeFileSync(notes,`Disposable Mac update rehearsal ${version}. No production release.\n`);
    const before=new Set(readdirSync(artifacts));
    execFileSync(process.execPath,[path.join(desktop,'scripts/build-installed.mjs'),'--test-build','--version',version,'--public-key-file',key+'.pub','--native-test-origin',origin,'--notes-file',notes],{cwd:desktop,stdio:'inherit',env:{...process.env,TAURI_SIGNING_PRIVATE_KEY:key,TAURI_SIGNING_PRIVATE_KEY_PASSWORD:''}});
    const stages=readdirSync(artifacts).filter(name=>name.startsWith(`TEST-only-installed-${version}-`)&&!before.has(name));
    if(stages.length!==1)throw Error('Expected one new Mac rehearsal stage.');
    const stage=stages[0],proof=JSON.parse(readFileSync(path.join(artifacts,stage,'build-evidence.json'),'utf8'));
    if(proof.source!==source||proof.testOnly!==true||proof.nativeTestOrigin!==origin||proof.profile!=='release'||proof.platform!=='darwin-aarch64'||proof.installed!==false||proof.published!==false)throw Error('Unexpected native rehearsal proof.');
    const publicKey=readFileSync(key+'.pub','utf8').trim();
    if(createHash('sha256').update(publicKey).digest('hex').slice(0,16)!==proof.signingPublicKeyId||proof.signingPublicKeyId!==proof.embeddedPublicKeyId)throw Error('Disposable signing trust mismatch.');
    packages.push({version,stage,bytes:proof.bytes,sha256:proof.sha256,publicKeyId:proof.signingPublicKeyId});
  }
  writeFileSync(path.join(artifacts,'macos-update-rehearsal.json'),JSON.stringify({testOnly:true,source,origin,port,token,packages,settingsDirectory:'ssh_clipboard-update-test',installed:false,builtinUpdateTested:false,productionSigningUsed:false},null,2)+'\n');
} finally {
  for(const [name,text] of original)writeFileSync(path.join(root,name),text);
  if(path.dirname(privateRoot)!==path.resolve(tmpdir())||!path.basename(privateRoot).startsWith('ssh-clipboard-mac-test-key-'))throw Error('Unexpected disposable-key cleanup path.');
  rmSync(privateRoot,{recursive:true,force:true});
}
