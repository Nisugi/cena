import {overviewLayout} from './region-overview.mjs';
import {overviewNameLines} from './region-overview-view.mjs';
import {worldModel} from './world-model.mjs';

const $=id=>document.getElementById(id),ns='http://www.w3.org/2000/svg';
const node=(tag,text)=>{const n=document.createElement(tag);if(text)n.textContent=text;return n;};
const button=(text,fn)=>{const n=node('button',text);n.onclick=fn;return n;};
const link=(text,url)=>{const n=node('a',text);n.href=url;return n;};
const svgNode=(tag,attrs={},text)=>{const n=document.createElementNS(ns,tag);for(const [k,v] of Object.entries(attrs))n.setAttribute(k,v);if(text)n.textContent=text;return n;};
const read=async path=>{const r=await fetch(path);if(!r.ok)throw Error(`${path}: HTTP ${r.status}`);return r.json();};
try{
 const data=await read('world.json'),byId=new Map(data.nodes.map(n=>[n.id,n])),svg=$('world-map');
 let model,layout,box,selected=null,drag=null,searchPromise=null,searchVersion=0;
 const regionHref=(n,tab='map')=>n.url+'#overview='+tab;
 const roomHref=(region,id)=>byId.get(region).url+'#room='+id+'&browse=1';
 const paintCamera=()=>svg.setAttribute('viewBox',`${box.x} ${box.y} ${box.w} ${box.h}`);
 const fit=()=>{box={x:0,y:0,w:layout.width,h:layout.height};paintCamera();};
 const zoom=f=>{const w=Math.max(300,Math.min(layout.width*3,box.w*f)),h=w*box.h/box.w;box={x:box.x+(box.w-w)/2,y:box.y+(box.h-h)/2,w,h};paintCamera();};
 function recordsView(records){
  const list=node('div');
  for(const r of records){
   const row=node('p');row.append(node('span',`${byId.get(r.source).name} #${r.from} → ${byId.get(r.target).name} #${r.to} · ${r.edge.cmd||'Scripted / conditional passage'}${r.special?' · special':''}${r.closed?' · closed endpoint':''} `),link('Source room',roomHref(r.source,r.from)),link('Destination room',roomHref(r.target,r.to)));list.append(row);
  }return list;
 }
 function inspect(id){
  selected=id;const n=byId.get(id),detail=$('world-detail');
  detail.replaceChildren(node('h2',n.name),node('p',`${n.rooms.toLocaleString()} source-region rooms · ${n.maps} assigned maps · ${n.unassigned.toLocaleString()} rooms without an area.`));
  if(!n.geographic)detail.append(node('p','Non-geographical grouping, not a physical region or inferred connecting hub.'));
  if(n.url)detail.append(link('Open region overview',regionHref(n)),link('Browse its places',regionHref(n,'places')));
  const pairs=model.links.filter(l=>l.a===id||l.b===id);
  detail.append(node('p',`${pairs.length} region pairs under the current connection filters. These records do not confirm a safe or usable route.`));
  for(const l of pairs){
   const other=byId.get(l.a===id?l.b:l.a),out=l.records.filter(r=>r.source===id).length,d=node('details');
   d.append(node('summary',`${other.name} · ${out} outgoing / ${l.records.length-out} incoming`));
   // Large portal sets need not materialize until expanded.
   d.ontoggle=()=>{if(d.open&&d.children.length===1)d.append(recordsView(l.records));};detail.append(d);
  }
  svg.querySelectorAll('[data-world-region]').forEach(g=>g.setAttribute('aria-pressed',g.dataset.worldRegion===id));
  svg.querySelectorAll('[data-world-link]').forEach(p=>{const l=model.links.find(l=>l.id===p.dataset.worldLink);p.style.opacity=l.a===id||l.b===id?'1':'.15';});
 }
 function draw(){
  model=worldModel(data,{special:$('world-special').checked,buckets:$('world-buckets').checked});layout=overviewLayout(model);svg.replaceChildren();
  const defs=svgNode('defs'),marker=svgNode('marker',{id:'world-arrow',viewBox:'0 0 10 10',refX:9,refY:5,markerWidth:7,markerHeight:7,orient:'auto-start-reverse',markerUnits:'userSpaceOnUse'});marker.append(svgNode('path',{d:'M 0 0 L 10 5 L 0 10 z',fill:'#d4deea'}));defs.append(marker);svg.append(defs);
  for(const l of layout.links){if(!l.points)continue;
   const p=svgNode('path',{d:l.points.map((p,i)=>`${i?'L':'M'}${p.x} ${p.y}`).join(' '),fill:'none',stroke:l.special===l.records.length?'#deb66a':'#7bacc6','stroke-width':2,'stroke-dasharray':l.special===l.records.length?'7 5':'none','data-world-link':l.id,...(l.ab?{'marker-end':'url(#world-arrow)'}:{}),...(l.ba?{'marker-start':'url(#world-arrow)'}:{})});
   p.append(svgNode('title',{},`${byId.get(l.a).name} → ${byId.get(l.b).name}: ${l.ab}; reverse: ${l.ba}.`));svg.append(p);
  }
  for(const n of layout.nodes){const g=svgNode('g',{'data-world-region':n.id,role:'button',tabindex:0,'aria-label':'Select '+n.name});
   g.append(svgNode('rect',{x:n.x-n.w/2,y:n.y-n.h/2,width:n.w,height:n.h,rx:20,fill:n.geographic?'#18313c':'#302838',stroke:n.geographic?'#81b4b1':'#b5a0ba','stroke-width':2}),svgNode('title',{},n.name));
   const lines=overviewNameLines(n.name);for(const [i,line] of lines.entries())g.append(svgNode('text',{x:n.x,y:n.y-10+(i-(lines.length-1)/2)*19,'text-anchor':'middle',fill:'#e6efef','font-size':17},line));
   g.append(svgNode('text',{x:n.x,y:n.y+29,'text-anchor':'middle',fill:'#adc6ce','font-size':13},n.geographic?`${n.maps} maps · ${n.rooms.toLocaleString()} rooms`:'Non-geographical bucket'));
   g.onclick=()=>inspect(n.id);g.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();inspect(n.id);}};svg.append(g);
  }
  const undrawn=layout.links.filter(l=>!l.points),directory=$('world-undrawn');directory.hidden=!undrawn.length;directory.lastElementChild.replaceChildren();directory.firstElementChild.textContent=`${undrawn.length} region pairs without a collision-free schematic line — inspect their recorded exits`;
  for(const l of undrawn)directory.lastElementChild.append(button(byId.get(l.a).name+' ↔ '+byId.get(l.b).name,()=>{$('world-detail').replaceChildren(node('h2','Recorded connections'),recordsView(l.records));}));
  $('world-status').textContent=`${model.nodes.length} of ${data.nodes.length} region groups shown · ${model.links.length} direct region pairs · ${model.links.reduce((s,l)=>s+l.records.length,0)} directed records. ${data.excluded_endpoint_records.length} exits to absent/excluded rooms are not mapped as region links.`;
  fit();if(selected&&model.nodes.some(n=>n.id===selected))inspect(selected);else{selected=null;$('world-detail').replaceChildren(node('h2','Select a region'),node('p','Choose a bubble or a region below. Then open its overview to explore Map and Places.'));}void search();
 }
 async function search(){
  const version=++searchVersion,q=$('world-query').value.trim().toLowerCase(),results=$('world-results');results.replaceChildren();
  const params=new URLSearchParams(location.hash.slice(1));if(q)params.set('q',$('world-query').value.trim());else params.delete('q');history.replaceState(null,'',location.pathname+(params.size?'#'+params:''));
  $('results-heading').textContent=q?'Search results · all regions':'Regions';$('search-status').textContent='';
  const matches=data.nodes.filter(n=>!q||n.name.toLowerCase().includes(q));
  for(const n of matches){const card=node('article');card.className='world-card';card.append(node('h2',n.name),node('p',`${n.maps} assigned maps · ${n.rooms.toLocaleString()} source rooms${n.geographic?'':' · non-geographical bucket'}`));if(n.url)card.append(link('Region overview',regionHref(n)),link('Places',regionHref(n,'places')));results.append(card);}
  if(!q){svg.querySelectorAll('[data-world-region]').forEach(g=>g.classList.remove('dim'));return;}
  $('search-status').textContent='Searching places and rooms…';
  try{
   searchPromise??=read('search.json').catch(e=>{searchPromise=null;throw e;});const rooms=await searchPromise;if(version!==searchVersion)return;
   const hits=rooms.filter(r=>String(r.id)===q||`${r.title} ${r.area} ${byId.get(r.region).name}`.toLowerCase().includes(q));
   const regions=new Set([...matches.map(n=>n.id),...hits.map(r=>r.region)]);svg.querySelectorAll('[data-world-region]').forEach(g=>g.classList.toggle('dim',!regions.has(g.dataset.worldRegion)));
   for(const r of hits.slice(0,80)){const card=node('article');card.className='world-card';card.dataset.worldRoom=r.id;card.append(link(`${r.title} · #${r.id}`,roomHref(r.region,r.id)),node('p',`${byId.get(r.region).name} · ${r.area||'Area unassigned'}${r.status==='closed'?' · closed':''}`));results.append(card);}
   $('search-status').textContent=`${matches.length} region names and ${hits.length.toLocaleString()} rooms match${hits.length>80?' · showing first 80 rooms; refine your search':''}. Room results use source-region ownership, without duplicate context copies.`;
  }catch(e){if(version===searchVersion)$('search-status').textContent='Room search unavailable: '+e.message;}
 }
 $('world-query').value=new URLSearchParams(location.hash.slice(1)).get('q')||'';$('world-query').oninput=search;
 $('world-special').onchange=$('world-buckets').onchange=draw;$('world-fit').onclick=fit;$('world-in').onclick=()=>zoom(.8);$('world-out').onclick=()=>zoom(1.25);
 svg.onwheel=e=>{e.preventDefault();zoom(e.deltaY<0?.9:1.1);};
 svg.onpointerdown=e=>{if(e.button!==0||e.target.closest('[data-world-region]'))return;drag={x:e.clientX,y:e.clientY,box:{...box}};svg.setPointerCapture(e.pointerId);};
 svg.onpointermove=e=>{if(!drag)return;const r=svg.getBoundingClientRect(),scale=Math.min(r.width/box.w,r.height/box.h);box={...drag.box,x:drag.box.x-(e.clientX-drag.x)/scale,y:drag.box.y-(e.clientY-drag.y)/scale};paintCamera();};svg.onpointerup=svg.onpointercancel=()=>{drag=null;};
 draw();
}catch(e){$('world-status').textContent='World navigation could not load: '+e.message;}
