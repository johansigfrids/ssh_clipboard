// Opt-in Windows native rehearsal. Builds only; the owner installs/confirms separately.
import {spawnSync,spawn} from 'node:child_process';
import {randomBytes} from 'node:crypto';
import {mkdirSync,mkdtempSync,copyFileSync,existsSync,writeFileSync,readFileSync,symlinkSync,readdirSync,cpSync,openSync,closeSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../../',import.meta.url));
if(process.platform!=='win32'||process.argv.length!==4||process.argv[2]!=='--state-file')throw new Error('Pass --state-file <existing isolated native acceptance state> on Windows.');
// A malformed quoted PATH entry can hide later Rust tools from the native CLI
// even when Node/where.exe can find them. Scope the verified tool directory to
// this rehearsal process; never edit the user's persisted environment.
const cargoLookup=spawnSync('where.exe',['cargo.exe'],{encoding:'utf8'});
const cargo=cargoLookup.stdout?.split(/\r?\n/).find(file=>path.isAbsolute(file)&&existsSync(file));
if(cargoLookup.status!==0||!cargo)throw new Error('Existing Cargo installation required.');
const pathKey=Object.keys(process.env).find(key=>key.toLowerCase()==='path')??'Path';
process.env[pathKey]=path.dirname(cargo)+';'+(process.env[pathKey]??'');
const statePath=path.resolve(process.argv[3]);
const artifacts=path.join(root,'desktop/artifacts');
const prior=path.dirname(statePath);
if(path.dirname(prior)!==path.resolve(artifacts)||!path.basename(prior).startsWith('TEST-only-native-acceptance-'))throw new Error('Only the owned disposable native acceptance workspace is supported.');
const state=JSON.parse(readFileSync(statePath,'utf8').replace(/^\uFEFF/,''));
const keyA=path.join(prior,'DISPOSABLE-TEST.key');
if(state.identity!=='io.github.johansigfrids.sshclipboard.update-test'||path.resolve(state.keyPath)!==keyA||!existsSync(keyA))throw new Error('Expected existing disposable test identity/key.');
const fixture=mkdtempSync(path.join(artifacts,'TEST-only-native-updater-'));
const token=randomBytes(16).toString('hex');
const configPath=path.join(fixture,'server.json'),readyPath=path.join(fixture,'ready.json'),controlPath=path.join(fixture,'control.json');
const config={testOnly:true,root:fixture,token,assets:{},feeds:{}};
const save=()=>writeFileSync(configPath,JSON.stringify(config,null,2));
save();writeFileSync(controlPath,JSON.stringify({targetVersion:'0.3.3',mode:'valid'}));
const log=openSync(path.join(fixture,'server.log'),'a');
const child=spawn(process.execPath,[path.join(root,'desktop/scripts/native-update-server.mjs'),configPath,readyPath],{cwd:root,detached:true,windowsHide:true,stdio:['ignore',log,log]});
child.unref();closeSync(log);
state.nativeUpdaterFixture=fixture;writeFileSync(statePath,JSON.stringify(state,null,2));
try {
  for(let attempt=0;!existsSync(readyPath)&&attempt<200;attempt++)await new Promise(resolve=>setTimeout(resolve,50));
  if(!existsSync(readyPath))throw new Error('Loopback fixture did not start.');
  const {origin}=JSON.parse(readFileSync(readyPath,'utf8'));
  const keyB=path.join(fixture,'DISPOSABLE-NEXT.key');
  const cli=path.join(root,'desktop/node_modules/@tauri-apps/cli/tauri.js');
  const generated=spawnSync(process.execPath,[cli,'signer','generate','--ci','--password','','--write-keys',keyB],{cwd:root,stdio:'pipe'});
  if(generated.status!==0)throw new Error('Disposable next-key generation failed; signer output withheld.');
  const listed=spawnSync('git',['ls-files','-z'],{cwd:root,encoding:'utf8'});
  if(listed.status!==0)throw new Error('Could not inventory committed source.');
  const head=spawnSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).stdout.trim();
  const packages=[];
  for(const version of ['0.3.2','0.3.3','0.3.4','0.3.5']) {
    const snapshot=mkdtempSync(path.join(root,'target','native-updater-'+version+'-'));
    for(const relative of listed.stdout.split('\0').filter(Boolean)){
      if(relative.split('/').some(part=>part==='..'))throw new Error('Unexpected tracked path.');
      const output=path.join(snapshot,relative);mkdirSync(path.dirname(output),{recursive:true});copyFileSync(path.join(root,relative),output);
    }
    const core=path.join(snapshot,'Cargo.toml');
    const coreText=readFileSync(core,'utf8'),coreVersion=coreText.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
    writeFileSync(core,coreText.replace(/^version\s*=\s*"([^"]+)"/m,`version = "${version}"`));
    for(const relative of ['Cargo.lock','desktop/src-tauri/Cargo.lock']){
      const file=path.join(snapshot,relative),body=readFileSync(file,'utf8');
      const expression=new RegExp('name = "ssh_clipboard"\\r?\\nversion = "'+coreVersion.replaceAll('.','\\.')+'"');
      const next=body.replace(expression,`name = "ssh_clipboard"\nversion = "${version}"`);
      if(next===body)throw new Error('Core lockfile anchor missing.');writeFileSync(file,next);
    }
    for(const relative of ['target','desktop/node_modules','desktop/src-tauri/target'])symlinkSync(path.join(root,relative),path.join(snapshot,relative),'junction');
    const embeddedKey=['0.3.4','0.3.5'].includes(version)?keyB:keyA;
    const signingKey=version==='0.3.5'?keyB:keyA;
    const notes=path.join(fixture,'notes-'+version+'.txt');writeFileSync(notes,`Isolated native updater rehearsal ${version}. No production release. Configuration and startup should survive.\n`);
    console.log('Building signed native updater rehearsal '+version);
    const args=[path.join(snapshot,'desktop/scripts/build-installed.mjs'),'--test-build','--version',version,'--public-key-file',embeddedKey+'.pub','--native-test-origin',origin,'--notes-file',notes];
    if(version==='0.3.4')args.push('--transition-from-public-key-file',keyA+'.pub');
    const built=spawnSync(process.execPath,args,{cwd:snapshot,stdio:'inherit',env:{...process.env,TAURI_SIGNING_PRIVATE_KEY:signingKey,TAURI_SIGNING_PRIVATE_KEY_PASSWORD:''}});
    if(built.status!==0)throw new Error('Native updater package staging failed.');
    const stages=path.join(snapshot,'desktop/artifacts');
    const matches=readdirSync(stages).filter(name=>name.startsWith(`TEST-only-installed-${version}-`));
    if(matches.length!==1)throw new Error('Unexpected native package stage.');
    const destination=path.join(fixture,'packages',version);mkdirSync(path.dirname(destination),{recursive:true});
    cpSync(path.join(stages,matches[0]),destination,{recursive:true,errorOnExist:true,force:false});
    const candidate=JSON.parse(readFileSync(path.join(destination,'candidate.json'),'utf8'));
    const evidence=JSON.parse(readFileSync(path.join(destination,'build-evidence.json'),'utf8'));
    if(!evidence.testOnly||evidence.nativeTestOrigin!==origin||evidence.version!==version)throw new Error('Unexpected rehearsal package evidence.');
    const name=path.basename(new URL(candidate.url).pathname);
    config.assets[`download/desktop-v${version}/${name}`]=`packages/${version}/${name}`;
    for(const feed of readdirSync(destination).filter(name=>/^desktop-stable-[a-f0-9]{16}\.json$/.test(name))){
      const route='latest/download/'+feed;config.feeds[route]??={};config.feeds[route][version]=`packages/${version}/${feed}`;
    }
    packages.push({version,installer:path.join(destination,name),source:head,embeddedKeyId:evidence.embeddedPublicKeyId,signingKeyId:evidence.signingPublicKeyId});save();
  }
  writeFileSync(path.join(fixture,'rehearsal.json'),JSON.stringify({testOnly:true,source:head,origin,packages,installed:false,builtinUpdateTested:false},null,2));
  state.nativeUpdaterBootstrap=packages[0].installer;writeFileSync(statePath,JSON.stringify(state,null,2));
  console.log('Native updater rehearsal ready: '+fixture);
  console.log('Bootstrap test installer: '+packages[0].installer);
} catch(error) {
  writeFileSync(controlPath,JSON.stringify({mode:'stop'}));throw error;
}
