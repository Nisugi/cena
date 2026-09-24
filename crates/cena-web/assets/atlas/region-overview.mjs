// Region topology, not world coordinates. Every line retains its exact,
// directed boundary records. Unassigned context never becomes a fake hub.
export function regionOverview(data,context=null){
 const membership=new Map(Object.entries(data.scenes).flatMap(([key,s])=>s.sheet.rooms.map(r=>[r.id,key])));
 const nodes=Object.entries(data.scenes).filter(([,s])=>s.assignment_kind!=='region-only').map(([id,s])=>{
  const creatures=(context?.creatures||[]).filter(c=>c.associations.some(a=>a.group===id));
  const levels=creatures.map(c=>c.level).filter(Number.isFinite),live=r=>data.rooms[r]?.status!=='closed';
  return {id,name:s.label,rooms:s.sheet.rooms.length,entry:s.units.find(u=>u.kind==='Streets')?.rooms.find(live)??s.sheet.rooms.find(r=>live(r.id))?.id??s.sheet.rooms[0]?.id,
   creatures,level:levels.length?[Math.min(...levels),Math.max(...levels)]:null,pending:[],outside:[]};
 }).sort((a,b)=>a.name.localeCompare(b.name));
 const byId=new Map(nodes.map(n=>[n.id,n])),links=new Map();
 for(const n of nodes)for(const r of data.scenes[n.id].sheet.rooms)for(const [ordinal,edge] of data.rooms[r.id].exits.entries()){
  const to=membership.get(edge.to);if(to===n.id)continue;
  const record={from:r.id,to:edge.to,ordinal,source:n.id,target:to||null,edge};
  if(!to){n.outside.push(record);continue;}
  if(!byId.has(to)){n.pending.push(record);continue;}
  const [a,b]=[n.id,to].sort(),key=a+'|'+b;
  if(!links.has(key))links.set(key,{id:key,a,b,records:[]});links.get(key).records.push(record);
 }
 for(const l of links.values()){
  l.records.sort((a,b)=>a.from-b.from||a.ordinal-b.ordinal);
  l.ab=l.records.filter(r=>r.source===l.a).length;l.ba=l.records.filter(r=>r.source===l.b).length;
  l.special=l.records.filter(r=>!r.edge.cmd||r.edge.kind==='scripted'||'pass' in r.edge||r.edge.routine).length;
 }
 return {nodes,links:[...links.values()].sort((a,b)=>a.id.localeCompare(b.id)),unassignedFrames:Object.values(data.scenes).filter(s=>s.assignment_kind==='region-only').length};
}

export function overviewLayout(model){
 const columns=Math.max(1,Math.ceil(Math.sqrt(model.nodes.length*1.3))),rows=Math.ceil(model.nodes.length/columns);
 const width=columns*300,height=Math.max(160,rows*160);
 const cells=Array.from({length:columns*rows},(_,i)=>({x:(i%columns)*300+150,y:Math.floor(i/columns)*160+80}));
 const center={x:width/2,y:height/2};cells.sort((a,b)=>Math.hypot(a.x-center.x,a.y-center.y)-Math.hypot(b.x-center.x,b.y-center.y)||a.y-b.y||a.x-b.x);
 const degree=id=>model.links.filter(l=>l.a===id||l.b===id).length;
 const ordered=[...model.nodes].sort((a,b)=>Number(b.id==='town')-Number(a.id==='town')||degree(b.id)-degree(a.id)||a.id.localeCompare(b.id));
 const positions=new Map(ordered.map((n,i)=>[n.id,{...cells[i]}]));
 const cost=()=>model.links.reduce((sum,l)=>{const a=positions.get(l.a),b=positions.get(l.b);return sum+Math.hypot(a.x-b.x,a.y-b.y);},0);
 let best=cost();
 for(let pass=0;pass<8;pass++){let improved=false;
  for(let i=0;i<ordered.length;i++)for(let j=i+1;j<ordered.length;j++){
   const a=ordered[i].id,b=ordered[j].id;if(a==='town'||b==='town')continue;
   const pa=positions.get(a),pb=positions.get(b);positions.set(a,pb);positions.set(b,pa);const next=cost();
   if(next<best-0.001){best=next;improved=true;}else{positions.set(a,pa);positions.set(b,pb);}
  }
  if(!improved)break;
 }
 const nodes=model.nodes.map(n=>({...n,...positions.get(n.id),w:240,h:88}));
 // Small shared routing lattice: lines cannot pass through an unrelated bubble.
 const step=10,cols=width/step+1,rs=height/step+1,size=cols*rs,blocked=new Uint8Array(size);
 for(const n of nodes)for(let y=Math.floor((n.y-n.h/2-8)/step);y<=Math.ceil((n.y+n.h/2+8)/step);y++)for(let x=Math.floor((n.x-n.w/2-8)/step);x<=Math.ceil((n.x+n.w/2+8)/step);x++)if(x>=0&&x<cols&&y>=0&&y<rs)blocked[y*cols+x]=1;
 const ports=n=>[...[-20,0,20].flatMap(d=>[{x:n.x-140,y:n.y+d,b:{x:n.x-n.w/2,y:n.y+d}},{x:n.x+140,y:n.y+d,b:{x:n.x+n.w/2,y:n.y+d}}]),
  ...[-80,-40,0,40,80].flatMap(d=>[{x:n.x+d,y:n.y-70,b:{x:n.x+d,y:n.y-n.h/2}},{x:n.x+d,y:n.y+70,b:{x:n.x+d,y:n.y+n.h/2}}])];
 const at=p=>Math.round(p.y/step)*cols+Math.round(p.x/step),point=i=>({x:(i%cols)*step,y:Math.floor(i/cols)*step});
 const byId=new Map(nodes.map(n=>[n.id,n]));
 const usedSegments=new Set(),usedPorts=new Set(),segment=(a,b)=>a<b?`${a}:${b}`:`${b}:${a}`;
 const links=model.links.map(l=>{
  const starts=ports(byId.get(l.a)).filter(p=>!usedPorts.has(at(p))),ends=ports(byId.get(l.b)).filter(p=>!usedPorts.has(at(p))),goals=new Map(ends.map(p=>[at(p),p]));
  const prev=new Int32Array(size).fill(-1),queue=new Int32Array(size);let head=0,tail=0,found=-1;
  starts.forEach((p,i)=>{const k=at(p);if(k>=0&&k<size&&!blocked[k]){prev[k]=-2-i;queue[tail++]=k;}});
  while(head<tail){const k=queue[head++];if(goals.has(k)){found=k;break;}
   const x=k%cols,y=Math.floor(k/cols);
   for(const next of [x>0?k-1:-1,x<cols-1?k+1:-1,y>0?k-cols:-1,y<rs-1?k+cols:-1])if(next>=0&&!blocked[next]&&prev[next]===-1&&!usedSegments.has(segment(k,next))){prev[next]=k;queue[tail++]=next;}
  }
  if(found<0)return {...l,points:null}; // explicit link directory remains available.
  const path=[];let k=found;while(prev[k]>=0){path.unshift(point(k));usedSegments.add(segment(k,prev[k]));k=prev[k];}
  usedPorts.add(k);usedPorts.add(found);
  path.unshift(starts[-prev[k]-2].b,point(k));path.push(goals.get(found).b);
  const points=path.filter((p,i)=>!i||i===path.length-1||((p.x-path[i-1].x)*(path[i+1].y-p.y)!==(p.y-path[i-1].y)*(path[i+1].x-p.x)));
  return {...l,points};
 });
 return {nodes,links,width,height};
}
