import assert from 'node:assert/strict';
import fs from 'node:fs';
import {test} from 'node:test';
import {creaturesForRegion} from './creatures.mjs';
import {regionModel} from '../../crates/cena-web/assets/atlas/region-model.mjs';
import {huntingSections,darkstoneArea} from '../../crates/cena-web/assets/atlas/hunting-sections.mjs';

const entry=(id,name,uids=[])=>({id,record:{name,areas:[{uids}],level:55},source:{path:'synthetic'}});
const catalogue=entries=>({entries,failures:[],source:{path:'synthetic'}});
const match=(entries,rooms)=>creaturesForRegion(catalogue(entries),rooms,rooms.map(r=>({id:String(r.id),group:'area'})));

test('exact room tags recover creatures without UIDs, without guessing adjacent rooms',()=>{
 const result=match([entry('steed','nightmare steed')],[
  {id:1,tags:['nightmare steed','nightmare steed']},
  {id:2,tags:[' NIGHTMARE STEED ']},
  {id:3,tags:[],exits:[{to:1}]},
  {id:4,tags:['no nightmare steed','nightmare steeds','nightmare steed skull','bounty:nightmare steed']},
 ]);
 assert.equal(result.creatures.length,1);
 assert.deepEqual(result.creatures[0].associations.map(a=>[a.basis,a.roomIds]),[['room_tag_match',['1','2']]]);
 assert.equal(result.coverage.catalogue.recordsWithoutHabitatUids,1);
 assert.equal(result.coverage.matchedRooms,2);
 assert.equal(result.coverage.roomsWithoutAssociation,2);
});

test('UID and tag evidence stays separate, but coverage and browser counts are deduplicated',()=>{
 const rooms=[{id:1,uid:[10],tags:['nightmare steed']},{id:2,uid:[11],tags:[]},{id:3,tags:['nightmare steed']}];
 const result=match([entry('steed','nightmare steed',[10,11])],rooms);
 const c=result.creatures[0];
 assert.deepEqual(c.associations.map(a=>[a.basis,a.roomIds]),[
  ['room_uid_match',['1','2']],['room_tag_match',['1','3']]]);
 assert.equal(result.coverage.matchedRooms,3);
 assert.equal(result.coverage.groups[0].creatures,1);
 assert.equal(result.coverage.groups[0].matchedRooms,3);
 const data={rooms:Object.fromEntries(rooms.map(r=>[r.id,{...r,exits:[]}])),scenes:{area:{label:'Test',sheet:{rooms},units:[]}}};
 const model=regionModel(data,{creatures:[{...c.record,id:c.id,associations:c.associations}]});
 assert.equal(model.byArea.get('area').length,1);
 assert.deepEqual([...model.habitat('steed','area')],[1,2,3]);
});

test('ambiguous normalized names cannot produce tag matches; UID evidence remains usable',()=>{
 const result=match([entry('one','steed',[10]),entry('two',' STEED ')],[{id:1,uid:[10],tags:['steed']}]);
 assert.deepEqual(result.creatures.map(c=>c.id),['one']);
 assert.deepEqual(result.creatures[0].associations.map(a=>a.basis),['room_uid_match']);
});

test('real Darkstone tags reach hunting presentation without filling untagged Deep Mist rooms',()=>{
 const read=p=>JSON.parse(fs.readFileSync(new URL(p,import.meta.url),'utf8'));
 const data=read('../../crates/cena-web/atlas-data/landing/data.json');
 const display=Object.entries(data.scenes).flatMap(([group,s])=>s.sheet.rooms.map(r=>({id:String(r.id),group})));
 const result=creaturesForRegion(read('./creature-catalogue.json'),Object.values(data.rooms),display);
 const creatures=result.creatures.map(c=>({...c.record,id:c.id,associations:c.associations}));
 const steed=creatures.find(c=>c.id==='nightmare_steed');
 assert.ok(steed,'nightmare steed must survive the no-habitat-UID case');
 assert.equal(new Set(steed.associations.flatMap(a=>a.roomIds)).size,11);
 const hunts=huntingSections(data,{creatures},darkstoneArea);
 const mist=hunts.hunts.find(h=>h.id==='mist');
 assert.ok(mist);
 assert.deepEqual(mist.creatures.find(c=>c.id==='nightmare_steed').roomIds,[3817,3818]);
 assert.equal(mist.level,'55–63');
 assert.equal(hunts.byRoom.get(22230).creatures.find(c=>c.id==='nightmare_steed').roomIds.includes(22230),false);
 assert.deepEqual(hunts.hunts.find(h=>h.id==='dungeon-3:roa_ter').roomIds,
  [7047,7048,7049,7050,7051,7052,7053,7054,7055,7056,7057,7058,7059,7060,7061,7062,7063,7064,7065,7066,7067,7068,7069]);
 const banshee=creatures.find(c=>c.id==='banshee');
 assert.equal(banshee.associations.filter(a=>a.group===darkstoneArea).flatMap(a=>a.roomIds).length,32);
});
