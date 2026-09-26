// Destination identity is geography, not a creature population. Keep the raw
// populations for evidence and validating old corrections; never rewrite either.
import {darkstoneArea} from './hunting-sections.mjs';
import {selectionSource,assessCorrection,makeCorrection} from './hunting-corrections.mjs';

const sorted=ids=>[...new Set(ids)].sort((a,b)=>a-b);
function creaturesOf(hunts){
 const creatures=new Map();
 for(const h of hunts)for(const c of h.creatures){
  const prior=creatures.get(c.id);
  creatures.set(c.id,{...c,roomIds:sorted([...(prior?.roomIds||[]),...c.roomIds])});
 }
 return [...creatures.values()].sort((a,b)=>a.id.localeCompare(b.id));
}

export function huntingDestinations(data,area,raw,records){
 const buckets=new Map();
 for(const h of raw){
  // Author-confirmed destination: cavern + adjoining dark/winding tunnels.
  // Deep Mist is separate. This is NOT a global merge by monster or map name.
  const darkLair=h.generated&&h.source.baseRooms.length>0&&h.source.baseRooms.every(id=>
   (data.rooms[id]?.title?.[0]||'').replace(/^\[|\]$/g,'')==='Darkstone, Dark Lair');
  const cavern=area===darkstoneArea&&(['dark-tunnel','dark-cavern'].includes(h.placeId)||darkLair);
  // A sheet panel may contain distinct hunts (e.g. roa'ters vs. banshees).
  // Only exact, reviewed membership can join those; a floor is not a hunt.
  const key=cavern?'destination:dark-cavern':!h.generated&&!h.panel?'place:'+h.placeId:
   h.panel&&h.boundaryStatus==='locally-reviewed'?'reviewed:'+h.placeId+':'+h.roomIds.join(','):'hunt:'+h.id;
  if(!buckets.has(key))buckets.set(key,[]);
  buckets.get(key).push(h);
 }
 const destinations=[...buckets].map(([key,parts])=>{
  if(parts.length===1&&!key.startsWith('destination:'))return parts[0];
  const first=parts[0],id=key.startsWith('destination:')?key:'destination:'+first.placeId+
   (first.panel?':'+parts.map(h=>h.id).sort().join('|'):'');
  const name=id==='destination:dark-cavern'?'Dark Cavern':first.place;
  const creatures=creaturesOf(parts),levels=creatures.map(c=>c.level).filter(Number.isFinite);
  const base={...first,id,name,mapName:undefined,place:name,creatures,generated:false,
   roomIds:sorted(parts.flatMap(h=>h.source.baseRooms)),
   level:levels.length?`${Math.min(...levels)}${Math.min(...levels)===Math.max(...levels)?'':'–'+Math.max(...levels)}`:null,
   evidence:'Named hunting destination. Creature matches remain room-specific; selecting or adding a room does not assert that every listed creature spawns there.'};
  const source=selectionSource(data,area,base),record=records.find(r=>r.area===area&&r.hunt===id);
  const inheritedIssues=parts.flatMap(h=>h.boundaryIssues.map(issue=>h.name+': '+issue));
  const inheritedRooms=sorted(parts.flatMap(h=>h.roomIds));
  // A newly combined destination has not itself been reviewed. Valid member
  // corrections carry forward, but do not certify the new union as reviewed.
  // A saved canonical boundary wins, including explicit removals and emptiness.
  const assessment=record?assessCorrection(record,source):null;
  const roomIds=assessment?.rooms||inheritedRooms;
  const issues=assessment?.issues||inheritedIssues;
  const carried=parts.some(h=>h.record);
  const draftRecord=record||makeCorrection(source,roomIds,
   parts.filter(h=>h.record?.notes).map(h=>h.name+': '+h.record.notes).join('\n').slice(0,4000));
  return {...base,source,record:record||null,draftRecord,roomIds,
   boundaryStatus:issues.length?'needs-review':record?'locally-reviewed':'generated',boundaryIssues:issues,
   inheritedCorrections:!record&&carried,
   reviewRooms:assessment?.reviewRooms||sorted(parts.flatMap(h=>h.reviewRooms||h.roomIds)),
   memberHunts:parts.map(h=>h.id),placeIds:[...new Set(parts.map(h=>h.placeId))],
   // Preserve grouping identity after a canonical edit excludes a member room.
   // An exclusion changes the hunt boundary, not that room's named place.
   destinationFootprint:sorted([...source.baseRooms,...inheritedRooms,...roomIds])};
 });
 // A generated fragment completely covered by ONE named destination is
 // evidence within that destination, not another choice. Never merge on name,
 // proximity, partial overlap, or when a fragment has its own saved boundary.
 const absorbed=new Map();
 for(const h of destinations){
  if(!h.generated||h.record||!h.roomIds.length)continue;
  const parents=destinations.filter(p=>p!==h&&!p.generated&&p.boundaryStatus!=='needs-review'&&
   h.roomIds.every(id=>(p.destinationFootprint||p.roomIds).includes(id))&&h.creatures.every(c=>p.creatures.some(other=>other.id===c.id)));
  if(parents.length!==1)continue;
  const parent=parents[0];absorbed.set(h.id,parent.id);
  parent.creatures=creaturesOf([parent,h]);
 }
 const hunts=destinations.filter(h=>!absorbed.has(h.id));
 const known=new Set(hunts.flatMap(h=>[h.id,...(h.memberHunts||[])]));
 const sectionsHidden=new Set(hunts.flatMap(h=>h.placeIds||[]));
 return {hunts,known,sectionsHidden};
}
