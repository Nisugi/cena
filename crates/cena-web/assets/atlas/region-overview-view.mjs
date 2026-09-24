import {regionOverview,overviewLayout} from './region-overview.mjs';
import {command,title} from './model.mjs';

let overviewMount=0;
export function overviewNameLines(name,limit=24){
 const words=name.trim().split(/\s+/),lines=[''];
 for(const word of words){
  const candidate=(lines.at(-1)+' '+word).trim();
  if(candidate.length<=limit||!lines.at(-1))lines[lines.length-1]=candidate;
  else if(lines.length===1)lines.push(word);
  else lines[1]+=' '+word;
 }
 if(lines.length>2)lines.length=2;
 if(lines[1]?.length>limit)lines[1]=lines[1].slice(0,Math.max(1,limit-1)).trimEnd()+'…';
 if(lines[0].length>limit)lines[0]=lines[0].slice(0,Math.max(1,limit-1)).trimEnd()+'…';
 return lines;
}

export function regionOverviewView(root,{data,context,browse,directory,worldUrl='../corpus/'}){
 const doc=root.ownerDocument||root,$=id=>root.querySelector('#'+id),ns='http://www.w3.org/2000/svg';
 const regionName=data.presentation?.title?.trim()||data.presentation?.region?.trim()||data.scenes?.town?.label||'Regional map';
 const node=(tag,text,cls)=>{const n=doc.createElement(tag);if(text)n.textContent=text;if(cls)n.className=cls;return n;};
 const svgNode=(tag,attrs={},text)=>{const n=doc.createElementNS(ns,tag);for(const [k,v] of Object.entries(attrs))n.setAttribute(k,v);if(text)n.textContent=text;return n;};
 const button=(text,fn)=>{const n=node('button',text);n.onclick=fn;return n;};
 const launch=button('Region overview',()=>open());launch.id='region-overview';($('browse-region')||$('area-tabs')).before(launch);
 const dialog=node('dialog',null,'region-overview-dialog');dialog.setAttribute('aria-label',regionName+' connection overview');root.append(dialog);
 const close=button('Close overview',()=>dialog.close());close.className='overview-close';dialog.append(close,node('h2',regionName+' · region overview'));
 const tabs=node('div',null,'overview-tabs');tabs.setAttribute('role','tablist');tabs.setAttribute('aria-label','Region view');
 const mapPane=node('section'),placesPane=node('section'),filters=node('div'),selection=node('p',null,'overview-selection');
 const mount=++overviewMount;mapPane.id='overview-map-'+mount;placesPane.id='overview-places-'+mount;
 let activeTab='map',selectedKey=null,directoryUI=null;
 const tabButtons=new Map();
 function tab(which){activeTab=which;for(const [key,b] of tabButtons){b.setAttribute('aria-selected',key===which);b.tabIndex=key===which?0:-1;}(which==='map'?mapPane:placesPane).hidden=false;(which==='map'?placesPane:mapPane).hidden=true;}
 for(const [key,label,pane] of [['map','Map',mapPane],['places','Places',placesPane]]){
  const b=button(label,()=>tab(key));b.id='overview-tab-'+key+'-'+mount;b.setAttribute('role','tab');b.setAttribute('aria-controls',pane.id);pane.setAttribute('role','tabpanel');pane.setAttribute('aria-labelledby',b.id);
  b.onkeydown=e=>{if(['ArrowLeft','ArrowRight','Home','End'].includes(e.key)){e.preventDefault();const next=e.key==='Home'?'map':e.key==='End'?'places':key==='map'?'places':'map';tab(next);tabButtons.get(next).focus();}};tabButtons.set(key,b);tabs.append(b);
 }
 const world=node('a','Search all regions →');world.href=worldUrl;world.onclick=()=>{world.href=worldUrl+'#q='+encodeURIComponent(directoryUI?.query()||'');};filters.append(world);
 dialog.append(tabs,filters,selection,mapPane,placesPane);tab('map');
 mapPane.append(node('p','Schematic connections—not compass positions, distances, or a route planner. Select an area, then Open map. Search and filters apply to both tabs; dimmed maps remain as context.'));
 const controls=node('div',null,'overview-controls'),body=node('div',null,'overview-body'),viewport=node('div',null,'overview-viewport');
 const svg=svgNode('svg',{role:'group','aria-label':'Connected main maps'});viewport.append(svg);
 const detail=node('aside',null,'overview-detail');detail.setAttribute('aria-live','polite');body.append(viewport,detail);mapPane.append(controls,body);
 const model=regionOverview(data,context),layout=overviewLayout(model),byId=new Map(layout.nodes.map(n=>[n.id,n]));
 let box={x:0,y:0,w:layout.width,h:layout.height},drag=null;
 const paintCamera=()=>svg.setAttribute('viewBox',`${box.x} ${box.y} ${box.w} ${box.h}`);
 const zoom=f=>{const w=Math.max(350,Math.min(layout.width*3,box.w*f)),h=w*box.h/box.w;box={x:box.x+(box.w-w)/2,y:box.y+(box.h-h)/2,w,h};paintCamera();};
 controls.append(button('+',()=>zoom(.8)),button('−',()=>zoom(1.25)),button('Fit overview',()=>{box={x:0,y:0,w:layout.width,h:layout.height};paintCamera();}),
  node('span','Blue: plain exits · Gold dashed: scripted/special only · Arrowheads: recorded direction · Crossings are not junctions','hint'));
 const note=node('p',`${model.nodes.length} assigned main maps · ${model.links.length} direct map pairs. ${model.unassignedFrames} provisional frames are available in Places (choose All display frames); they are not collapsed into a misleading shared hub.`, 'hint');mapPane.append(note);
 function showRecords(records){
  const list=node('div',null,'overview-records');
  for(const r of records){
   const target=data.scenes[r.target]?.label||title(data.outside?.[r.to])||'Unbundled destination';
   const b=button(`${byId.get(r.source)?.name||r.source} #${r.from} → ${target} #${r.to} · ${command(r.edge)}`,()=>{dialog.close();browse(r.to in data.rooms?r.to:r.from);});b.dataset.overviewExit=`${r.from}:${r.ordinal}`;list.append(b);
  }
  return list;
 }
 const undrawn=layout.links.filter(l=>!l.points);
 if(undrawn.length){
  const directory=node('details',null,'overview-undrawn');directory.id='overview-link-directory';directory.open=true;
  directory.append(node('summary',`${undrawn.length} direct map ${undrawn.length===1?'pair has':'pairs have'} no collision-free schematic line · exact exit directory`));
  directory.append(node('p','These recorded connections remain available below. No substitute geometry, junction, return exit, or route is inferred.','hint'));
  for(const l of undrawn){
   const a=byId.get(l.a),b=byId.get(l.b),openRecords=()=>{detail.replaceChildren(node('h3',a.name+' ↔ '+b.name),node('p','Schematic line unavailable; showing the exact recorded directed exits.','hint'),showRecords(l.records));};
   const entry=button(`${a.name} ↔ ${b.name} · ${l.records.length} recorded ${l.records.length===1?'exit':'exits'}`,openRecords);entry.dataset.overviewUndrawnLink=l.id;directory.append(entry);
  }
  mapPane.append(directory);
 }
 function inspect(n){
  selectedKey=n.id;selection.textContent='Selected: '+n.name;directoryUI?.select(n.id);
  svg.querySelectorAll('[data-overview-node]').forEach(g=>g.setAttribute('aria-pressed',g.dataset.overviewNode===n.id));
  detail.replaceChildren(node('h3',n.name),node('p',`${n.rooms} rooms · ${n.level?'Reference levels '+n.level.join('–'):'Habitat coverage unknown'}`));
  const openMap=button('Open map',()=>{dialog.close();browse(n.entry);});openMap.setAttribute('aria-label','Open map: '+n.name);detail.append(openMap);
  if(n.creatures.length)detail.append(node('p',n.creatures.map(c=>`${c.name} (${c.level??'?'})`).join(' · ')));
  detail.append(node('p','Installed habitat references, not live creatures. Browsing never moves your character or changes the route destination.','hint'));
  for(const l of model.links.filter(l=>l.a===n.id||l.b===n.id)){
   const other=byId.get(l.a===n.id?l.b:l.a),out=l.records.filter(r=>r.source===n.id).length,back=l.records.length-out;
   const d=node('details');d.append(node('summary',`${other.name} · ${out} outgoing / ${back} incoming${l.special?' · special exits included':''}`),showRecords(l.records));detail.append(d);
  }
  for(const [label,records] of [['Unassigned context connections',n.pending],['Unbundled destinations',n.outside]])if(records.length){const d=node('details');d.append(node('summary',`${label} · ${records.length} records`),showRecords(records));detail.append(d);}
  svg.querySelectorAll('[data-overview-link]').forEach(e=>{const l=layout.links.find(l=>l.id===e.dataset.overviewLink);e.classList.toggle('focused',l.a===n.id||l.b===n.id);});
 }
 const markerId='overview-arrow-'+mount;
 const defs=svgNode('defs');const marker=svgNode('marker',{id:markerId,viewBox:'0 0 10 10',refX:9,refY:5,markerWidth:7,markerHeight:7,orient:'auto-start-reverse',markerUnits:'userSpaceOnUse'});marker.append(svgNode('path',{d:'M 0 0 L 10 5 L 0 10 z',fill:'#d4deea'}));defs.append(marker);svg.append(defs);
 for(const l of layout.links){
  if(!l.points)continue;
  const path=svgNode('path',{d:l.points.map((p,i)=>`${i?'L':'M'}${p.x} ${p.y}`).join(' '),fill:'none',stroke:l.special===l.records.length?'#e4b774':'#7bacc6','stroke-width':2,'stroke-dasharray':l.special===l.records.length?'7 5':'none','data-overview-link':l.id,...(l.ab?{'marker-end':`url(#${markerId})`}:{}),...(l.ba?{'marker-start':`url(#${markerId})`}:{})});
  path.append(svgNode('title',{},`${byId.get(l.a).name} ↔ ${byId.get(l.b).name}: ${l.ab} forward, ${l.ba} reverse recorded exits; ${l.special} scripted/special.`));
  path.onclick=()=>{detail.replaceChildren(node('h3',byId.get(l.a).name+' ↔ '+byId.get(l.b).name),showRecords(l.records));};svg.append(path);
 }
 for(const n of layout.nodes){
  const group=svgNode('g',{'data-overview-node':n.id,role:'button',tabindex:0,'aria-label':'Select '+n.name});
  group.append(svgNode('rect',{x:n.x-n.w/2,y:n.y-n.h/2,width:n.w,height:n.h,rx:24,fill:n.id==='town'?'#343020':n.creatures.length?'#15342f':'#172a3b',stroke:n.id==='town'?'#ffc778':'#80afa8','stroke-width':2}));
  group.append(svgNode('title',{},n.name));
  const lines=overviewNameLines(n.name);
  for(const [i,line] of lines.entries())group.append(svgNode('text',{x:n.x,y:n.y-12+(i-(lines.length-1)/2)*19,'text-anchor':'middle',fill:'#e6efef','font-size':17},line));
  group.append(svgNode('text',{x:n.x,y:n.y+29,'text-anchor':'middle',fill:'#adc6ce','font-size':13},n.level?`Lv ${n.level[0]}${n.level[1]!==n.level[0]?'–'+n.level[1]:''} · ${n.rooms} rooms`:`${n.rooms} rooms`));
  group.onclick=()=>inspect(n);group.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();group.onclick();}};svg.append(group);
 }
 svg.onwheel=e=>{e.preventDefault();zoom(e.deltaY<0?.9:1.1);};
 svg.onpointerdown=e=>{if(e.button!==0||e.target.closest('[data-overview-node]'))return;drag={x:e.clientX,y:e.clientY,box:{...box}};svg.setPointerCapture(e.pointerId);};
 svg.onpointermove=e=>{if(!drag)return;const r=svg.getBoundingClientRect(),scale=Math.min(r.width/box.w,r.height/box.h);box={...drag.box,x:drag.box.x-(e.clientX-drag.x)/scale,y:drag.box.y-(e.clientY-drag.y)/scale};paintCamera();};
 svg.onpointerup=svg.onpointercancel=()=>{drag=null;};
 function select(key){
  const n=byId.get(key);if(n){inspect(n);return;}
  const a=directory?.model.areas.find(a=>a.key===key);if(!a)return;
  selectedKey=key;directoryUI?.select(key);selection.textContent='Selected: '+a.label+' · provisional frame (Places tab)';
  svg.querySelectorAll('[data-overview-node]').forEach(g=>g.setAttribute('aria-pressed','false'));
  detail.replaceChildren(node('h3',a.label),node('p','No assigned main-map bubble. This provisional frame does not imply an approved area.'),button('Open map',()=>{dialog.close();browse(a.entryRoom);}));
 }
 if(directory)directoryUI=directory.mountDirectory(placesPane,{controlsHost:filters,close:()=>dialog.close(),select,onFilter:hits=>{
  const keys=new Set(hits.map(a=>a.key));svg.querySelectorAll('[data-overview-node]').forEach(g=>g.classList.toggle('filtered-context',!keys.has(g.dataset.overviewNode)));
 }});
 else{tabButtons.get('places').hidden=true;filters.hidden=true;}
 function open(which=activeTab){
  paintCamera();
  const first=byId.get('town')||layout.nodes[0];
  if(!selectedKey&&first)inspect(first);
  else if(!selectedKey)detail.replaceChildren(node('h3','No assigned main maps'),node('p','Only region-only context frames are available. Open Places to inspect them individually; this overview does not invent a shared hub or connecting exits.','hint'));
  tab(which);if(!dialog.open)dialog.showModal();
 }
 return {open,select};
}
