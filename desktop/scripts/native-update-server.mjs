// Capability-routed loopback fixture. No production trust, publication or installation.
import http from 'node:http';
import {createReadStream,readFileSync,writeFileSync,appendFileSync,realpathSync,statSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

function json(file) {
  if(statSync(file).size>1024*1024)throw new Error('Oversized fixture state.');
  return JSON.parse(readFileSync(file,'utf8').replace(/^\uFEFF/,''));
}
export async function startFixture(configPath,readyPath) {
  const initial=json(configPath);
  if(initial.testOnly!==true||!/^[0-9a-f]{32}$/.test(initial.token))throw new Error('Explicit test fixture identity required.');
  const root=realpathSync(initial.root);
  const controlPath=path.join(root,'control.json');
  const log=path.join(root,'requests.jsonl');
  const ownedFile=relative=>{
    if(typeof relative!=='string'||path.isAbsolute(relative))throw new Error('Invalid asset.');
    const file=realpathSync(path.join(root,relative));
    const inside=path.relative(root,file);
    if(inside.startsWith('..')||path.isAbsolute(inside)||!statSync(file).isFile())throw new Error('Asset outside fixture.');
    return file;
  };
  const server=http.createServer((request,response)=>{
    try {
      if(request.method!=='GET'||request.url.length>2048){response.writeHead(405).end();return;}
      const prefix='/'+initial.token+'/';
      if(!request.url.startsWith(prefix)){response.writeHead(404).end();return;}
      const route=request.url.slice(prefix.length);
      const config=json(configPath),control=json(controlPath);
      let relative=config.assets?.[route];
      const feed=config.feeds?.[route];
      if(feed)relative=feed[control.targetVersion];
      if(!relative){response.writeHead(404).end();return;}
      const file=ownedFile(relative),size=statSync(file).size;
      appendFileSync(log,JSON.stringify({time:new Date().toISOString(),kind:feed?'feed':'package',route,mode:control.mode,bytes:size})+'\n');
      response.writeHead(200,{'Content-Length':size,'Content-Type':feed?'application/json':'application/octet-stream','Cache-Control':'no-store','Connection':'close'});
      if(feed){response.end(readFileSync(file));return;}
      if(control.mode==='tampered'){
        const bytes=readFileSync(file);bytes[0]^=1;response.end(bytes);return;
      }
      if(control.mode==='interrupted'){
        response.write(readFileSync(file).subarray(0,Math.max(1,Math.floor(size/2))));
        setTimeout(()=>response.destroy(),30);return;
      }
      const stream=createReadStream(file,{highWaterMark:control.mode==='slow'?16384:65536});
      stream.on('error',()=>response.destroy());
      response.on('close',()=>stream.destroy());
      if(control.mode==='slow'){
        let timer;
        stream.on('data',chunk=>{stream.pause();response.write(chunk);timer=setTimeout(()=>stream.resume(),250);});
        stream.on('end',()=>response.end());
        response.on('close',()=>clearTimeout(timer));
      } else stream.pipe(response);
    } catch { if(!response.headersSent)response.writeHead(503);response.end(); }
  });
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
  const origin=`http://127.0.0.1:${server.address().port}/${initial.token}`;
  writeFileSync(readyPath,JSON.stringify({pid:process.pid,origin,testOnly:true},null,2));
  const stop=setInterval(()=>{try{if(json(controlPath).mode==='stop'){server.closeAllConnections();server.close();}}catch{}},500);
  server.on('close',()=>clearInterval(stop));
  return {server,origin};
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  await startFixture(process.argv[2],process.argv[3]);
}
