// Human hunting selections are overlays, never room, exit or habitat edits.
export const correctionSchema='hydra-hunting-corrections-v1';
export const correctionStorageKey='hydra.hunting.corrections.v1';
export const maxCorrectionBytes=5_000_000;
const sorted=ids=>[...new Set(ids)].sort((a,b)=>a-b);
const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
export const selectionKey=(area,hunt)=>JSON.stringify([area,hunt]);
export const emptyCorrections=()=>({schema:correctionSchema,records:[]});

function fields(value,keys){
 if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!keys.includes(k)))throw Error('Unexpected correction fields');
}
function text(value,max){if(typeof value!=='string'||value.length>max)throw Error('Invalid correction text');return value;}
function ids(value){
 if(!Array.isArray(value)||value.length>40000||value.some(id=>!Number.isInteger(id)||id<0||id>4294967295)||new Set(value).size!==value.length)throw Error('Invalid or duplicate room IDs');
 return sorted(value);
}
export function parseCorrections(raw){
 if(typeof raw!=='string'||raw.length>maxCorrectionBytes||new TextEncoder().encode(raw).length>maxCorrectionBytes)throw Error('Correction file is too large');
 const input=JSON.parse(raw);fields(input,['schema','records']);
 if(input.schema!==correctionSchema||!Array.isArray(input.records)||input.records.length>2000)throw Error('Unsupported correction document');
 const keys=new Set();
 const records=input.records.map(r=>{
  fields(r,['area','hunt','name','mapSha256','baseRooms','creatures','added','removed','notes']);
  const area=text(r.area,500),hunt=text(r.hunt,2000),name=text(r.name,500),mapSha256=text(r.mapSha256,64),notes=text(r.notes,4000);
  if(!area||!hunt||!name||!/^[a-f0-9]{64}$/.test(mapSha256))throw Error('Missing selection identity or map fingerprint');
  if(!Array.isArray(r.creatures)||r.creatures.length>500||r.creatures.some(c=>typeof c!=='string'||c.length>500)||new Set(r.creatures).size!==r.creatures.length)throw Error('Invalid creature identities');
  const baseRooms=ids(r.baseRooms),added=ids(r.added),removed=ids(r.removed),base=new Set(baseRooms);
  if(added.some(id=>base.has(id))||removed.some(id=>!base.has(id)))throw Error('Invalid membership delta');
  const key=selectionKey(area,hunt);if(keys.has(key))throw Error('Duplicate hunting selection');keys.add(key);
  return {area,hunt,name,mapSha256,baseRooms,creatures:[...r.creatures].sort(),added,removed,notes};
 });
 return {schema:correctionSchema,records};
}
export function selectionSource(data,area,hunt){
 const allowed=new Set((data.scenes[area]?.sheet.rooms||[]).filter(r=>data.rooms[r.id]&&data.rooms[r.id].status!=='closed').map(r=>r.id));
 return {area,hunt:hunt.id,name:hunt.name,mapSha256:data.provenance?.hashes?.map,
  baseRooms:sorted(hunt.roomIds.filter(id=>allowed.has(id))),creatures:[...new Set(hunt.creatures.map(c=>String(c.id)))].sort(),allowed};
}
export function assessCorrection(record,source){
 if(!record)return {status:'base',rooms:[...source.baseRooms],issues:[],missing:[]};
 const issues=[];
 if(record.area!==source.area||record.hunt!==source.hunt)issues.push('Selection identity changed');
 if(record.mapSha256!==source.mapSha256)issues.push('Map fingerprint changed');
 if(!same(record.baseRooms,source.baseRooms))issues.push('Generated room membership changed');
 if(!same(record.creatures,source.creatures))issues.push('Creature identities changed');
 const removed=new Set(record.removed),intended=sorted([...record.baseRooms.filter(id=>!removed.has(id)),...record.added]);
 const missing=intended.filter(id=>!source.allowed.has(id));
 if(missing.length)issues.push('Selected rooms are missing, closed or no longer on this area map');
 return {status:issues.length?'review':'saved',issues,missing,rooms:issues.length?[...source.baseRooms]:intended,
  // For explicit human review only, never silently applied on a new snapshot.
  reviewRooms:intended.filter(id=>source.allowed.has(id))};
}
export function makeCorrection(source,rooms,notes=''){
 const chosen=sorted(rooms),base=new Set(source.baseRooms),set=new Set(chosen);
 if(chosen.some(id=>!source.allowed.has(id)))throw Error('Selection contains rooms outside this area or closed rooms');
 const r={area:source.area,hunt:source.hunt,name:source.name,mapSha256:source.mapSha256,
  baseRooms:[...source.baseRooms],creatures:[...source.creatures],added:chosen.filter(id=>!base.has(id)),removed:source.baseRooms.filter(id=>!set.has(id)),notes};
 return parseCorrections(JSON.stringify({schema:correctionSchema,records:[r]})).records[0];
}
export function correctionStore(storage){
 let raw=null,document=emptyCorrections(),error='';
 try{raw=storage?.getItem(correctionStorageKey)??null;if(raw)document=parseCorrections(raw);}
 catch(e){error='Local corrections could not be read: '+e.message;}
 return {
  get error(){return error;},get document(){return structuredClone(document);},
  find(area,hunt){const r=document.records.find(r=>r.area===area&&r.hunt===hunt);return r?structuredClone(r):undefined;},
  save(records){
   if(error)throw Error(error+'; existing storage was left untouched.');
   if(!storage)throw Error('Browser storage unavailable; export your working copy instead');
   if(storage.getItem(correctionStorageKey)!==raw)throw Error('Corrections changed in another tab. Export your work, then reload before saving.');
   const merged=new Map(document.records.map(r=>[selectionKey(r.area,r.hunt),r]));
   for(const r of records)merged.set(selectionKey(r.area,r.hunt),r);
   const next=parseCorrections(JSON.stringify({schema:correctionSchema,records:[...merged.values()]}));
   const nextRaw=JSON.stringify(next);storage.setItem(correctionStorageKey,nextRaw);
   // A failed write must never look like a successful save.
   document=next;raw=nextRaw;return structuredClone(next);
  }
 };
}

export function selectionDraft(source,record,review=null){
 const assessment=assessCorrection(record,source);
 if(review?.issues.length){assessment.status='review';assessment.issues=review.issues;assessment.reviewRooms=review.rooms;}
 let rooms=new Set(assessment.rooms),notes=record?.notes||'',reviewed=false;
 let baseline={rooms:sorted(rooms),notes},undo=[];
 function change(next){const before=sorted(rooms);if(same(before,sorted(next)))return;undo.push(before);if(undo.length>100)undo.shift();rooms=new Set(next);}
 return {
  source,assessment,get rooms(){return new Set(rooms);},get notes(){return notes;},set notes(value){notes=value;},
  get dirty(){return reviewed||!same(baseline,{rooms:sorted(rooms),notes});},get canUndo(){return undo.length>0;},
  get needsReview(){return assessment.status==='review'&&!reviewed;},
  edit(ids,mode){if(this.needsReview)return;const next=new Set(rooms);for(const id of new Set(ids)){if(!source.allowed.has(id))continue;if(mode==='remove'||mode==='toggle'&&next.has(id))next.delete(id);else if(mode==='add'||mode==='toggle')next.add(id);}change(next);},
  undo(){if(undo.length)rooms=new Set(undo.pop());},
  reset(){if(!this.needsReview)change(source.baseRooms);},
  review(){change(assessment.reviewRooms||source.baseRooms);reviewed=true;},
  record(){if(this.needsReview)throw Error('Review changed source data before saving');return makeCorrection(source,rooms,notes);},
  saved(record=null,keepUndo=false){baseline=record?{rooms:sorted([...record.baseRooms.filter(id=>!record.removed.includes(id)),...record.added]),notes:record.notes}:{rooms:sorted(rooms),notes};reviewed=false;assessment.status='saved';if(!keepUndo)undo=[];}
 };
}

// Rectangle selection is screen-space, so zoom and translated cameras agree.
export function rectangleRooms(rooms,a,b,view,viewport){
 const scale=viewport.width/view.w,minX=Math.max(0,Math.min(a.x,b.x)),maxX=Math.min(viewport.width,Math.max(a.x,b.x));
 const minY=Math.max(0,Math.min(a.y,b.y)),maxY=Math.min(viewport.height,Math.max(a.y,b.y));
 return rooms.filter(r=>{const x=(r.cell.x-view.x)*scale,y=(r.cell.y-view.y)*scale;return x>=minX&&x<=maxX&&y>=minY&&y<=maxY;}).map(r=>r.id);
}
