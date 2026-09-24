// Map-specific, evidence-backed display alignment. The native export and room
// graph remain immutable; this does not claim to fix the upstream solver.
import {alignTown} from './town-layout.mjs';
import {alignReferenceScenes} from './reference-layout.mjs';
export function preparePresentation(raw,{classic=true,layout=raw.presentation?.layout_policy}={}){
 // New-town pilots never inherit Landing's geographic adapters. Native mode
 // preserves the supplied scene byte-for-byte; safe edge drawing is separate.
 if(raw.presentation)return layout==='reference'?alignReferenceScenes(raw):raw;
 const base=alignTown(classic?readableCatacombs(raw):raw);
 const aligned=alignReferenceScenes(base);
 // The original catacomb comparison must remain the original geometry.
 if(!classic)aligned.scenes.catacombs=raw.scenes.catacombs;
 return packDisplayGroups(aligned);
}
const IMAGE='wl-catacombs-1264234799.png',SCALE=6.75;
const compass={north:[0,-1],south:[0,1],east:[1,0],west:[-1,0],northeast:[1,-1],northwest:[-1,-1],southeast:[1,1],southwest:[-1,1]};
export const pairKey=(a,b)=>[a,b].sort((a,b)=>a-b).join(':');
const distance=(a,b)=>Math.sqrt((a.x-b.x)**2+(a.y-b.y)**2);
function segmentIntersectsBox(a,b,box){
 let lo=0,hi=1;
 for(const axis of ['x','y']){
  const delta=b[axis]-a[axis],min=box[axis],max=min+box[axis==='x'?'w':'h'];
  if(Math.abs(delta)<1e-9){if(a[axis]<min||a[axis]>max)return false;continue;}
  const s=(min-a[axis])/delta,t=(max-a[axis])/delta;lo=Math.max(lo,Math.min(s,t));hi=Math.min(hi,Math.max(s,t));
  if(lo>hi)return false;
 }return true;
}
// Automatic rigid placement of building groups. Keep the street skeleton
// fixed and search nearest free space; no room-specific offsets or IDs.
export function packDisplayGroups(input){
 const output={...input,scenes:{...input.scenes}};
 for(const [area,original] of Object.entries(input.scenes)){
  if(original.reference_layout)continue; // Never repack rooms within artwork.
  if(!original.units.some(u=>u.kind==='Building'))continue;
  const scene=structuredClone(original),moves=[],unresolved=[],margin=scene.town_alignment?2.3:1.25;
  const byId=new Map(scene.sheet.rooms.map(r=>[r.id,r]));
  const streetEdges=scene.sheet.edges.filter(e=>byId.get(e.a_room)?.unit===0&&byId.get(e.b_room)?.unit===0);
  const offsets=[];
  for(let dx=-24;dx<=24;dx++)for(let dy=-24;dy<=24;dy++)offsets.push({x:dx,y:dy});
  // Distance first; ties prefer down before sideways/up, without hardcoding
  // any particular building or making a geographic bearing claim.
  offsets.sort((a,b)=>(a.x*a.x+a.y*a.y)-(b.x*b.x+b.y*b.y)||(a.y<0)-(b.y<0)||Math.abs(a.x)-Math.abs(b.x)||b.y-a.y||a.x-b.x);
  for(const [unit,u] of scene.units.entries()){
   if(u.kind!=='Building')continue;
   const members=scene.sheet.rooms.filter(r=>r.unit===unit),ids=new Set(members.map(r=>r.id));if(!members.length)continue;
   const minX=Math.min(...members.map(r=>r.cell.x)),maxX=Math.max(...members.map(r=>r.cell.x)),minY=Math.min(...members.map(r=>r.cell.y)),maxY=Math.max(...members.map(r=>r.cell.y));
   const others=scene.sheet.rooms.filter(r=>!ids.has(r.id));
   const externalEdges=scene.sheet.edges.filter(e=>!ids.has(e.a_room)&&!ids.has(e.b_room));
   const free=offset=>{
    const box={x:minX+offset.x-margin,y:minY+offset.y-margin,w:maxX-minX+margin*2,h:maxY-minY+margin*2};
    if(others.some(r=>r.cell.x>=box.x&&r.cell.x<=box.x+box.w&&r.cell.y>=box.y&&r.cell.y<=box.y+box.h))return false;
    // A street cannot cut through the footprint, even between its rooms.
    if(streetEdges.some(e=>segmentIntersectsBox(byId.get(e.a_room).cell,byId.get(e.b_room).cell,box)))return false;
    return members.every(r=>externalEdges.every(e=>segmentDistance({x:r.cell.x+offset.x,y:r.cell.y+offset.y},byId.get(e.a_room).cell,byId.get(e.b_room).cell)>margin));
   };
   if(free({x:0,y:0}))continue;
   const offset=offsets.find(free);
   if(!offset){unresolved.push({unit,name:u.name,reason:'No free rigid placement within search radius; renderer must use safe connectors.'});continue;}
   for(const r of members){r.cell={x:r.cell.x+offset.x,y:r.cell.y+offset.y};r.display_position_source=[r.display_position_source,'automatic rigid group placement around fixed street obstacles'].filter(Boolean).join('; ');}
   moves.push({unit,name:u.name,offset,room_ids:members.map(r=>r.id)});
  }
  for(const e of scene.sheet.edges){e.a={...byId.get(e.a_room).cell};e.b={...byId.get(e.b_room).cell};delete e.points;}
  scene.group_placement={rule:'Building footprints avoid streets; rigid translations preserve internal geometry. No ownership or exit changes.',moves,unresolved};output.scenes[area]=scene;
 }
 return output;
}
export function segmentDistance(p,a,b){
 const dx=b.x-a.x,dy=b.y-a.y,t=Math.max(0,Math.min(1,((p.x-a.x)*dx+(p.y-a.y)*dy)/(dx*dx+dy*dy||1)));
 return Math.hypot(p.x-a.x-t*dx,p.y-a.y-t*dy);
}
function clear(points,obstacles,clearance=1.15){
 const limit=clearance*clearance;
 for(let i=1;i<points.length;i++){
  const a=points[i-1],b=points[i],dx=b.x-a.x,dy=b.y-a.y,length=dx*dx+dy*dy||1;
  const left=Math.min(a.x,b.x)-clearance,right=Math.max(a.x,b.x)+clearance,top=Math.min(a.y,b.y)-clearance,bottom=Math.max(a.y,b.y)+clearance;
  for(const p of obstacles){
   if(p.x<left||p.x>right||p.y<top||p.y>bottom)continue;
   const t=Math.max(0,Math.min(1,((p.x-a.x)*dx+(p.y-a.y)*dy)/length)),x=p.x-a.x-t*dx,y=p.y-a.y-t*dy;
   if(x*x+y*y<=limit)return false;
  }
 }
 return true;
}
function connector(a,b,obstacles,clearance=1.15){
 // Every candidate includes its endpoints. If either is already too close
 // to another room, no dogleg/visibility search can possibly make it safe.
 const limit=clearance*clearance;
 if(obstacles.some(p=>(p.x-a.x)**2+(p.y-a.y)**2<=limit||(p.x-b.x)**2+(p.y-b.y)**2<=limit))return null;
 const candidates=[[a,b],[a,{x:a.x,y:b.y},b],[a,{x:b.x,y:a.y},b]];
 const gap=Math.max(4,clearance*2+1);
 const xs=[a.x-gap,a.x+gap,b.x-gap,b.x+gap,Math.min(a.x,b.x)-gap*2,Math.max(a.x,b.x)+gap*2];
 const ys=[a.y-gap,a.y+gap,b.y-gap,b.y+gap,Math.min(a.y,b.y)-gap*2,Math.max(a.y,b.y)+gap*2];
 for(const x of xs)candidates.push([a,{x,y:a.y},{x,y:b.y},b]);
 for(const y of ys)candidates.push([a,{x:a.x,y},{x:b.x,y},b]);
 const score=p=>p.slice(1).reduce((sum,b,i)=>sum+distance(p[i],b),0)+(p.length-2)*2;
 const simple=candidates.filter(p=>clear(p,obstacles,clearance)).sort((a,b)=>score(a)-score(b))[0];
 if(simple)return simple;
 // A two-bend dogleg can fail in a street with obstacles on both sides.
 // Bounded visibility search adds waypoints around nearby blockers, without
 // treating them as rooms or changing the graph. Every segment is checked
 // against ALL rooms, including obstacles outside this candidate subset.
 const blockers=obstacles.map(p=>({p,d:segmentDistance(p,a,b)})).filter(p=>p.d<gap*2).sort((a,b)=>a.d-b.d).slice(0,12);
 const radius=(clearance+.05)/Math.cos(Math.PI/8),nodes=[a,b];
 for(const {p} of blockers)for(let i=0;i<8;i++){
  const q={x:p.x+Math.cos(i*Math.PI/4)*radius,y:p.y+Math.sin(i*Math.PI/4)*radius};
  if(obstacles.every(o=>(q.x-o.x)**2+(q.y-o.y)**2>limit))nodes.push(q);
 }
 // Euclidean distance is an admissible, consistent lower bound for this
 // visibility graph. A* avoids exploring every detour behind the destination.
 const costs=nodes.map(()=>Infinity),remaining=nodes.map(p=>distance(p,b)),prev=[],done=new Set();costs[0]=0;
 let visibilityChecks=0;
 while(done.size<nodes.length){
  let at=-1;for(let i=0;i<nodes.length;i++)if(!done.has(i)&&(at<0||costs[i]+remaining[i]<costs[at]+remaining[at]))at=i;
  if(at<0||!Number.isFinite(costs[at]))return null;
  if(at===1){const path=[];for(let i=1;i!==undefined;i=prev[i])path.unshift(nodes[i]);return path;}
  done.add(at);
  for(let i=0;i<nodes.length;i++){
   if(done.has(i))continue;
   const cost=costs[at]+distance(nodes[at],nodes[i])+.1;
   if(cost>=costs[i])continue;
   // Interactive drawing is bounded work, not an exhaustive routing solver.
   // Keep the exact native passage as a clickable continuation when a dense
   // visibility graph exceeds this budget. Never draw an unchecked shortcut.
   if(++visibilityChecks>256)return null;
   if(!clear([nodes[at],nodes[i]],obstacles,clearance))continue;
   costs[i]=cost;prev[i]=at;
  }
 }
 return null;
}
// Hard drawing invariant at the current screen scale, for every map. A room
// not named by an edge must never look like an intermediate stop on that edge.
// Both ordinary lines and route animation consume this SAME safe geometry.
const bridgeCache=new WeakMap();
function importantBridges(scene,records){
 if(bridgeCache.get(scene)?.records===records)return bridgeCache.get(scene).important;
 const ids=new Set(scene.sheet.rooms.map(r=>r.id)),adj=new Map([...ids].map(id=>[id,new Set()]));
 for(const id of ids)for(const e of records[id].exits)if(ids.has(e.to)&&id!==e.to){adj.get(id).add(e.to);adj.get(e.to).add(id);}
 const seen=new Map(),low=new Map(),size=new Map(),important=new Set();let time=0;
 function walk(id,parent,pending){
  seen.set(id,++time);low.set(id,time);size.set(id,1);
  for(const next of adj.get(id)){
   if(next===parent)continue;
   if(seen.has(next)){low.set(id,Math.min(low.get(id),seen.get(next)));continue;}
   walk(next,id,pending);size.set(id,size.get(id)+size.get(next));low.set(id,Math.min(low.get(id),low.get(next)));
   if(low.get(next)>seen.get(id))pending.push({pair:pairKey(id,next),count:size.get(next)});
  }
 }
 for(const id of ids)if(!seen.has(id)){
  const pending=[];walk(id,null,pending);
  for(const edge of pending)if(Math.min(edge.count,size.get(id)-edge.count)>=12)important.add(edge.pair);
 }
 // Undirected bridges identify drawing importance only, never traversability
 // or a return exit. Threshold avoids promoting short dead-end spokes.
 bridgeCache.set(scene,{records,important});return important;
}
// Scene and record snapshots are immutable. Cache graph preparation separately
// from scale-dependent geometry; changing scene OR record identity invalidates.
const drawingInputs=new WeakMap();
function prepareDrawing(scene,records){
 const previous=drawingInputs.get(scene);if(previous?.records===records)return previous;
 const byId=new Map(scene.sheet.rooms.map(r=>[r.id,r])),seen=new Set();
 const bridges=importantBridges(scene,records);
 const candidates=new Map();for(const e of scene.sheet.edges){const key=pairKey(e.a_room,e.b_room);if(!candidates.has(key))candidates.set(key,e);}
 for(const room of scene.sheet.rooms)for(const e of records[room.id].exits){
  const target=byId.get(e.to);if(!target)continue;
  const key=pairKey(room.id,e.to);
  if(!candidates.has(key))candidates.set(key,{a_room:room.id,b_room:e.to,a:room.cell,b:target.cell,kind:'Connector',unit:room.unit===target.unit?room.unit:null,supplemented:true});
 }
 const entries=[];
 for(const e of candidates.values()){
  const key=pairKey(e.a_room,e.b_room);if(seen.has(key))continue;seen.add(key);
  const links=e.records||[e.a_room,e.b_room].flatMap(id=>(records[id]?.exits||[]).flatMap((edge,ordinal)=>edge.to===(id===e.a_room?e.b_room:e.a_room)?[{from:id,to:edge.to,ordinal,edge}]:[]));
  const obstacles=scene.sheet.rooms.filter(r=>r.id!==e.a_room&&r.id!==e.b_room).map(r=>r.cell);
  entries.push({edge:{...e,records:links,important_connector:!!e.important_connector||bridges.has(key)},obstacles});
 }
 const prepared={records,entries,scales:new Map()};drawingInputs.set(scene,prepared);return prepared;
}
export function obstacleSafeScene(scene,records,clearance){
 const prepared=prepareDrawing(scene,records),cache=prepared.scales;
 // Exact clearances only: a cache hit never relaxes the current pixel rule.
 if(cache.has(clearance)){const value=cache.get(clearance);cache.delete(clearance);cache.set(clearance,value);return value;}
 const edges=[];
 for(const {edge:e,obstacles} of prepared.entries){
  let points=e.points||[e.a,e.b];
  if(e.kind==='Continuation')points=null;
  else if(!clear(points,obstacles,clearance))points=connector(e.a,e.b,obstacles,clearance);
  edges.push({...e,points,kind:points?(e.kind==='Stub'?'Connector':e.kind):'Continuation',clearance});
 }
 const result={...scene,sheet:{...scene.sheet,edges}};cache.set(clearance,result);
 if(cache.size>8)cache.delete(cache.keys().next().value);
 return result;
}
// Camera changes may reuse a MORE conservative clearance, never a smaller
// one. Limit excess padding to 20% so zooming in restores useful detail.
// The first frame uses exact scale; subsequent cold scales round UP to a
// bounded 9.1% band. No path is temporarily drawn with stale unsafe clearance.
export function cameraDrawing(scene,records,clearance){
 const cache=prepareDrawing(scene,records).scales;
 let best=Infinity;for(const scale of cache.keys())if(scale>=clearance&&scale<=clearance*1.2&&scale<best)best=scale;
 if(Number.isFinite(best))return obstacleSafeScene(scene,records,best);
 const scale=cache.size?Math.max(clearance,2**(Math.ceil(Math.log2(clearance)*8)/8)):clearance;
 return obstacleSafeScene(scene,records,scale);
}
export function readableCatacombs(input){
 const original=input.scenes.catacombs;if(!original)return input;
 const scene=structuredClone(original),placed=new Map(),unplaced=[];
 for(const room of scene.sheet.rooms){
  const image=input.rooms[room.id]?.image,rect=image?.rect;
  if(image?.file===IMAGE&&rect?.length===4&&rect.every(Number.isFinite)){
   room.cell={x:(rect[0]+rect[2])/2/SCALE,y:(rect[1]+rect[3])/2/SCALE};
   room.display_position_source='recorded classic-map rectangle';placed.set(room.id,room);
  }else unplaced.push(room);
 }
 // This adapter is explicitly for this well-covered reference sheet. Refuse
 // to guess a layout for a different or sparsely mapped future room set.
 if(placed.size<190||unplaced.length>4)return input;
 const anchoredCount=placed.size,insets=[];
 for(const room of unplaced){
  const neighbors=[...placed.values()].filter(n=>(input.rooms[room.id].exits||[]).some(e=>e.to===n.id)||(input.rooms[n.id].exits||[]).some(e=>e.to===room.id));
  const anchor=neighbors[0]?.cell||{x:Math.min(...[...placed.values()].map(r=>r.cell.x)),y:Math.max(...[...placed.values()].map(r=>r.cell.y))+8};
  let cell;
  for(const radius of [5,8,12,16]){
   for(const [dx,dy] of [[0,-1],[1,0],[0,1],[-1,0],[1,1],[-1,-1]]){
    const p={x:anchor.x+radius*dx,y:anchor.y+radius*dy};
    if([...placed.values()].every(r=>distance(p,r.cell)>=3)){cell=p;break;}
   }if(cell)break;
  }
  if(!cell)return input;
  room.cell=cell;room.display_position_source='schematic inset beside recorded connection, not surveyed position';placed.set(room.id,room);insets.push(room.id);
 }
 const drawings=new Map(),conflicts=[],continuations=[];
 for(const source of scene.sheet.rooms)for(const [ordinal,exit] of input.rooms[source.id].exits.entries()){
  const target=placed.get(exit.to);if(!target)continue;
  const key=pairKey(source.id,target.id),a=source.cell,b=target.cell,v=compass[exit.cmd];
  const dx=b.x-a.x,dy=b.y-a.y;
  const conflict=!!v&&((v[0]&&Math.sign(dx)!==v[0])||(v[1]&&Math.sign(dy)!==v[1])||(!v[0]&&Math.abs(dx)>1.5)||(!v[1]&&Math.abs(dy)>1.5));
  if(conflict)conflicts.push({from:source.id,to:target.id,command:exit.cmd,reason:'Recorded bearing disagrees with classic-map placement; shown as a non-spatial connection.'});
  const entry={from:source.id,to:target.id,ordinal,edge:exit};
  if(drawings.has(key)){const existing=drawings.get(key);existing.records.push(entry);if(conflict)existing.bearing_conflict=true;continue;}
  const obstacles=[...placed.values()].filter(r=>r.id!==source.id&&r.id!==target.id).map(r=>r.cell);
  const straight=clear([a,b],obstacles),ordinary=!!v&&!conflict;
  const points=ordinary&&straight?[a,b]:connector(a,b,obstacles);
  const drawing={a_room:source.id,b_room:target.id,a,b,unit:0,kind:ordinary?'Directional':'Connector',points,records:[entry],bearing_conflict:conflict};
  // Never paint a misleading line through other rooms when no clear path is
  // found. Paired, named clickable continuations retain the actual exits.
  if(!points){drawing.kind='Continuation';continuations.push(key);}
  drawings.set(key,drawing);
 }
 scene.sheet.edges=[...drawings.values()];
 scene.display_layout={kind:'classic-aligned',source:IMAGE,anchored:anchoredCount,insets,bearing_conflicts:conflicts,continuations,
  note:'Classic-map display alignment; native exits, areas and focus units unchanged. Dashed connectors are schematic, not compass or floor claims.'};
 scene.display_sections=[
  {name:'Sewers · eight spokes',rooms:scene.sheet.rooms.filter(r=>/sewer/i.test(input.rooms[r.id].title?.[0]||'')).map(r=>r.id)},
  {name:'Catacombs · tunnels',rooms:scene.sheet.rooms.filter(r=>!/sewer/i.test(input.rooms[r.id].title?.[0]||'')).map(r=>r.id)}
 ];
 return {...input,scenes:{...input.scenes,catacombs:scene}};
}
export function passagePoints(scene,from,to,lookup){
 const e=scene.sheet.edges.find(e=>pairKey(e.a_room,e.b_room)===pairKey(from,to));
 if(e?.kind==='Continuation')return null;
 if(e?.points)return e.a_room===from?e.points:[...e.points].reverse();
 return [lookup[from].cell,lookup[to].cell];
}
