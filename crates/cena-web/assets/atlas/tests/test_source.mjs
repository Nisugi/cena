import assert from 'node:assert/strict';
import {offlineSource} from '../offline-source.mjs';
const text=JSON.stringify({rooms:{1:{id:1}},scenes:{}});
const bytes=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text));
const hash=Buffer.from(bytes).toString('hex');
const load=sidecar=>offlineSource(new URL('http://localhost/atlas/landing/'),{
 fetcher:async url=>String(url).endsWith('/data.json')?new Response(text):new Response(JSON.stringify(sidecar)),
}).load();
const good={schema:'hydra-region-context-v1',data_sha256:hash,creatures:[]};
assert.deepEqual((await load(good)).regionContext,good);
for(const bad of [{...good,data_sha256:'stale'},{...good,schema:'future'}]){
 const result=await load(bad);assert.equal(result.regionContext,null);assert.match(result.contextWarning,/do not match/);
 assert.equal(result.data.rooms[1].id,1,'Reference failure never hides the map itself');
}
const failure=offlineSource(new URL('http://localhost/'),{fetcher:async()=>new Response('',{status:404})});
await assert.rejects(failure.load(),/Cannot load/);
console.log('PASS: exact hash binding, stale/unknown sidecars withheld, missing data fails visibly.');
