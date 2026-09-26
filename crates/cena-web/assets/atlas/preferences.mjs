import {title} from './model.mjs';
export const categories={
  landmarks:{name:'Landmarks',symbol:'◆',color:'#f3df9f'},
  bank:{name:'Bank',symbol:'◉',icon:'coins',color:'#e5be52'},
  furrier:{name:'Furrier',symbol:'⬡',icon:'hide',color:'#cfaa80'},
  gemshop:{name:'Gemshop',symbol:'◇',icon:'gem',color:'#64dafa'},
  pawnshop:{name:'Pawnshop',symbol:'⚖',icon:'scales',color:'#f3a25c'},
  advguild:{name:'Adventurer’s Guild',symbol:'⛨',icon:'shield',color:'#829fff'},
  locksmith:{name:'Locksmith',symbol:'⚿',icon:'key',color:'#c4a0ef'},
  healer:{name:'Healer',symbol:'✚',icon:'cross',color:'#f38d99'},
  herbalist:{name:'Herbalist',symbol:'♧',icon:'leaf',color:'#91d578'},
  alchemist:{name:'Alchemist',symbol:'⚗',icon:'flask',color:'#60d3bd'},
  shops:{name:'Shops',symbol:'▣',color:'#74d6ae'},
  services:{name:'Services',symbol:'✚',color:'#79c9ee'},
  guilds:{name:'Guilds & societies',symbol:'⚑',color:'#c5a2f4'},
  houses:{name:'Houses & meeting halls',symbol:'⌂',color:'#eda7bd'}
};
export function defaults(){return {layers:Object.fromEntries(Object.keys(categories).map(k=>[k,true])),colors:Object.fromEntries(Object.entries(categories).map(([k,v])=>[k,v.color])),labels:'names',transitions:'names',hideLabels:false,numbers:false,animate:true,routeColor:'#57f3cb'};}
export function cleanPreferences(raw){
  const p=defaults();if(!raw||typeof raw!=='object')return p;
  for(const k of Object.keys(categories)){
    if(typeof raw.layers?.[k]==='boolean')p.layers[k]=raw.layers[k];
    if(/^#[a-f\d]{6}$/i.test(raw.colors?.[k]||''))p.colors[k]=raw.colors[k];
  }
  for(const k of ['labels','transitions'])if(['symbols','names','details'].includes(raw[k]))p[k]=raw[k];
  for(const k of ['numbers','animate','hideLabels'])if(typeof raw[k]==='boolean')p[k]=raw[k];
  if(/^#[a-f\d]{6}$/i.test(raw.routeColor||''))p.routeColor=raw.routeColor;
  return p;
}
export function preset(name,current){
  const p=cleanPreferences(current);
  if(name==='travel'){for(const k of Object.keys(p.layers))p.layers[k]=k==='landmarks';p.labels='symbols';p.transitions='details';}
  else{for(const k of Object.keys(p.layers))p.layers[k]=true;p.labels='names';p.transitions='names';p.numbers=name==='explorer';}
  return p;
}
// Marker identity is not area ownership. Aggregate only at the same source
// room and category, preserving every directed exit as a separate choice.
export function placeMarkers(features){
 const folded=name=>name.trim().toLocaleLowerCase('en-US').replace(/\s+/g,' ');
 const groups=new Map();
 for(const feature of features){
  let name=feature.name;
  if(feature.key==='houses'&&!feature.entrance){
   // A co-located courtyard may use "X" while its recorded exits name
   // "X Inn". This narrow local presentation alias does not merge two
   // differently named destinations or create a global house-name registry.
   const alias=features.find(f=>f.from===feature.from&&f.entrance&&f.key==='houses'&&folded(f.name)===folded(name)+' inn');
   if(alias)name=alias.name;
  }
  const identity=JSON.stringify([feature.from,feature.key,folded(name)]);
  if(!groups.has(identity))groups.set(identity,{...feature,name,identity,links:[],members:[],local:false});
  const group=groups.get(identity);group.members.push(feature);
  if(feature.entrance)group.links.push(feature);else group.local=true;
 }
 return [...groups.values()].map(group=>({...group,entrance:group.links.length>0,to:group.links.length===1?group.links[0].to:group.from}));
}
// A local label can sit outside a gate, before the actual building focus.
// Find preview choices through directed, ordinary same-place approach rooms
// within that one focus. Keep the real exit source and approach evidence; this
// does not add an edge, change ownership, or claim the route is accessible.
export function resolvePlaceMarker(data,lookup,hints,marker){
 const source=lookup[marker.from];
 if(marker.entrance||marker.key==='landmarks'||!source||
    data.scenes[source.area]?.units[source.unit]?.kind!=='Streets')return marker;
 const samePlace=id=>(hints[id]?.features||[]).some(f=>!f.entrance&&f.key===marker.key&&f.name===marker.name);
 const seen=new Set([marker.from]),queue=[{id:marker.from,path:[]}],links=[];
 for(const {id,path} of queue)for(const [ordinal,edge] of (data.rooms[id]?.exits||[]).entries()){
  const target=lookup[edge.to];
  if(!edge.cmd||edge.kind==='scripted'||edge.steps||edge.routine||'pass' in edge||!target||!samePlace(edge.to))continue;
  const step={from:id,to:edge.to,ordinal,edge};
  if(target.area===source.area&&target.unit===source.unit){
   if(!seen.has(edge.to)){seen.add(edge.to);queue.push({id:edge.to,path:[...path,step]});}
  }else links.push({key:marker.key,name:marker.name,...step,entrance:true,approach:path});
 }
 return links.length?{...marker,entrance:true,approach:true,links}:marker;
}
// Search/highlight hints, not ownership assignments. Prefer metadata; name
// matches are explicitly heuristic and never alter membership or routing.
export function annotations(data,lookup){
  const result={};
  for(const r of Object.values(data.rooms)){
    const text=title(r),tags=[...(r.tags||[]),...(r.meta||[]),...(lookup[r.id]?.service_tags||[])];
    const kinds=[];
    const prefix=text.split(',')[0];
    // Exact service tags first; title fallbacks name a business, not a nearby
    // road, plant tag, guild pickup spot or generic "house".
    const rules={bank:/\bbank\b/i,furrier:/\bfurrier\b/i,gemshop:/\bgem(?:shop|cutter| dealer|'s shop)\b/i,pawnshop:/\bpawnshop\b/i,advguild:/\badventurer['’]?s? guild\b/i,locksmith:/\blocksmith\b/i,healer:/\bhealer\b/i,herbalist:/\bherbalist\b/i,alchemist:/\balchemist\b/i};
    for(const [key,pattern] of Object.entries(rules))if(tags.includes(key)||(key==='locksmith'&&tags.includes('locksmithpool'))||(!/\b(road|street|lane|way|rd\.)\b/i.test(prefix)&&pattern.test(prefix)))kinds.push(key);
    if(tags.some(t=>['tsc','northgate','westgate'].includes(t))||/town square central|outside gate|adventurer.?s.? guild/i.test(text))kinds.push('landmarks');
    if(data.presentation?.start_room===r.id&&!kinds.includes('landmarks'))kinds.push('landmarks');
    if(tags.some(t=>/shop$|^shop:/.test(t))||/boutique|armory|emporium|shop|store|outfitter|bakery|confection|grocer|tykel.s arms|baker.s shop|rugs|the new look|depot/i.test(text))kinds.push('shops');
    if(tags.some(t=>/^(bank|healer|locksmith|advpickup|portmaster)$/.test(t))||/bank|locksmith|healer|adventurer.?s.? guild|stables|moot hall/i.test(text))kinds.push('services');
    if(tags.some(t=>/^society:|guild$/.test(t))||/guild|council,|sunfist|voln/i.test(text))kinds.push('guilds');
    if(tags.some(t=>/^che:|^mho:/.test(t))||/house (aspis|brigatta|of paupers)|helden hall|twilight hall|silvergate|paupers|willow hall/i.test(text))kinds.push('houses');
    if(kinds.some(k=>categories[k].icon))for(const k of ['shops','services','guilds']){const i=kinds.indexOf(k);if(i>=0)kinds.splice(i,1);}
    let name=text;
    if(tags.includes('tsc'))name='Town Square Central (TSC)';
    // An explicit exit from this room is stronger evidence than a title guess.
    if(r.area==='town'&&r.exits.some(e=>e.cmd==='go town gates')){kinds.unshift('landmarks');name='North gate';}
    if(r.area==='town'&&r.exits.some(e=>e.cmd==='go gate'&&/outside gate/i.test(title(data.outside[e.to])))){kinds.unshift('landmarks');name='Town gate';}
    // Prefer an explicit house name among recorded title variants, without
    // replacing the room's primary title (e.g. Paupers / House of Paupers).
    const houseName=(r.title||[]).map(t=>t.replace(/^\[|\]$/g,'').split(',')[0]).find(t=>/^House\s/i.test(t))||prefix;
    result[r.id]={name,kinds:[...new Set(kinds)],features:[...new Set(kinds)].map(key=>({key,name:key==='landmarks'?name:key==='houses'?houseName:prefix,from:r.id,to:r.id,entrance:false,evidence:tags.includes(key)?'Explicit service metadata':'Metadata/title highlight hint'})),evidence:'Metadata/title highlight hint; not a verified service inventory'};
  }
  // Carry a building's category to its recorded street doorway. This adds a
  // highlight hint only; it never adds a room or alters area ownership.
  const direct=structuredClone(result);
  for(const r of Object.values(data.rooms)){
    const source=lookup[r.id];if(!source)continue;
    const entrances=[];
    for(const [ordinal,e] of r.exits.entries()){
      const target=lookup[e.to];
      if(!e.cmd||!target||(target.area===source.area&&target.unit===source.unit))continue;
      // One hop only, never recursively spread a category through the graph.
      const features=(direct[e.to]?.features||[]).filter(f=>f.key!=='landmarks');
      for(const f of features){
        // An exit between two previews of the same named place is not another
        // business entrance. Its native transition still remains available.
        if(direct[r.id].features.some(own=>own.key===f.key&&own.name===f.name))continue;
        result[r.id].features.push({...f,from:r.id,to:e.to,ordinal,edge:e,entrance:true,evidence:`Recorded entrance: ${e.cmd} · #${r.id} → #${e.to}`});
        result[r.id].kinds=[...new Set([...result[r.id].kinds,f.key])];entrances.push(f.name);
      }
    }
    if(entrances.length){result[r.id].entrances=[...new Set(entrances)];result[r.id].evidence+='; recorded doorway to '+result[r.id].entrances.join(', ');}
  }
  return result;
}
