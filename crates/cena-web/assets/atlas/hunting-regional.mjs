// Conservative presentation fallback. Names do not assign ownership or floors;
// connectivity only separates local habitat patches, never supplies new exits.
const palette=['#6bc6b4','#cd9a87','#b5a0ed','#89a9eb','#c2bd7d','#dc9ab5','#73bed1','#89b590'];
export function habitatColor(key){let hash=0;for(const c of key)hash=(Math.imul(hash,31)+c.charCodeAt(0))>>>0;return palette[hash%palette.length];}
export function regionalPlaceRules(data,scene,excluded=new Set()){
 const places=new Map();
 for(const {id} of scene.sheet.rooms){
  if(excluded.has(id))continue;
  const title=(data.rooms[id]?.title?.[0]||'').replace(/^\[|\]$/g,'').trim();
  if(!title)continue;
  const name=title.split(',')[0].trim(),key=name.replace(/^The /i,'').toLowerCase();
  if(!places.has(key))places.set(key,{names:new Set(),roomIds:[]});
  const place=places.get(key);place.names.add(name);place.roomIds.push(id);
 }
 return [...places].sort(([a],[b])=>a.localeCompare(b)).map(([key,p])=>({
  id:'title:'+encodeURIComponent(key),name:[...p.names].sort()[0],roomIds:p.roomIds.sort((a,b)=>a-b),
  color:habitatColor(key),provisional:true,generated:true,labelEligible:p.roomIds.length>=3
 }));
}
// Weak connectivity is used for drawing patches, not for travel reachability.
// Only recorded plain local movement (or one explicit scripted compass move)
// can join rooms. Opaque scripts, transport and unmatched intermediaries do not.
export function habitatPatches(data,roomIds){
 const ids=new Set(roomIds),neighbors=new Map([...ids].map(id=>[id,new Set()]));
 for(const id of ids)for(const e of data.rooms[id]?.exits||[]){
  if(!ids.has(e.to)||'pass' in e||e.routine)continue;
  let cmd=e.cmd;
  if(e.kind==='scripted'){
   const step=e.steps?.length===1?e.steps[0]:null;
   if(!step||Object.keys(step).length!==1||typeof step.move!=='string'||!/^(?:n|ne|e|se|s|sw|w|nw|u|d|north|northeast|east|southeast|south|southwest|west|northwest|up|down)$/i.test(step.move))continue;
   cmd=step.move;
  }else if(!['cardinal','vertical','out','go','climb','other'].includes(e.kind))continue;
  if(typeof cmd!=='string'||!/^(?:n|ne|e|se|s|sw|w|nw|u|d|north|northeast|east|southeast|south|southwest|west|northwest|up|down|out|(?:go|climb|crawl|enter|swim) [\w' -]+)$/i.test(cmd.trim()))continue;
  if(/\b(?:portal|teleport|caravan|urchin|ferry|boat|ship|transport)\b/i.test(cmd))continue;
  neighbors.get(id).add(e.to);neighbors.get(e.to).add(id);
 }
 const remaining=new Set([...ids].sort((a,b)=>a-b)),patches=[];
 for(const seed of remaining){
  remaining.delete(seed);const patch=[seed];
  for(let i=0;i<patch.length;i++)for(const next of neighbors.get(patch[i]))if(remaining.delete(next))patch.push(next);
  patches.push(patch.sort((a,b)=>a-b));
 }
 return patches;
}
