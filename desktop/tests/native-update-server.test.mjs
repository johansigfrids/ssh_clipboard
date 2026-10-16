import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {startFixture} from '../scripts/native-update-server.mjs';
test('loopback fixture serves only declared owned assets and produces bounded network faults',async()=>{
  const root=mkdtempSync(path.join(os.tmpdir(),'ssh-clipboard-native-test-'));
  let server;
  try {
    const bytes=Buffer.alloc(128*1024,42);writeFileSync(path.join(root,'package.exe'),bytes);
    writeFileSync(path.join(root,'feed.json'),'{"version":"0.3.3"}');
    const configPath=path.join(root,'server.json'),controlPath=path.join(root,'control.json');
    const config={testOnly:true,root,token:'0123456789abcdef0123456789abcdef',assets:{'download/package.exe':'package.exe'},feeds:{'latest/feed.json':{'0.3.3':'feed.json'}}};
    writeFileSync(configPath,JSON.stringify(config));
    const control=mode=>writeFileSync(controlPath,JSON.stringify({targetVersion:'0.3.3',mode}));
    control('valid');
    const started=await startFixture(configPath,path.join(root,'ready.json'));server=started.server;
    const url=started.origin+'/download/package.exe';
    assert.equal((await fetch(started.origin+'/latest/feed.json')).status,200);
    assert.equal((await fetch(started.origin+'/unknown')).status,404);
    assert.equal((await fetch(started.origin.replace(config.token,'wrong')+'/download/package.exe')).status,404);
    assert.equal((await fetch(url,{method:'POST'})).status,405);
    assert.deepEqual(Buffer.from(await (await fetch(url)).arrayBuffer()),bytes);
    control('tampered');const changed=Buffer.from(await (await fetch(url)).arrayBuffer());assert.equal(changed.length,bytes.length);assert.notDeepEqual(changed,bytes);
    control('interrupted');await assert.rejects(async()=>{await (await fetch(url)).arrayBuffer();});
    control('slow');await assert.rejects(async()=>{await (await fetch(url,{signal:AbortSignal.timeout(100)})).arrayBuffer();});
    config.assets['download/package.exe']='../outside';writeFileSync(configPath,JSON.stringify(config));assert.equal((await fetch(url)).status,503);
  } finally {
    if(server){server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
    const resolved=path.resolve(root);if(path.dirname(resolved)!==path.resolve(os.tmpdir())||!path.basename(resolved).startsWith('ssh-clipboard-native-test-'))throw new Error('Unexpected fixture cleanup path.');
    rmSync(resolved,{recursive:true,force:true});
  }
});
