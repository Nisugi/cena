// Offline route preview. Uses native directed exits, never drawing edges.
// The caller supplies the graph; no network, character state or travel commands.
export function edgeWeight(edge,{metric='cost',scripted=false}={}) {
  if ('pass' in edge || edge.routine || typeof edge.cost!=='number' || !Number.isFinite(edge.cost) || edge.cost<0) return null;
  if (!edge.cmd) {
    if (!scripted || !edge.steps?.length) return null;
    // Traveller-dependent transport is never a fixed road. Only simple local
    // put/move sequences are eligible, and even those are explicitly unverified.
    if (!edge.steps.every(s=>Object.keys(s).length===1 && ['put','move'].includes(Object.keys(s)[0]) &&
      typeof Object.values(s)[0]==='string' && !/transport|portmaster|urchin|teleport|travel|wagon/i.test(Object.values(s)[0]))) return null;
  }
  if (/^urchin |^event transport|portmaster/i.test(edge.cmd||'')) return null;
  return metric==='steps'?1:edge.cost;
}

export function routeTo(rooms,from,to,options={}) {
  from=Number(from);to=Number(to);
  if(!rooms[from]||!rooms[to])return {status:'unbundled',ids:[],edges:[]};
  const allowed=r=>!['closed','gone','virtual'].includes(r?.status);
  if(!allowed(rooms[from])||!allowed(rooms[to]))return {status:'unreachable',ids:[],edges:[]};
  const distance=new Map([[from,0]]),previous=new Map(),open=new Set([from]),done=new Set();
  while(open.size){
    let id=null;
    for(const n of open)if(id===null||distance.get(n)<distance.get(id)||(distance.get(n)===distance.get(id)&&n<id))id=n;
    open.delete(id);if(id===to)break;done.add(id);
    for(const [ordinal,edge] of (rooms[id].exits||[]).entries()){
      if(!rooms[edge.to]||!allowed(rooms[edge.to])||done.has(edge.to))continue;
      const weight=edgeWeight(edge,options);if(weight===null)continue;
      const cost=distance.get(id)+weight;
      if(cost<(distance.get(edge.to)??Infinity)){
        distance.set(edge.to,cost);previous.set(edge.to,{from:id,to:edge.to,ordinal,edge});open.add(edge.to);
      }
    }
  }
  if(!distance.has(to))return {status:'unreachable',ids:[],edges:[]};
  const edges=[];let id=to;
  while(id!==from){const step=previous.get(id);edges.unshift(step);id=step.from;}
  return {status:'found',ids:[from,...edges.map(e=>e.to)],edges,cost:distance.get(to),scripted:edges.filter(e=>!e.edge.cmd).length};
}

// A visit is a contiguous run on one map. Town → underground → town stays
// three visits; the two town portions must NEVER be joined across the gap.
export function routeVisits(route,lookup) {
  const visits=[];
  for(const id of route.ids){
    const area=lookup[id]?.area;if(!area)continue;
    if(visits.at(-1)?.area!==area)visits.push({area,ids:[]});
    visits.at(-1).ids.push(id);
  }
  return visits;
}
export function routeOnMap(route,lookup,area) {
  return {
    segments:route.edges.filter(e=>lookup[e.from]?.area===area&&lookup[e.to]?.area===area),
    crossings:route.edges.filter(e=>lookup[e.from]?.area!==lookup[e.to]?.area&&(lookup[e.from]?.area===area||lookup[e.to]?.area===area))
  };
}
