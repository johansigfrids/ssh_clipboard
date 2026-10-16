import test from 'node:test';
import assert from 'node:assert/strict';
import {nativeOrigin} from '../scripts/native-update-config.mjs';
test('native transport requires isolated release tests and refuses inherited-production-like origins',()=>{
  const valid='http://127.0.0.1:41234/0123456789abcdef0123456789abcdef';
  assert.equal(nativeOrigin(undefined,false,false),undefined);
  assert.equal(nativeOrigin(valid,true,false),valid);
  assert.throws(()=>nativeOrigin(valid,false,false));
  assert.throws(()=>nativeOrigin(valid,true,true));
  for(const bad of [valid.replace('127.0.0.1','localhost'),valid+'/',valid+'?x=1',valid.replace('41234','80'),valid.replace('41234','99999'),valid.replace('http:','https:'),valid.replace('41234','041234')])assert.throws(()=>nativeOrigin(bad,true,false));
});
