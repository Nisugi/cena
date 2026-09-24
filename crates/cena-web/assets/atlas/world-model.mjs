// A display filter over exact source records, not a route graph.
export function worldModel(data,{special=false,buckets=false}={}){
 const nodes=data.nodes.filter(n=>buckets||n.geographic),ids=new Set(nodes.map(n=>n.id));
 const links=data.links.filter(l=>ids.has(l.a)&&ids.has(l.b)).map(l=>{
  const records=l.records.filter(r=>special||!r.special);
  return {...l,records,ab:records.filter(r=>r.source===l.a).length,ba:records.filter(r=>r.source===l.b).length,special:records.filter(r=>r.special).length};
 }).filter(l=>l.records.length);
 return {nodes,links};
}
