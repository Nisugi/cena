// Presentation-only places, checked against named reference sheets and current
// first room titles. These are NOT new native areas, hunt bounds or plate edits.
import {regionalPlaceRules,habitatPatches,habitatColor} from './hunting-regional.mjs';
export const hillsArea='area-wehnimers.landing.lysierian.hills';
export const gatesArea='area-wehnimers.landing.outside.gates';
export const darkstoneArea='area-wehnimers.landing.darkstone.castle';
const prefix=name=>t=>t===name||t.startsWith(name+', ');
const hillsRules=[
 {id:'smokey',name:'Smokey Cave',color:'#6bc6b4',matches:prefix('Smokey Cave')},
 {id:'blackened',name:'Blackened Cave',color:'#b5a0ed',matches:prefix('Blackened Cave')},
 {id:'mine',name:'Abandoned Mine',color:'#dc9ab5',matches:prefix('Abandoned Mine')},
 {id:'monastery',name:'Monastery',color:'#89a9eb',matches:prefix('Monastery')},
 {id:'shores',name:"Shores of Lough Ne’halin",color:'#73bed1',matches:prefix("Shores of Lough Ne'halin")},
 {id:'road',name:'Old Noralgar Road',color:'#a9ba7d',matches:prefix('Old Noralgar Road')},
 {id:'lake',name:'Underground Lake',color:'#7d99c8',matches:prefix('Underground Lake')},
 {id:'marliese',name:'Lake Marliese',color:'#9ecac0',matches:t=>t==='Lysierian Hills, Lake Marliese'},
 {id:'village',name:'Ruined village',color:'#cd9a87',matches:t=>/^Lysierian Hills, (Ruined Village|Store(?: Porch| Bedroom)?|Storeroom|Great Room|Shack|Cottage|Bedroom|Saloon(?: Kitchen)?)$/.test(t),provisional:true},
 {id:'woods',name:'Wooded paths',color:'#89b590',matches:t=>prefix('Lysierian Hills')(t),habitatAny:['black_bear','great_boar'],provisional:true},
 {id:'gryphon',name:'Gryphon Holding',color:'#a9aebd',matches:prefix('Gryphon Holding')}
];
const gatesRules=[
 {id:'forest',name:'Lower Dragonsclaw · Forest',color:'#89b590',matches:t=>t==='Lower Dragonsclaw, Forest'},
 {id:'hills',name:'Lower Dragonsclaw · Wooded Hills',color:'#a9ba7d',matches:t=>t==='Lower Dragonsclaw, Wooded Hills'},
 {id:'grasslands',name:'Lower Dragonsclaw · Grasslands',color:'#c2bd7d',matches:t=>t==='Lower Dragonsclaw, Grasslands'},
 {id:'log-field',name:'Log Field',color:'#bb9d80',matches:t=>t==='Lower Dragonsclaw, Log Field'},
 {id:'exterior',name:'Town approaches',color:'#8cb8c9',matches:t=>/^Wehnimer's, (Exterior|Outside Gate)$/.test(t)||prefix("Wehnimer's Exterior")(t),provisional:true},
 {id:'trollfang',name:'Upper Trollfang',color:'#c5a081',matches:prefix('Upper Trollfang')},
 {id:'badlands',name:'Badlands',color:'#d19b8a',matches:prefix('Badlands')},
 {id:'colossus',name:'Colossus',color:'#a7a0ce',matches:prefix('Colossus')},
 {id:'reach',name:'Lake Eonak & the Shadow',color:'#73bed1',matches:prefix("Melgorehn's Reach")},
 {id:'sunfist',name:'Guardians of Sunfist',color:'#b8a0e3',matches:prefix('Guardians of Sunfist')},
 {id:'wedding',name:'Wedding Glade',color:'#dc9ab5',matches:prefix('Wedding Glade')}
];
const darkstoneImage='wl-darkstone-1717380773.png';
// These are bounded, version-specific source-sheet panels, NOT inferred z
// coordinates. Require a dungeon title and a fully enclosed native rectangle.
const dungeonPanel=(x0,y0,x1,y1)=>(t,r)=>{
 const rect=r.image?.rect;
 return /^Darkstone, (Dungeon|Tunnel|Outer Chamber)$/.test(t)&&r.image?.file===darkstoneImage&&
  Array.isArray(rect)&&rect.length===4&&rect.every(Number.isFinite)&&
  rect[0]<=rect[2]&&rect[1]<=rect[3]&&rect[0]>=x0&&rect[1]>=y0&&rect[2]<=x1&&rect[3]<=y1;
};
const darkstoneRules=[
 {id:'bleaklands',name:'Bleaklands · Storm Plains',color:'#a9ba7d',matches:t=>t==='Bleaklands, Storm Plains'},
 {id:'nightmare',name:'Nightmare Cavern & shrine',color:'#b5a0ed',matches:t=>t==='Bleaklands, Nightmare Cavern'||t==='Shrine of Nightmares'},
 {id:'approach',name:'Forest & gorge approach',color:'#89b590',matches:t=>prefix('Forest Path')(t)||t==='Forest Clearing'||prefix('Gorge')(t),provisional:true},
 {id:'road',name:'Derelict Road & plateau',color:'#bb9d80',matches:t=>/^Darkstone, (Derelict Road|Plateau|Siege Tower)$/.test(t)},
 {id:'waterfall',name:'Waterfall caverns',color:'#73bed1',matches:t=>t==='Cavern'||t==='Behind the Waterfall',provisional:true},
 {id:'ward',name:'Outer Ward',color:'#c2bd7d',matches:t=>t==='Castle Darkstone, Outer Ward'},
 {id:'battlements',name:'Parapets & guardtowers',color:'#89a9eb',matches:t=>/^Castle Darkstone, (Guardtower|Parapet)$/.test(t)},
 {id:'keep',name:'Inner Keep & Great Hall',color:'#dc9ab5',matches:t=>/^Castle Darkstone, (Inner Keep|Great Hall|Stables|Crypt)$/.test(t)},
 {id:'towers',name:'Castle towers',color:'#a7a0ce',matches:t=>/^Castle Darkstone, (Tower|Tower's Top)$/.test(t)},
 {id:'dungeon-1',name:'Dungeon · 1st level',color:'#6bc6b4',matches:dungeonPanel(850,280,1160,700),panel:true},
 {id:'dungeon-2',name:'Dungeon · 2nd level',color:'#b5a0ed',matches:dungeonPanel(630,800,1130,1200),panel:true},
 {id:'dungeon-3',name:'Dungeon · 3rd level',color:'#cd9a87',matches:dungeonPanel(40,740,450,1310),panel:true},
 {id:'dark-tunnel',name:'Dark & winding tunnels',color:'#9ecac0',matches:t=>/^Darkstone, (A Dark Tunnel|Winding Tunnel)$/.test(t)},
 {id:'dark-cavern',name:'Dark Cavern',color:'#7d99c8',matches:t=>t==='A Dark Cavern'||t==='Darkstone, Burrow'},
 {id:'mist',name:'Deep Mist',color:'#b9aac9',matches:t=>t==='A Deep Mist'}
];
const definitions=new Map([
 [hillsArea,{rules:hillsRules,reference:'wl-lysierian-1264234799.png'}],
 [gatesArea,{rules:gatesRules,reference:'wl-gates-1264234799.png'}],
 [darkstoneArea,{rules:darkstoneRules,reference:darkstoneImage}]
]);
export function huntingSections(data,context,area=hillsArea){
 const scene=data.scenes[area],definition=definitions.get(area);
 const empty={area,sections:[],byRoom:new Map(),hunts:[],byHuntRoom:new Map()};
 if(!scene)return empty;
 const hasPresentationRegion=!!data.presentation&&Object.prototype.hasOwnProperty.call(data.presentation,'region');
 const inPresentationRegion=hasPresentationRegion&&scene.region===data.presentation.region;
 const assigned=!!scene.assignment_kind&&scene.assignment_kind!=='region-only'&&scene.sheet.rooms.length>0;
 if(!definition&&!inPresentationRegion&&!assigned&&scene.region!=="Wehnimer's Landing")return empty;
 const member=new Set(scene.sheet.rooms.map(r=>r.id));
 const refs=(context?.creatures||[]).flatMap(c=>{
  const ids=c.associations.filter(a=>a.group===area&&a.basis==='room_uid_match').flatMap(a=>a.roomIds.map(Number)).filter(id=>member.has(id));
  return ids.length?[{...c,roomIds:new Set(ids)}]:[];
 }).sort((a,b)=>a.id.localeCompare(b.id));
 // A missing or stale habitat sidecar never turns a generic service/town
 // frame into a hunting view. Recognizable area and transition names remain
 // available through the ordinary atlas and region overview.
 if(!definition&&!refs.length)return empty;
 const sections=[],byRoom=new Map();
 function add(rule){
  const roomIds=rule.roomIds||scene.sheet.rooms.map(r=>r.id).filter(id=>{
   const t=(data.rooms[id].title?.[0]||'').replace(/^\[|\]$/g,'');
   return rule.matches(t,data.rooms[id])&&(!rule.habitatAny||refs.some(c=>rule.habitatAny.includes(c.id)&&c.roomIds.has(id)));
  }).sort((a,b)=>a-b);
  if(!roomIds.length)return;
  const creatures=refs.flatMap(c=>{const ids=roomIds.filter(id=>c.roomIds.has(id));return ids.length?[{...c,roomIds:ids}]:[];});
  const levels=creatures.map(c=>c.level).filter(Number.isFinite),matched=new Set(creatures.flatMap(c=>c.roomIds));
  const section={id:rule.id,name:rule.name,color:rule.color,provisional:!!rule.provisional,generated:!!rule.generated,labelEligible:rule.labelEligible!==false,roomIds,creatures,
   level:levels.length?`${Math.min(...levels)}${Math.min(...levels)===Math.max(...levels)?'':'–'+Math.max(...levels)}`:null,
   unknown:roomIds.filter(id=>!matched.has(id)),
   evidence:rule.generated?'Provisional place label from current room-title prefixes within this exported display frame. Not an approved area, floor or hunting boundary.':rule.panel?`Source-sheet level from ${definition.reference}, matched by dungeon title and native image rectangle. Presentation only: not a solved elevation or new plate; source coordinates still need human review.`:
    rule.provisional?'Provisional descriptive grouping from current titles and/or habitat UID matches; not an official hunting boundary.':`Named place from current first room titles; naming reference: ${definition.reference}. Extent is title-matched, not an approved hunting boundary.`};
  for(const id of roomIds){if(byRoom.has(id))throw Error(`Overlapping place rules at room ${id}`);byRoom.set(id,section);}
  sections.push(section);
 }
 for(const rule of definition?.rules||[])add(rule);
 // Preserve reviewed pilot rules; fill only previously uncovered rooms.
 const excluded=new Set(byRoom.keys());
 for(const rule of regionalPlaceRules(data,scene,excluded)){
  // Pilots retain neutral labels already reviewed; fallback there adds only
  // places with known habitats. New regional maps also get quiet place names.
  if(definition&&!rule.roomIds.some(id=>refs.some(c=>c.roomIds.has(id))))continue;
  add(rule);
 }
 // Geography/PNG panels only provide naming context. Hunting membership is
 // the exact union of recorded habitat matches, never the whole named panel.
 // Merge creature populations ONLY when they share a matched room (transitive
 // co-occurrence). Merely being adjacent, on one floor or similar in level is
 // insufficient. These are reference overlays, not automated hunt selections.
 const hunts=[],byHuntRoom=new Map(),splitColors=['#6bc6b4','#cd9a87','#b5a0ed','#89a9eb'];
 // New areas use title groups for labels, not hunt boundaries. A population
 // does not become forty separate hunts because a castle has forty room names.
 const generated=sections.filter(s=>s.generated),generatedIds=new Set(generated.flatMap(s=>s.roomIds));
 const populationSources=sections.filter(s=>!s.generated);
 if(generated.length)populationSources.push({id:'habitat',name:scene.label,generated:true,provisional:true,
  roomIds:[...generatedIds],creatures:refs.flatMap(c=>{const roomIds=[...c.roomIds].filter(id=>generatedIds.has(id)).sort((a,b)=>a-b);return roomIds.length?[{...c,roomIds}]:[];})});
 for(const s of populationSources){
  const remaining=new Set(s.creatures.map(c=>c.id)),groups=[];
  for(const c of s.creatures){
   if(!remaining.delete(c.id))continue;
   const creatures=[c],rooms=new Set(c.roomIds);
   for(let i=0;i<creatures.length;i++)for(const other of s.creatures){
    if(remaining.has(other.id)&&other.roomIds.some(id=>rooms.has(id))){remaining.delete(other.id);creatures.push(other);for(const id of other.roomIds)rooms.add(id);}
   }
   const patches=s.generated?habitatPatches(data,[...rooms]):[[...rooms].sort((a,b)=>a-b)];
   for(const roomIds of patches){const ids=new Set(roomIds);groups.push({roomIds,creatures:creatures.flatMap(c=>{
    const matched=c.roomIds.filter(id=>ids.has(id));return matched.length?[{...c,roomIds:matched}]:[];
   })});}
  }
  for(const [i,g] of groups.entries()){
   const multiple=groups.length>1,levels=g.creatures.map(c=>c.level).filter(Number.isFinite);
   const names=g.creatures.map(c=>c.name.replace(/^./,s=>s.toUpperCase()));
   const places=new Map();for(const id of g.roomIds){const name=byRoom.get(id)?.name||s.name;places.set(name,(places.get(name)||0)+1);}
   const ranked=[...places].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0]));
   const place=s.generated&&ranked[0][1]>=g.roomIds.length/2?ranked[0][0]:s.name;
   const population=g.creatures.map(c=>c.id).sort().join('+');
   const briefNames=names.join(' / ').length>42?names[0]+` +${names.length-1} types`:names.join(' / ');
   const h={...s,...g,id:s.generated?s.id+':'+population+':'+g.roomIds[0]:multiple?s.id+':'+population:s.id,
    name:s.generated?`${place} · ${names.join(' / ')}`:multiple?names.join(' / '):s.name,place,
    mapName:s.generated?`${place} · ${briefNames}`:undefined,
    color:s.generated?habitatColor(place+':'+population):multiple?splitColors[i%splitColors.length]:s.color,
    unknown:[],level:levels.length?`${Math.min(...levels)}${Math.min(...levels)===Math.max(...levels)?'':'–'+Math.max(...levels)}`:null,
    evidence:'Exact installed habitat matches; populations combine only through shared matched rooms. '+(s.generated?'Separated into locally connected matched-room patches (plain exits or a single recorded compass move); one-way links do not imply return travel. ':'')+'Not a guaranteed spawn, safe area or approved hunting selection. Place context: '+place};
   hunts.push(h);for(const id of h.roomIds)byHuntRoom.set(id,h);
  }
 }
 const duplicates=new Map();for(const h of hunts){const list=duplicates.get(h.name)||[];list.push(h);duplicates.set(h.name,list);}
 for(const list of duplicates.values())if(list.length>1)for(const [i,h] of list.entries()){
  h.name+=` · patch ${i+1}`;if(h.mapName)h.mapName+=` · ${i+1}`;
 }
 return {area,sections,byRoom,hunts,byHuntRoom};
}
