import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {worldModel} from '../world-model.mjs';
const data=JSON.parse(readFileSync(new URL('../../../atlas-data/corpus/world.json',import.meta.url))),before=JSON.stringify(data);
for(const special of [false,true])for(const buckets of [false,true]){
 const m=worldModel(data,{special,buckets}),ids=new Set(m.nodes.map(n=>n.id));
 assert.ok(m.nodes.every(n=>buckets||n.geographic));
 for(const l of m.links){
  const source=data.links.find(p=>p.id===l.id);
  assert.deepEqual(l.records,source.records.filter(r=>special||!r.special));
  assert.ok(ids.has(l.a)&&ids.has(l.b));assert.equal(l.ab+l.ba,l.records.length);
  assert.equal(l.ab,l.records.filter(r=>r.source===l.a).length);
 }
}
assert.equal(JSON.stringify(data),before);
const fixture={nodes:[{id:'a',geographic:true},{id:'transport',geographic:false},{id:'b',geographic:true}],links:[
 {id:'a|transport',a:'a',b:'transport',records:[{source:'a',target:'transport',special:false}]},
 {id:'b|transport',a:'b',b:'transport',records:[{source:'transport',target:'b',special:false}]},
]};
assert.equal(worldModel(fixture).links.length,0,'Hiding a bucket does not invent a connection through it');
assert.equal(worldModel(fixture,{buckets:true}).links[0].ba,0,'One-way records do not gain return edges');
console.log('PASS: world filters preserve exact records, directed counts, input data and no synthetic bucket shortcuts.');
