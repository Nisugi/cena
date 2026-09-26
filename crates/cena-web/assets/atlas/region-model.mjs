// Reference indexes; scene membership remains owned by the imported map.
export function regionModel(data,context=null){
 const membership=new Map(),creatures=new Map(),byArea=new Map();
 for(const [key,s] of Object.entries(data.scenes))for(const r of s.sheet.rooms){
  if(membership.has(r.id))throw Error(`Duplicate scene room ${r.id}`);
  membership.set(r.id,key);
 }
 for(const c of context?.creatures||[]){
  const rooms=new Set(),groups=new Map();
  for(const a of c.associations){
   for(const id of a.roomIds){if(membership.get(Number(id))!==a.group)throw Error(`Stale habitat association ${id}`);rooms.add(Number(id));}
   const ids=groups.get(a.group)||new Set();for(const id of a.roomIds)ids.add(Number(id));groups.set(a.group,ids);
  }
  for(const [group,ids] of groups){const list=byArea.get(group)||[];list.push({...c,roomIds:[...ids]});byArea.set(group,list);}
  creatures.set(c.id,{...c,rooms});
 }
 const areas=Object.entries(data.scenes).map(([key,s])=>{
  const roomIds=s.sheet.rooms.map(r=>r.id),refs=byArea.get(key)||[];
  const exits=roomIds.flatMap(from=>data.rooms[from].exits.filter(e=>membership.get(e.to)!==key).map(edge=>({from,to:edge.to,edge,area:membership.get(edge.to)||null})));
  // A regional browse should start on the native street/outdoor focus when
  // present, not whichever cellar happened to serialize first. Not a claim
  // that this room is an entrance, accessible, safe, or a hunt start.
  const live=id=>data.rooms[id]?.status!=='closed';
  const entryRoom=s.units.find(u=>u.kind==='Streets')?.rooms.find(live)??roomIds.find(live)??roomIds[0];
  return {key,label:s.label,roomIds,entryRoom,provisional:s.assignment_kind==='region-only',creatures:refs,exits,
   search:[s.label,...roomIds.flatMap(id=>[id,...(data.rooms[id].title||[])]),...refs.map(c=>c.name)].join(' ').toLowerCase()};
 }).sort((a,b)=>Number(a.provisional)-Number(b.provisional)||a.label.localeCompare(b.label));
 return {areas,creatures,membership,byArea,context,
  search({query='',kind='assigned',min=null,max=null}={}){
   const q=query.trim().toLowerCase();
   return areas.filter(a=>(kind!=='assigned'||!a.provisional)&&(kind!=='provisional'||a.provisional)&&
    (!q||a.search.includes(q))&&(kind!=='habitat'||a.creatures.length)&&
    ((min===null&&max===null)||a.creatures.some(c=>Number.isFinite(c.level)&&(min===null||c.level>=min)&&(max===null||c.level<=max))));
  },
  habitat(id,area){return new Set((byArea.get(area)||[]).find(c=>c.id===id)?.roomIds||[]);}
 };
}
