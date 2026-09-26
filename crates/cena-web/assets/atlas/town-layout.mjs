// Reference-backed Landing display adapter. Room identity, exits and exported
// area membership are untouched. No per-room coordinate overrides.
const IMAGE='wl-wehnimers-1264234799.png',SCALE=10;
const directions={north:[0,-1],south:[0,1],east:[1,0],west:[-1,0],northeast:[1,-1],northwest:[-1,-1],southeast:[1,1],southwest:[-1,1]};
const dist=(a,b)=>Math.hypot(a.x-b.x,a.y-b.y);
export function alignTown(input){
 const original=input.scenes.town;if(!original)return input;
 const s=structuredClone(original),old=new Map(original.sheet.rooms.map(r=>[r.id,r])),rooms=new Map(s.sheet.rooms.map(r=>[r.id,r]));
 const anchored=new Set(),placed=[],insets=[],adjusted=[];
 // Only exterior/context rooms use the shared town sheet. Interior artwork
 // can use its own scale or inset; it is not blindly projected onto town.
 for(const r of s.sheet.rooms.filter(r=>r.unit===0)){
  const image=input.rooms[r.id].image,rect=image?.rect;
  if(image?.file===IMAGE&&rect?.length===4&&rect.every(Number.isFinite)){
   r.cell={x:(rect[0]+rect[2])/2/SCALE,y:(rect[1]+rect[3])/2/SCALE};
   r.display_position_source='recorded town artwork coordinate';anchored.add(r.id);placed.push(r);
  }
 }
 if(anchored.size<150)return input; // Not a generic world-geometry guess.
 const candidates=anchor=>{
  const out=[];for(let x=-32;x<=32;x++)for(let y=-32;y<=32;y++)out.push({x:anchor.x+x,y:anchor.y+y});
  return out.sort((a,b)=>dist(a,anchor)-dist(b,anchor)||b.y-a.y||a.x-b.x);
 };
 // Some recorded click rectangles overlap despite representing different
 // streets. Use perpendicular cardinal neighbours to recover row/column
 // alignment, then require separation. Never silently call that surveyed.
 for(const r of placed){
  if(!placed.some(n=>n.id!==r.id&&dist(n.cell,r.cell)<1.8))continue;
  const anchor={...r.cell};
  for(const axis of ['x','y']){
   const values=input.rooms[r.id].exits.flatMap(e=>{
    const v=directions[e.cmd],n=rooms.get(e.to);
    return v&&n&&anchored.has(n.id)&&v[axis==='x'?0:1]===0?[n.cell[axis]]:[];
   }).sort((a,b)=>a-b);
   if(values.length)anchor[axis]=values[Math.floor(values.length/2)];
  }
  r.cell=candidates(anchor).find(p=>placed.every(n=>n.id===r.id||dist(n.cell,p)>=1.8))||r.cell;
  r.display_position_source='schematic separation of conflicting artwork coordinates using cardinal neighbours';adjusted.push(r.id);
 }
 for(const r of s.sheet.rooms.filter(r=>r.unit===0&&!anchored.has(r.id))){
  const neighbours=placed.filter(n=>input.rooms[n.id].exits.some(e=>e.to===r.id)||input.rooms[r.id].exits.some(e=>e.to===n.id));
  const parent=neighbours[0];let anchor=parent?{...parent.cell}:{x:Math.max(...placed.map(n=>n.cell.x))+8,y:Math.min(...placed.map(n=>n.cell.y))};
  const edge=parent&&input.rooms[parent.id].exits.find(e=>e.to===r.id),v=directions[edge?.cmd];
  if(v)anchor={x:anchor.x+v[0]*4,y:anchor.y+v[1]*4};else anchor.y+=4;
  r.cell=candidates(anchor).find(p=>placed.every(n=>dist(n.cell,p)>2.4))||anchor;
  r.display_position_source='schematic context beside recorded doorway; no surveyed position';placed.push(r);insets.push(r.id);
 }
 // Preserve each interior's shape, translating it with its street anchor.
 // The generic footprint packer subsequently finds clear nearby space.
 for(const [unit,u] of s.units.entries()){
  if(unit===0)continue;
  const ids=new Set(u.rooms),members=u.rooms.map(id=>rooms.get(id));
  // The town artwork uses a larger cell spacing than the native unit grid.
  // Scale whole interior shapes uniformly before packing; adjacent rooms
  // are not collisions and must not be individually scattered apart.
  const pivot={...members[0].cell};
  for(const r of members){r.cell={x:pivot.x+(r.cell.x-pivot.x)*2,y:pivot.y+(r.cell.y-pivot.y)*2};r.display_position_source='native interior shape uniformly scaled and translated beside town context';}
  const doors=s.sheet.edges.flatMap(e=>ids.has(e.a_room)&&rooms.get(e.b_room)?.unit===0?[e.b_room]:ids.has(e.b_room)&&rooms.get(e.a_room)?.unit===0?[e.a_room]:[]);
  if(!doors.length)for(const id of ids)for(const e of input.rooms[id].exits)if(rooms.get(e.to)?.unit===0)doors.push(e.to);
  const door=doors.sort((a,b)=>a-b)[0];
  if(door!==undefined){
   const a=rooms.get(door).cell,b=old.get(door).cell;
   for(const r of members)r.cell={x:r.cell.x+a.x-b.x,y:r.cell.y+a.y-b.y};
  }
  // A rigid translation cannot fix stacks INSIDE a group. Give each
  // coincident room a distinct schematic cell; retain and label provenance.
  const local=[];
  for(const r of members.sort((a,b)=>a.id-b.id)){
   if(local.some(n=>dist(n.cell,r.cell)<1e-6)){
    r.cell=candidates(r.cell).find(p=>local.every(n=>dist(n.cell,p)>=1.8)&&placed.every(n=>dist(n.cell,p)>1.8))||r.cell;
    adjusted.push(r.id);r.display_position_source='schematic de-stacking of coincident native interior rooms';
   }
   local.push(r);
  }
 }
 for(const e of s.sheet.edges){e.a={...rooms.get(e.a_room).cell};e.b={...rooms.get(e.b_room).cell};delete e.points;}
 s.town_alignment={source:IMAGE,anchored:anchored.size,insets,adjusted,
  note:'Town artwork anchors exterior/context rooms; interior translations and unanchored context are schematic. Exported ownership and native exits unchanged.'};
 return {...input,scenes:{...input.scenes,town:s}};
}
