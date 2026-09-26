// Generic artwork-coordinate composition. Never classifies rooms or invents
// exits. Each image is an independent coordinate frame, not a global floor.
const dist=(a,b)=>Math.hypot(a.x-b.x,a.y-b.y);
const key=(a,b)=>[a,b].sort((a,b)=>a-b).join(':');
const compass={north:[0,-1],south:[0,1],east:[1,0],west:[-1,0],northeast:[1,-1],northwest:[-1,-1],southeast:[1,1],southwest:[-1,1]};
export function imagePoint(record){
 const image=record?.image,r=image?.rect;
 if(!image?.file||!r||r.length!==4||!r.every(Number.isFinite)||r[0]<0||r[1]<0||r[2]<=r[0]||r[3]<=r[1])return null;
 return {file:image.file,x:(r[0]+r[2])/2,y:(r[1]+r[3])/2};
}
function extent(rooms){
 const xs=rooms.map(r=>r.cell.x),ys=rooms.map(r=>r.cell.y);
 return {left:Math.min(...xs),right:Math.max(...xs),top:Math.min(...ys),bottom:Math.max(...ys)};
}
function freeNear(origin,occupied,separation){
 if(occupied.every(p=>dist(p,origin)>=separation))return origin;
 for(let ring=1;ring<=occupied.length+1;ring++)for(let i=0;i<ring*8;i++){
  const theta=i/(ring*8)*Math.PI*2,p={x:origin.x+Math.cos(theta)*ring*separation,y:origin.y+Math.sin(theta)*ring*separation};
  if(occupied.every(o=>dist(o,p)>=separation))return p;
 }
 throw Error('Reference inset placement exhausted');
}
function alignScene(original,records){
 const points=new Map(original.sheet.rooms.map(r=>[r.id,imagePoint(records[r.id])]).filter(([,p])=>p));
 if(!points.size)return original;
 const scene=structuredClone(original),byId=new Map(scene.sheet.rooms.map(r=>[r.id,r]));
 const panels=[],adjusted=[],insets=[],files=[...new Set([...points.values()].map(p=>p.file))].sort();
 let cursor=0;
 for(const file of files){
  const members=scene.sheet.rooms.filter(r=>points.get(r.id)?.file===file).sort((a,b)=>a.id-b.id);
  const originals=members.map(r=>points.get(r.id));
  const nearest=originals.map((p,i)=>Math.min(...originals.filter((q,j)=>i!==j&&dist(p,q)>1).map(q=>dist(p,q)))).filter(Number.isFinite).sort((a,b)=>a-b);
  const scale=4/(nearest[Math.floor(nearest.length/2)]||40);
  const offset={x:cursor-Math.min(...originals.map(p=>p.x))*scale,y:-Math.min(...originals.map(p=>p.y))*scale};
  for(const r of members){const p=points.get(r.id);r.cell={x:p.x*scale+offset.x,y:p.y*scale+offset.y};r.reference_frame=file;r.reference_kind='artwork';r.display_position_source=`Recorded artwork rectangle: ${file}; uniformly scaled display frame`;}
  // Protect every unique reference location first. Only conflicting markers
  // move, so a crowded/shared marker cannot push an entire map out of shape.
  const protectedPoints=members.map(r=>({...r.cell})),settled=[];
  for(const r of members){
   if(settled.some(p=>dist(p,r.cell)<1.25)){
    r.cell=freeNear(r.cell,[...protectedPoints,...settled],1.5);adjusted.push(r.id);
    r.display_position_source=`Schematic separation of shared/overlapping artwork marker: ${file}`;
   }
   settled.push(r.cell);
  }
  const b=extent(members);panels.push({file,scale,offset,rooms:members.map(r=>r.id)});cursor=b.right+12;
 }
 // Rooms without artwork positions get explicit connected schematic insets.
 // Preserve their native relative shape where possible, but do not pretend
 // their coordinates belong to any artwork's reference frame.
 const missing=new Set(scene.sheet.rooms.filter(r=>!points.has(r.id)).map(r=>r.id)),adj=new Map([...missing].map(id=>[id,new Set()]));
 for(const id of missing)for(const e of records[id].exits)if(missing.has(e.to)){adj.get(id).add(e.to);adj.get(e.to).add(id);}
 const base=extent(scene.sheet.rooms.filter(r=>points.has(r.id)));
 let insetX=base.left,shelfY=base.bottom+12,rowHeight=0;
 while(missing.size){
  const seed=Math.min(...missing),ids=[seed];missing.delete(seed);
  for(let i=0;i<ids.length;i++)for(const next of adj.get(ids[i]))if(missing.delete(next))ids.push(next);
  const members=ids.sort((a,b)=>a-b).map(id=>byId.get(id)),b=extent(members),occupied=[];
  const width=(b.right-b.left)*3;
  if(insetX>base.left&&insetX+width>Math.max(base.right,base.left+24)){insetX=base.left;shelfY+=rowHeight+8;rowHeight=0;}
  for(const r of members){
   const desired={x:insetX+(r.cell.x-b.left)*3,y:shelfY+(r.cell.y-b.top)*3};
   r.cell=freeNear(desired,occupied,2.5);occupied.push(r.cell);
   r.reference_frame=`inset:${seed}`;r.reference_kind='schematic';r.display_position_source='Schematic connected inset: no usable artwork coordinate';
  }
  const placed=extent(members);rowHeight=Math.max(rowHeight,placed.bottom-shelfY);
  insets.push({rooms:ids,name:`Unmapped detail · ${records[seed].title?.[0]||'#'+seed}`});insetX=placed.right+8;
 }
 // Rebuild drawing edges solely from recorded exits. Old solver edge geometry
 // cannot survive changing coordinate frames, but topology is unchanged.
 const drawings=new Map(),conflicts=[];
 for(const r of scene.sheet.rooms)for(const [ordinal,e] of records[r.id].exits.entries()){
  const target=byId.get(e.to);if(!target)continue;
  const pair=key(r.id,target.id),v=compass[e.cmd],dx=target.cell.x-r.cell.x,dy=target.cell.y-r.cell.y;
  const cross=!!r.reference_frame&&!!target.reference_frame&&r.reference_frame!==target.reference_frame;
  const conflict=!cross&&!!v&&((v[0]&&Math.sign(dx)!==v[0])||(v[1]&&Math.sign(dy)!==v[1]));
  if(conflict)conflicts.push({from:r.id,to:target.id,command:e.cmd});
  const record={from:r.id,to:target.id,ordinal,edge:e};
  if(drawings.has(pair)){drawings.get(pair).records.push(record);continue;}
  drawings.set(pair,{a_room:r.id,b_room:target.id,a:{...r.cell},b:{...target.cell},unit:r.unit===target.unit?r.unit:null,
   kind:cross?'Continuation':v&&!conflict&&r.reference_frame===target.reference_frame?'Directional':'Connector',
   records:[record],independent_frames:cross,inset_link:cross&&(r.reference_kind==='schematic'||target.reference_kind==='schematic'),important_connector:cross&&r.reference_kind==='artwork'&&target.reference_kind==='artwork'});
 }
 scene.sheet.edges=[...drawings.values()];
 scene.reference_layout={kind:'artwork-aligned',panels,anchored:points.size,adjusted,insets,bearing_conflicts:conflicts,
  note:'Each artwork is a separate display frame. Uniform image geometry is preserved except marked shared-coordinate adjustments. Missing coordinates are schematic insets. Exits, focus units and exported ownership are unchanged.'};
 scene.display_sections=[...panels.map(p=>({name:`Artwork · ${p.file}`,rooms:p.rooms})),...(insets.length?[{name:`Unmapped detail · ${insets.reduce((n,s)=>n+s.rooms.length,0)} rooms`,rooms:insets.flatMap(s=>s.rooms)}]:[])];
 return scene;
}
export function alignReferenceScenes(data){
 const scenes={...data.scenes};
 for(const [key,scene] of Object.entries(scenes)){
  // Existing explicitly documented adapters already own these scenes. No
  // place-name/room-ID exceptions are used to opt other areas into alignment.
  if(scene.town_alignment||scene.display_layout)continue;
  scenes[key]=alignScene(scene,data.rooms);
 }
 return {...data,scenes};
}
