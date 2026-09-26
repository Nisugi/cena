import assert from 'node:assert/strict';
import {test} from 'node:test';
import {selectionSource,selectionDraft,makeCorrection,assessCorrection,parseCorrections,correctionSchema,correctionStore,correctionStorageKey,rectangleRooms} from '../hunting-corrections.mjs';

const data={provenance:{hashes:{map:'a'.repeat(64)}},scenes:{hills:{sheet:{rooms:[1,2,3,4].map(id=>({id}))}}},rooms:{1:{},2:{},3:{},4:{status:'closed'}}};
const hunt={id:'cave',name:'Cave',roomIds:[1,2],creatures:[{id:'rat'},{id:'cat'}]};
const source=()=>selectionSource(data,'hills',hunt);
const json=records=>JSON.stringify({schema:correctionSchema,records});
function memory(){const values=new Map();return {getItem:k=>values.get(k)??null,setItem:(k,v)=>values.set(k,v)};}

test('delta round trip; source geometry, ownership and habitat records stay untouched',()=>{
 const before=JSON.stringify(data),s=source(),r=makeCorrection(s,[2,3],'reviewed on map');
 assert.deepEqual(r.added,[3]);assert.deepEqual(r.removed,[1]);assert.deepEqual(r.creatures,['cat','rat']);
 const parsed=parseCorrections(json([r])).records[0];assert.deepEqual(assessCorrection(parsed,s).rooms,[2,3]);
 assert.equal(JSON.stringify(data),before);
 assert.throws(()=>makeCorrection(s,[4]),/outside this area or closed/);
 assert.throws(()=>makeCorrection(s,[99]),/outside this area or closed/);
});
test('map, base membership, creature and identity mismatches withhold saved edits',()=>{
 const s=source(),r=makeCorrection(s,[2,3]);
 for(const next of [{...s,mapSha256:'b'.repeat(64)},{...s,baseRooms:[1,3]},{...s,creatures:['zombie']},{...s,hunt:'other'}]){
  const a=assessCorrection(r,next);assert.equal(a.status,'review');assert.deepEqual(a.rooms,next.baseRooms);
 }
 const missing=assessCorrection(r,{...s,allowed:new Set([1,2])});
 assert.deepEqual(missing.missing,[3]);assert.deepEqual(missing.reviewRooms,[2]);
});
test('draft edits support overlap, batch undo, reset, empty membership and notes',()=>{
 const s=source(),a=selectionDraft(s),b=selectionDraft({...s,hunt:'other'});
 a.edit([1,3,3,4,99],'toggle');assert.deepEqual([...a.rooms].sort(),[2,3]);assert.ok(a.dirty);
 assert.deepEqual([...b.rooms],[1,2]);a.undo();assert.deepEqual([...a.rooms],[1,2]);assert.equal(a.dirty,false);
 a.edit([1,2],'remove');assert.equal(a.rooms.size,0);assert.deepEqual(a.record().removed,[1,2]);
 a.reset();assert.deepEqual([...a.rooms],[1,2]);a.undo();assert.equal(a.rooms.size,0);
 a.notes='review';a.saved();assert.equal(a.dirty,false);assert.equal(a.canUndo,false);a.notes='changed';assert.ok(a.dirty);
});
test('changed sources require explicit review followed by save',()=>{
 const s=source(),r=makeCorrection(s,[2,3]),d=selectionDraft({...s,mapSha256:'b'.repeat(64),allowed:new Set([1,2])},r);
 assert.ok(d.needsReview);d.edit([1],'remove');assert.deepEqual([...d.rooms],[1,2]);assert.throws(()=>d.record(),/Review/);
 d.review();assert.deepEqual([...d.rooms],[2]);assert.ok(d.dirty);assert.equal(d.needsReview,false);
 assert.equal(d.record().mapSha256,'b'.repeat(64));d.saved();assert.equal(d.needsReview,false);
});
test('autosave acknowledgement cannot mark newer edits saved and preserves undo',()=>{
 const d=selectionDraft(source());d.edit([1],'remove');const sent=d.record();
 d.edit([3],'add');d.saved(sent,true);assert.ok(d.dirty);assert.ok(d.canUndo);
 d.undo();assert.deepEqual([...d.rooms],[2]);assert.equal(d.dirty,false);
 d.undo();assert.deepEqual([...d.rooms],[1,2]);assert.ok(d.dirty);
});
test('local store retains other selections and rejects stale-tab or failed writes',()=>{
 const storage=memory(),a=correctionStore(storage),b=correctionStore(storage),r=makeCorrection(source(),[2,3]);
 a.save([r]);assert.throws(()=>b.save([r]),/another tab/);
 a.save([makeCorrection({...source(),hunt:'other'},[1,3])]);assert.equal(a.document.records.length,2);
 const fresh=correctionStore(storage);assert.deepEqual(fresh.find('hills','cave'),r);
 storage.setItem=()=>{throw Error('quota');};assert.throws(()=>fresh.save([makeCorrection(source(),[1])]),/quota/);
 assert.deepEqual(fresh.find('hills','cave'),r);
});
test('corrupt local storage and unavailable storage never pretend to save',()=>{
 const storage=memory();storage.setItem(correctionStorageKey,'broken');const store=correctionStore(storage);
 assert.match(store.error,/could not be read/);assert.throws(()=>store.save([]),/untouched/);assert.equal(storage.getItem(correctionStorageKey),'broken');
 assert.throws(()=>correctionStore(null).save([]),/unavailable/);
});
test('import is bounded, typed and cannot smuggle topology or ambiguous IDs',()=>{
 const r=makeCorrection(source(),[2,3]);
 for(const bad of [{...r,exits:[]},{...r,added:[3,3]},{...r,added:['3']},{...r,added:[1]},{...r,removed:[3]},{...r,mapSha256:'unknown'},{...r,creatures:['cat','cat']}])assert.throws(()=>parseCorrections(json([bad])));
 assert.throws(()=>parseCorrections(json([r,r])),/Duplicate/);
 assert.throws(()=>parseCorrections('x'.repeat(5_000_001)),/large/);
 assert.throws(()=>parseCorrections('{bad'));
});
test('rectangle selection respects camera translation, zoom, reverse drag and viewport',()=>{
 const rooms=[{id:1,cell:{x:11,y:21}},{id:2,cell:{x:12,y:22}},{id:3,cell:{x:15,y:24}},{id:4,cell:{x:9,y:20}}];
 const view={x:10,y:20,w:10,h:5},viewport={width:100,height:50};
 assert.deepEqual(rectangleRooms(rooms,{x:25,y:25},{x:5,y:5},view,viewport),[1,2]);
 assert.deepEqual(rectangleRooms(rooms,{x:-100,y:-100},{x:200,y:200},view,viewport),[1,2,3]);
 assert.deepEqual(rectangleRooms(rooms,{x:0,y:0},{x:15,y:15},{...view,w:5},viewport),[]);
});
