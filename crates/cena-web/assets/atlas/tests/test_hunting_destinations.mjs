import test from 'node:test';
import assert from 'node:assert/strict';
import {huntingCatalogue} from '../hunting-catalogue.mjs';
import {huntingSections,hillsArea,darkstoneArea} from '../hunting-sections.mjs';
import {emptyCorrections,makeCorrection,selectionSource,selectionDraft} from '../hunting-corrections.mjs';

function fixture(area,titles,populations){
 const rooms=Object.fromEntries(Object.entries(titles).map(([id,title])=>[id,{id:Number(id),title:[title],exits:[]}]));
 const data={provenance:{hashes:{map:'a'.repeat(64)}},rooms,scenes:{[area]:{label:'Test area',sheet:{rooms:Object.values(rooms)}}}};
 const context={creatures:Object.entries(populations).map(([id,roomIds])=>({id,name:id,level:50,associations:[{group:area,basis:'room_uid_match',roomIds}]}))};
 return {area,data,context};
}
const catalogue=(f,document=emptyCorrections())=>huntingCatalogue(f.data,f.context,document).area(f.area);
const correction=(f,id,rooms)=>makeCorrection(selectionSource(f.data,f.area,huntingSections(f.data,f.context,f.area).hunts.find(h=>h.id===id)),rooms);

test('one named cave is one destination even when its creatures have different matched rooms',()=>{
 const f=fixture(hillsArea,{1:'Smokey Cave, Entrance',2:'Smokey Cave, Tunnel',3:'Smokey Cave, Shrine'}, {fire_cat:[1],fire_rat:[2]});
 const model=catalogue(f);
 assert.equal(model.hunts.length,1,'Creature evidence must not split a named hunting destination');
 assert.equal(model.hunts[0].name,'Smokey Cave');
 assert.deepEqual(model.hunts[0].roomIds,[1,2],'No inferred spawns or automatic inclusion of unknown rooms');
 assert.deepEqual(model.hunts[0].creatures.map(c=>[c.id,c.roomIds]),[['fire_cat',[1]],['fire_rat',[2]]]);
});

test('Dark Cavern combines its tunnels and absorbs a covered fallback without losing saved rooms',()=>{
 const f=fixture(darkstoneArea,{3699:'Darkstone, A Dark Tunnel',3722:'A Dark Cavern',3819:'Darkstone'}, {massive_troll_king:[3699,3819],sheruvian_harbinger:[3722]});
 const document={...emptyCorrections(),records:[correction(f,'dark-tunnel',[3699,3819])]},before=JSON.stringify(document);
 const model=catalogue(f,document);
 assert.equal(model.hunts.length,1,'A saved member room must not also become a competing one-room destination');
 assert.equal(model.hunts[0].name,'Dark Cavern');
 assert.deepEqual(model.hunts[0].roomIds,[3699,3722,3819]);
 assert.deepEqual(model.hunts[0].creatures.map(c=>c.id).sort(),['massive_troll_king','sheruvian_harbinger']);
 assert.equal(JSON.stringify(document),before,'Loading must not rewrite user files');
});

test('canonical edits win over member boundaries and excluded rooms do not respawn as fallback hunts',()=>{
 const f=fixture(darkstoneArea,{3699:'Darkstone, A Dark Tunnel',3722:'A Dark Cavern',3819:'Darkstone'}, {massive_troll_king:[3699,3819],sheruvian_harbinger:[3722]});
 const document={...emptyCorrections(),records:[correction(f,'dark-tunnel',[3699,3819])]};
 const initial=catalogue(f,document).hunts[0];
 const draft=selectionDraft(initial.source,initial.draftRecord);
 assert.deepEqual([...draft.rooms],[3699,3722,3819]);
 draft.edit([3819],'remove');
 document.records.push(draft.record());
 const model=catalogue(f,document);
 assert.equal(model.hunts.length,1);
 assert.deepEqual(model.hunts[0].roomIds,[3699,3722]);
 assert.equal(model.hunts[0].boundaryStatus,'locally-reviewed');
 assert.equal(model.unmatchedCorrections.length,0);
 document.records[1]=makeCorrection(initial.source,[]);
 assert.deepEqual(catalogue(f,document).hunts[0].roomIds,[],'An empty saved boundary is not reset to generated');
});

test('separate named places and unclaimed or partially covered habitat patches remain separate',()=>{
 const f=fixture(hillsArea,{1:'Smokey Cave, Entrance',2:'Blackened Cave, Tunnel',3:'Lysierian Hills',4:'Lysierian Hills'}, {cat:[1,2,3,4]});
 f.data.rooms[3].exits=[{kind:'cardinal',cmd:'east',to:4}];
 const document={...emptyCorrections(),records:[correction(f,'smokey',[1,3])]};
 const model=catalogue(f,document);
 assert.equal(model.hunts.length,3,'Partial overlap or a shared creature is not destination identity');
 assert.ok(model.hunts.some(h=>h.name==='Blackened Cave'));
 assert.deepEqual(model.hunts.find(h=>h.generated).roomIds,[3,4]);
});

test('a fallback with its own saved correction is never silently absorbed',()=>{
 const f=fixture(hillsArea,{1:'Smokey Cave',2:'Elsewhere'}, {cat:[1,2]});
 const fallback=huntingSections(f.data,f.context,f.area).hunts.find(h=>h.generated);
 const document={...emptyCorrections(),records:[correction(f,'smokey',[1,2]),correction(f,fallback.id,[2])]};
 assert.equal(catalogue(f,document).hunts.length,2);
});

test('a canonical-only portable correction suppresses a fallback covered by its added rooms',()=>{
 const f=fixture(darkstoneArea,{3699:'Darkstone, A Dark Tunnel',3722:'A Dark Cavern',3819:'Darkstone'}, {massive_troll_king:[3699,3819],sheruvian_harbinger:[3722]});
 const h=catalogue(f).hunts.find(h=>h.name==='Dark Cavern');
 const document={...emptyCorrections(),records:[makeCorrection(h.source,[3699,3722,3819])]};
 const model=catalogue(f,document);
 assert.equal(model.hunts.length,1);
 assert.deepEqual(model.hunts[0].roomIds,[3699,3722,3819]);
});

test('Dark Lair belongs to the cavern destination even without legacy local corrections',()=>{
 const f=fixture(darkstoneArea,{3699:'Darkstone, A Dark Tunnel',3722:'A Dark Cavern',3819:'Darkstone, Dark Lair'}, {massive_troll_king:[3699,3819],sheruvian_harbinger:[3722]});
 const model=catalogue(f);assert.equal(model.hunts.length,1);
 const h=model.hunts[0];assert.deepEqual(h.roomIds,[3699,3722,3819]);
 const document={...emptyCorrections(),records:[makeCorrection(h.source,[3699,3722])]};
 const edited=catalogue(f,document);assert.equal(edited.hunts.length,1);
 assert.deepEqual(edited.hunts[0].roomIds,[3699,3722]);
});

test('stale member corrections cannot be laundered into an approved destination',()=>{
 const f=fixture(darkstoneArea,{3699:'Darkstone, A Dark Tunnel',3722:'A Dark Cavern',3819:'Darkstone'}, {massive_troll_king:[3699,3819],sheruvian_harbinger:[3722]});
 const record=correction(f,'dark-tunnel',[3699,3819]);record.mapSha256='b'.repeat(64);
 const h=catalogue(f,{...emptyCorrections(),records:[record]}).hunts.find(h=>h.name==='Dark Cavern');
 assert.equal(h.boundaryStatus,'needs-review');
 const draft=selectionDraft(h.source,h.draftRecord,{issues:h.boundaryIssues,rooms:h.reviewRooms});
 assert.ok(draft.needsReview);assert.throws(()=>draft.record(),/Review/);
 draft.review();assert.deepEqual([...draft.rooms],[3699,3722,3819]);
});

test('a shared dungeon panel is not a hunting area: roa\'ters remain separate',()=>{
 const f=fixture(darkstoneArea,{1:'Darkstone, Dungeon',2:'Darkstone, Dungeon'}, {roa_ter:[1],banshee:[2]});
 for(const r of Object.values(f.data.rooms))r.image={file:'wl-darkstone-1717380773.png',rect:[50,800,60,810]};
 assert.equal(catalogue(f).hunts.length,2);
 const records=[correction(f,'dungeon-3:roa_ter',[1,2]),correction(f,'dungeon-3:banshee',[1,2])];
 const merged=catalogue(f,{...emptyCorrections(),records});
 assert.equal(merged.hunts.length,1,'Identical reviewed room sets represent one destination');
 assert.equal(merged.hunts[0].name,'Dungeon · 3rd level');
});
