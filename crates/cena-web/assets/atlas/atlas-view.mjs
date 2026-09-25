import {title,command,index,focusName,transitions,initial,visit,bounds} from './model.mjs';
import {routeVisits,routeOnMap} from './navigation.mjs';
import {categories,defaults,cleanPreferences,preset,annotations,placeMarkers,resolvePlaceMarker} from './preferences.mjs';
import {placeLabel} from './labels.mjs';
import {icon} from './icons.mjs';
import {presentation,layoutMode} from './profile.mjs';
import {preparePresentation,passagePoints,cameraDrawing} from './display-layout.mjs';
import {nearestRoom,svgViewport} from './interaction.mjs';
import {regionalBrowser} from './region-view.mjs';
import {huntingView} from './hunting-view.mjs';
import {huntingEditor} from './hunting-editor.mjs';
import {nativeHuntSetup} from './native-hunt-setup.mjs';
import {regionOverviewView} from './region-overview-view.mjs';

export function supportsRegionOverview(data){
 if(data?.presentation&&Object.prototype.hasOwnProperty.call(data.presentation,'region'))return true;
 return Object.values(data?.scenes||{}).some(s=>!!s.assignment_kind&&s.assignment_kind!=='region-only'&&s.sheet?.rooms?.length>0);
}
// Mount-local state. The host owns loading, persistence and navigation.
export async function mountAtlas(root,{source,storage=null,initialHash='',onNavigate=()=>{},onOutside=null,developerHunting=false,huntingFiles=null,corrections,nativeSetup=null}={}){
 if(!source?.load||!source?.previewRoute)throw Error('Atlas requires an explicit data/route adapter');
 const document=root.ownerDocument||root,window=document.defaultView;
 const cleanup=new AbortController();let destroyed=false,resizeObserver;
 const on=(target,type,handler,options={})=>target.addEventListener(type,handler,{...options,signal:cleanup.signal});
 const $=id=>root.querySelector('#'+id), NS='http://www.w3.org/2000/svg';
const el=(tag,attrs={},text)=>{const n=document.createElementNS(NS,tag);for(const [k,v] of Object.entries(attrs))n.setAttribute(k,v);if(text!==undefined)n.textContent=text;return n;};
const button=(text,fn,cls='')=>{const b=document.createElement('button');b.textContent=text;b.className=cls;b.onclick=fn;return b;};
let data,rawData,lookup,state,view,drag,lastMove=0,history=[],views={},toastTimer,config;
let drawingCache,lastViewport,canvasBox,overlayAnchor,pendingFrame=0,lastConnectorKey='';
const textMeasure=document.createElement('canvas').getContext('2d'),textWidths=new Map();textMeasure.font='11px system-ui';
function textWidth(text){if(!textWidths.has(text)){if(textWidths.size>=2048)textWidths.clear();textWidths.set(text,textMeasure.measureText(text).width);}return textWidths.get(text);}
// Input changes the camera immediately; paint the latest camera once per
// animation frame. Explicit selection/settings renders remain synchronous.
function requestDraw(){if(!pendingFrame)pendingFrame=requestAnimationFrame(()=>{pendingFrame=0;draw(true);});}
let origin=228,destination=null,route={status:'idle',ids:[],edges:[]},prefs=defaults(),hints={};
const placeCache=new Map();
function markersFor(id){
 if(!placeCache.has(id))placeCache.set(id,placeMarkers(hints[id]?.features||[]).map(m=>resolvePlaceMarker(data,lookup,hints,m)));
 return placeCache.get(id);
}
let regional,overview,hunting,editor,setup,habitatRooms=new Set();
const preferenceKey='hydra.map.preferences.v1';
try{prefs=cleanPreferences(JSON.parse(storage?.getItem(preferenceKey)));}catch{/* Storage is optional. */}
function savePreferences(){try{storage?.setItem(preferenceKey,JSON.stringify(prefs));}catch{}draw();}
function recalculate(){route=destination===null?{status:'idle',ids:[],edges:[]}:source.previewRoute(data.rooms,origin,destination,{metric:$('route-metric').value,scripted:$('route-scripted').checked});}
function clearRoute(){destination=null;recalculate();render();}
function displayTransitions(drawing=scene()){
 const rows=new Map(transitions(data,lookup,state.area,state.unit).map(t=>[t.id,t]));
 for(const e of drawing.sheet.edges.filter(e=>e.kind==='Continuation'))for(const r of e.records){
  if(!e.important_connector&&state.mode!=='explorer'&&lookup[r.from].unit!==state.unit&&!route.edges.some(step=>step.from===r.from&&step.to===r.to))continue;
  const id=`${r.from}:${r.ordinal}`;
  rows.set(id,{...rows.get(id),id,from:r.from,to:r.to,edge:r.edge,bundled:true,
   destination:`${title(data.rooms[r.to])} #${r.to} · ${e.inset_link?'linked schematic inset':'same-map continuation'}`,
   crossArea:false,continuation:true,important:!!e.important_connector,independentFrames:!!e.independent_frames,
   connectorKey:[e.a_room,e.b_room].sort((a,b)=>a-b).join(':')});
 }
 return [...rows.values()];
}
function svgPath(points){return points.map((p,i)=>`${i?'L':'M'} ${p.x} ${p.y}`).join(' ');}
function areaName(area){return data.scenes[area]?.label||area;}
function buildRegionBrowser(){
 const q=$('region-filter').value.trim().toLowerCase(),select=$('region-select');select.replaceChildren();
 const empty=document.createElement('option');empty.value='';empty.textContent='Choose area or unassigned group…';select.append(empty);
 for(const [label,unassigned] of [['Assigned areas / existing demo',false],['Region only · area unassigned',true]]){
  const group=document.createElement('optgroup');group.label=label;
  for(const [key,s] of Object.entries(data.scenes).sort((a,b)=>a[1].label.localeCompare(b[1].label))){
   if((s.assignment_kind==='region-only')!==unassigned)continue;
   if(q&&!`${s.label} ${s.sheet.rooms.map(r=>title(data.rooms[r.id])).join(' ')}`.toLowerCase().includes(q))continue;
   const option=document.createElement('option');option.value=key;option.textContent=`${s.label} (${s.sheet.rooms.length})`;group.append(option);
  }
  if(group.children.length)select.append(group);
 }
 select.onchange=()=>{const s=data.scenes[select.value];if(s)go(s.sheet.rooms[0].id,{select:false,fit:true});};
}
function buildSettings(){
 $('category-controls').replaceChildren(...Object.entries(categories).map(([key,c])=>{
  const row=document.createElement('div');row.className='category-setting';const label=document.createElement('label'),check=document.createElement('input'),color=document.createElement('input');
  check.type='checkbox';check.checked=prefs.layers[key];check.dataset.category=key;check.onchange=()=>{prefs.layers[key]=check.checked;savePreferences();};
  let glyph=icon(c,prefs.colors[key]);label.append(check,glyph,document.createTextNode(c.name));color.type='color';color.value=prefs.colors[key];color.setAttribute('aria-label',c.name+' color');color.oninput=()=>{prefs.colors[key]=color.value;const next=icon(c,color.value);glyph.replaceWith(next);glyph=next;savePreferences();};row.append(label,color);return row;
 }));
 for(const [id,key,prop] of [['place-labels','labels','value'],['hide-labels','hideLabels','checked'],['transition-labels','transitions','value'],['room-numbers','numbers','checked'],['route-animation','animate','checked'],['route-color','routeColor','value']]){
  $(id)[prop]=prefs[key];$(id).onchange=()=>{prefs[key]=$(id)[prop];savePreferences();};
 }
 root.querySelectorAll('[data-preset]').forEach(b=>b.onclick=()=>{prefs=preset(b.dataset.preset,prefs);buildSettings();savePreferences();});
}
function renderRoute(){
 const start=`Preview start: ${hints[origin]?.name||title(data.rooms[origin])} #${origin}`;
 $('route-status').textContent=route.status==='idle'?`${start} · Select a room to preview a route.`:route.status==='found'?`${start} → ${hints[destination]?.name||title(data.rooms[destination])} #${destination} · ${route.edges.length} exits · ${$('route-metric').value==='steps'?'fewest exits':'recorded cost '+Number(route.cost.toFixed(3))}${route.scripted?' · includes unverified scripted passages':''}`:`${start} → #${destination} · No eligible route in the bundled data. This does not mean the destination is inaccessible in game.`;
 $('next-step').disabled=route.ids.length<2;$('clear-route').disabled=destination===null;
 $('route-visits').replaceChildren(...routeVisits(route,lookup).map((v,i)=>button(`${i+1} · ${areaName(v.area)} (${v.ids.length} rooms)`,()=>{go(v.ids[0],{select:false});fitRooms(v.ids.map(id=>lookup[id]));},v.area===state.area?'active':'')));
 $('route-crossings').replaceChildren(...routeOnMap(route,lookup,state.area).crossings.map(e=>{
  const leaving=lookup[e.from].area===state.area,other=leaving?e.to:e.from;
  const b=button(`${leaving?'↗ Continue into':'↘ Arrives from'} ${areaName(lookup[other].area)} · ${title(data.rooms[e.from])} #${e.from} → ${title(data.rooms[e.to])} #${e.to} · ${command(e.edge)}`,()=>go(other,{select:false,fit:true}),'route-crossing');b.dataset.routeCrossing=`${e.from}:${e.to}`;return b;
 }));
 $('route-steps').replaceChildren(...route.edges.map(e=>{const li=document.createElement('li');li.textContent=`#${e.from} — ${command(e.edge)} → ${title(data.rooms[e.to])} #${e.to}${!e.edge.cmd?' (unverified)':''}`;return li;}));
}
function notify(text){$('toast').textContent=text;$('toast').hidden=false;clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('toast').hidden=true,4000);}
function scene(){return data.scenes[state.area];}
function fitRooms(rooms){view=bounds(rooms);aspect();views[state.area]={...view};draw();}
function aspect(){const r=$('canvas').getBoundingClientRect();if(!r.width||!r.height)return;const ratio=r.width/r.height,cx=view.x+view.w/2,cy=view.y+view.h/2;if(view.w/view.h<ratio)view.w=view.h*ratio;else view.h=view.w/ratio;view.x=cx-view.w/2;view.y=cy-view.h/2;lastViewport={width:r.width,height:r.height};}
// Fit expands bounds once; ordinary resize must be reversible, not repeatedly
// expand both axes as scrollbar/viewport dimensions alternate.
function resizeCanvas(){
 if(!view)return;const r=$('canvas').getBoundingClientRect();if(!r.width||!r.height)return;
 if(lastViewport){const px=view.w/lastViewport.width,cx=view.x+view.w/2,cy=view.y+view.h/2;view.w=px*r.width;view.h=px*r.height;view.x=cx-view.w/2;view.y=cy-view.h/2;}
 lastViewport={width:r.width,height:r.height};draw();
}
function remember(){history.push({state:{...state},view:{...view}});if(history.length>80)history.shift();}
function go(id,{record=true,fit=false,select=true}={}){
 if(select&&setup?.room(Number(id)))return;
 const next=visit(state,lookup,Number(id));
 if(!next){outside(Number(id));return;}
 if(record)remember();
 if(select){destination=Number(id);recalculate();}
 views[state.area]={...view};const changed=next.area!==state.area;state=next;
 if(changed){view=views[state.area] ? {...views[state.area]} : bounds(scene().sheet.rooms);aspect();}
 if(fit){view=bounds(scene().sheet.rooms.filter(r=>r.unit===state.unit));aspect();}
 else if(!changed){const p=lookup[id].cell;if(p.x<view.x||p.x>view.x+view.w||p.y<view.y||p.y>view.y+view.h){view.x=p.x-view.w/2;view.y=p.y-view.h/2;}}
 onNavigate(`room=${state.room}&mode=${state.mode}`);render();
}
function outside(id){if(onOutside){onOutside(id);return;}const r=data.outside[id];$('dialog-title').textContent=title(r||{id});$('dialog-body').replaceChildren();const p=document.createElement('p');p.textContent=`Recorded destination #${id}${r?.location?' · '+r.location:''}. This destination is not bundled in the ${config.title} test. The exit is preserved, but this preview does not invent its map, access conditions, or a return path.`;$('dialog-body').append(p);$('dialog').showModal();}
function card(t){
 const b=button('',()=>go(t.to),'transition-link'+(t.role==='area'?' cross':'')+(t.from===state.room?' current':''));
 const strong=document.createElement('strong');strong.textContent=`↗ ${t.destination}`;
 const small=document.createElement('small');small.textContent=`${command(t.edge)} · #${t.from} → #${t.to}${t.role==='unassigned'?' · area unassigned; separate preview':''}${t.bundled?' · open preview':' · not bundled'}`;
 b.append(strong,small);b.dataset.from=t.from;b.dataset.to=t.to;return b;
}
function openTransitions(links,place=null){
 if(links.length===1&&!place?.local){go(links[0].to,{fit:!!place});return;}
 $('dialog-title').textContent=place?place.name+' · choose a destination':'Exits from '+title(data.rooms[links[0].from]);$('dialog-body').replaceChildren(...links.map(t=>{const b=card(t);b.onclick=()=>{$('dialog').close();go(t.to,{fit:!!place});};return b;}));
 if(place?.local)$('dialog-body').prepend(button('View '+title(data.rooms[place.from])+` #${place.from}`,()=>{$('dialog').close();go(place.from,{fit:true});}));
 $('dialog').showModal();
}
function activateLabel(node,links,place=null){
 node.style.pointerEvents='all';node.style.cursor='pointer';node.setAttribute('role','button');node.setAttribute('tabindex','0');
 node.setAttribute('aria-label',links.length===1?'Preview '+links[0].destination:'Choose from '+links.length+' recorded exits');
 node.onclick=e=>{e.stopPropagation();if(Date.now()-lastMove>180)openTransitions(links,place);};
 node.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();openTransitions(links,place);}};
}
function render(){
 hunting?.update(state.area);
 editor?.update(state.area);
 root.querySelector('h1').textContent=state.area==='town'?config.title:hunting?.active?scene().label:config.title;
 $('layout-controls').hidden=!data.presentation&&state.area!=='catacombs'&&!scene().reference_layout;
 $('layout-mode').parentElement.hidden=!data.presentation&&state.area!=='catacombs';
 const audit=scene().display_layout;
 const reference=scene().reference_layout;
 $('layout-note').textContent=reference?`Artwork-guided layout · ${reference.anchored}/${scene().sheet.rooms.length} rooms with reference coordinates · ${reference.adjusted.length} shared-coordinate adjustments · ${reference.insets.reduce((n,s)=>n+s.rooms.length,0)} rooms in schematic insets. Independent images are separate display frames, not floors. Recorded exits unchanged.`:state.area==='catacombs'?(audit?`Classic-map alignment · ${audit.anchored} anchored rooms · ${audit.insets.length} schematic insets · ${audit.bearing_conflicts.length/2} bearing-conflict pair. Recorded exits unchanged.`:'Original engine export · uncorrected placement for comparison.'):'';
 if(data.presentation&&!reference)$('layout-note').textContent='Native positions from the recorded engine revision; automated area placements need review. Artwork comparison is optional; missing artwork does not imply broken geometry.';
 if(data.presentation&&!reference){const rooms=scene().sheet.rooms,overlaps=rooms.length-new Set(rooms.map(r=>`${r.cell.x}:${r.cell.y}`)).size;if(overlaps)$('layout-note').textContent+=` Warning: ${overlaps} additional rooms share native positions. Try artwork comparison or room search; the source placement has not been silently repaired.`;}
 $('section-focus').replaceChildren(...(scene().display_sections||[]).map(s=>button(s.name,()=>fitRooms(s.rooms.map(id=>lookup[id])))));
 $('back').disabled=!history.length;
 const world=document.createElement('a');world.href='../corpus/';world.textContent='World';
 const region=button(config.title,()=>overview?.open());region.setAttribute('aria-label','Region overview: '+config.title);
 const area=document.createElement('span');area.textContent=scene().label;area.setAttribute('aria-current','page');
 $('breadcrumb').setAttribute('aria-label','Map hierarchy');$('breadcrumb').replaceChildren(world,document.createTextNode(' › '),region,document.createTextNode(' › '),area);
 $('focus-name').textContent=hunting?.active?scene().label:focusName(data,state.area,state.unit);
 $('focus-mode').setAttribute('aria-pressed',state.mode==='focus');$('explore-mode').setAttribute('aria-pressed',state.mode==='explorer');
 $('area-tabs').replaceChildren(...[...new Set([...config.primary_tabs,state.area])].filter(a=>data.scenes[a]).map(a=>button(a==='town'?config.home_label:a==='catacombs'?config.underground_label:areaName(a),()=>go(config.tab_rooms[a]||data.scenes[a].sheet.rooms[0].id,{select:false}),a===state.area?'active':'')));
 $('units').replaceChildren(...scene().units.flatMap((u,i)=>{if(!u.rooms.length)return [];const b=button(focusName(data,state.area,i),()=>go(u.door_rooms[0]||u.rooms[0],{fit:true}),i===state.unit?'active':'');const n=document.createElement('small');n.textContent=u.rooms.length;b.append(n);b.dataset.unit=i;return [b];}));
 const r=data.rooms[state.room];$('room-name').textContent=`${title(r)} · #${r.id}`;
 $('description').textContent=r.description?.[0]||'No room description in this snapshot.';
 $('room-meta').textContent=`Display: ${scene().label}  ·  Area: ${r.assignment?.area||'unassigned'}  ·  Region: ${r.assignment?.region||'unknown'}  ·  Status: ${r.status||'live'}  ·  Focus: ${focusName(data,state.area,state.unit)}  ·  Game UID: ${(r.uid||[]).join(', ')||'unknown'}`;
 $('exits').replaceChildren(...r.exits.map(e=>{const t=lookup[e.to],b=button(`${command(e)} → ${t?title(data.rooms[e.to]):title(data.outside[e.to])} #${e.to}`,()=>go(e.to),!t||t.area!==state.area||t.unit!==state.unit?'transition':'');b.dataset.to=e.to;return b;}));
 const evidence=[r.ownership_source,...(r.assignment?[`Exported area key: ${r.assignment.area_key||'none'}; region source: ${r.assignment.region_src||'unknown'}. Display frame and focus do not assign area ownership.`]:[]),'Base layout and focus unit: cena-map-layout '+data.provenance.engine_revision.slice(0,12),...Object.values(data.reviewed).filter(ref=>ref.member_ids.includes(r.id)).map(ref=>'Human review: '+ref.rationale)];
 const bridge=data.assignment_import?.display_bridges?.find(b=>b.rooms.includes(r.id));
 if(bridge)evidence.unshift(`Unassigned bridge display context: matching reciprocal compass anchors ${bridge.anchors.map(id=>'#'+id).join(', ')}. Source area remains unassigned; no new exit or ownership inferred.`);
 if(audit)evidence.push(`Display position: ${lookup[r.id].display_position_source}. ${audit.note}`,...audit.bearing_conflicts.filter(e=>e.from===r.id).map(e=>`#${e.from} → #${e.to}: ${e.command}. ${e.reason}`));
 if(scene().town_alignment)evidence.push(scene().town_alignment.note);
 if(reference)evidence.push(reference.note,lookup[r.id].display_position_source);
 if(scene().group_placement)evidence.push(scene().group_placement.rule,lookup[r.id].display_position_source||'Unmoved native display position');
 $('evidence').replaceChildren(...evidence.map(t=>{const p=document.createElement('p');p.textContent=t;return p;}));
 const ts=transitions(data,lookup,state.area,state.unit),bundled=ts.filter(t=>t.bundled),unbundled=ts.filter(t=>!t.bundled);
 const major=bundled.filter(t=>t.role==='area'),local=bundled.filter(t=>t.role!=='area');
 $('connection-context').textContent=`${major.length} area transitions · ${local.length} local or unassigned links. Gold marks area transitions; small blue arrows open other previews.`;
 $('transitions').replaceChildren(...major.map(card));if(!major.length){const p=document.createElement('p');p.textContent='No confirmed area transitions from this focus.';$('transitions').append(p);}
 let localList=$('local-connections');
 if(!localList){localList=document.createElement('details');localList.id='local-connections';localList.append(document.createElement('summary'),document.createElement('div'));$('transitions').after(localList);}
 localList.querySelector('summary').textContent=`${local.length} doorways & unassigned destinations`;
 localList.querySelector('div').replaceChildren(...local.map(card));
 $('outside-list').querySelector('summary').textContent=`${unbundled.length} other recorded exits · destinations not bundled`;
 $('outside-list').querySelector('div').replaceChildren(...unbundled.map(card));
 const underground=config.underground_label&&data.catacomb_boundary;
 $('all-access').hidden=!underground;
 if(underground){
  $('all-access').querySelector('summary').textContent=`All ${underground.external_endpoints.length} ${config.underground_label.toLowerCase()} connection points`;
  $('catacomb-access').replaceChildren(...underground.external_endpoints.map(id=>{
   const group=document.createElement('section'),name=button('⌖ '+title(data.rooms[id]||data.outside[id]),()=>go(id,{fit:true}));name.className='access-name';group.append(name);
   for(const edge of underground.directed_boundary_edges.filter(e=>e.external_endpoint===id)){
    const inward=edge.from===id,b=button(`${inward?'→ Into ':'← Out of '}${config.underground_label.toLowerCase()} · ${command(edge.edge)}${lookup[edge.to]?'':' · not bundled'}`,()=>go(edge.to));b.dataset.from=edge.from;b.dataset.to=edge.to;group.append(b);
   }return group;
  }));
 }else $('catacomb-access').replaceChildren();
 $('town-context').hidden=state.area==='town'&&state.unit===0;
 if(!$('town-context').hidden){
  const s=data.scenes.town.sheet,b=bounds(s.rooms),svg=el('svg',{viewBox:`${b.x} ${b.y} ${b.w} ${b.h}`,class:'context-svg','aria-hidden':'true'});
  for(const e of s.edges)if(e.unit===null)svg.append(el('line',{x1:e.a.x,y1:e.a.y,x2:e.b.x,y2:e.b.y,stroke:'#7c99ad','stroke-width':.25}));
  const local=state.area==='town'?lookup[state.room]:null;
  if(local)svg.append(el('circle',{cx:local.cell.x,cy:local.cell.y,r:2,fill:'#ffc778'}));
  $('town-context').replaceChildren(svg,document.createTextNode(state.area==='town'?`⌂ ${config.home_label} overview`:`⌂ ${config.home_label} · separate frame`));
 }
 regional?.update(state.area);renderRoute();draw();
}
function draw(cameraOnly=false){
 if(!view||!data)return;
 hunting?.setEditing(editor?.preview()||null);
 if(pendingFrame){cancelAnimationFrame(pendingFrame);pendingFrame=0;}
 const svg=$('map'),overlay=$('labels'),box=$('canvas').getBoundingClientRect();
 canvasBox=box;
 const px=view.w/box.width,active=r=>state.mode==='explorer'||r.unit===state.unit||habitatRooms.has(r.id);
 const geometryChanged=drawingCache?.scene!==scene()||drawingCache.px!==px,rebuild=!cameraOnly||geometryChanged;
 // Pure translation preserves every room/label clearance. Move the existing
 // overlay with the map during a drag; reflow newly visible labels on release.
 if(cameraOnly&&!geometryChanged&&drag?.moved&&overlayAnchor){
  svg.setAttribute('viewBox',`${view.x} ${view.y} ${view.w} ${view.h}`);
  overlay.style.transform=`translate(${(overlayAnchor.x-view.x)/px}px, ${(overlayAnchor.y-view.y)/px}px)`;
  return;
 }
 overlay.style.transform='';overlayAnchor={x:view.x,y:view.y};
 // Read fixed UI obstacles before changing SVG/sidebars, avoiding a forced
 // browser layout half way through rebuilding the overlay.
 const fixedObstacles=[$('town-context'),root.querySelector('.map-key')].filter(n=>n&&!n.hidden).map(n=>{const b=n.getBoundingClientRect();return {x:b.x-box.x,y:b.y-box.y,w:b.width,h:b.height};});
 if(geometryChanged)drawingCache={scene:scene(),px,drawing:cameraDrawing(scene(),data.rooms,px*10)};
 const drawing=drawingCache.drawing;
 svg.setAttribute('viewBox',`${view.x} ${view.y} ${view.w} ${view.h}`);overlay.setAttribute('viewBox',`0 0 ${box.width} ${box.height}`);overlay.replaceChildren();
 if(rebuild){
 svg.replaceChildren();
 const unmapped=scene().reference_layout?.insets.flatMap(s=>s.rooms)||[];
 if(unmapped.length){const b=bounds(unmapped.map(id=>lookup[id]));svg.append(el('rect',{x:b.x,y:b.y,width:b.w,height:b.h,rx:px*5,fill:'#181823','fill-opacity':.35,stroke:'#9a8cad','stroke-width':px,'stroke-dasharray':`${px*4} ${px*5}`,'pointer-events':'none','data-schematic-panel':'true'}));}
 const edgeLayer=el('g',{'pointer-events':'none'});
 for(const e of drawing.sheet.edges){
  if(e.kind==='Continuation')continue;
  const strong=state.mode==='explorer'||e.unit===state.unit||(state.unit===0&&e.unit===null)||(habitatRooms.has(e.a_room)&&habitatRooms.has(e.b_room));
  const attrs={stroke:strong?'#6e99b5':'#385164','stroke-width':px*(strong?1.35:.8),opacity:strong?.9:.65};
  if(e.kind==='Connector')attrs['stroke-dasharray']=`${px*5} ${px*4}`;
  if(e.points){edgeLayer.append(el('path',{d:svgPath(e.points),fill:'none',...attrs,'data-layout-edge':`${e.a_room}:${e.b_room}`,'data-points':JSON.stringify(e.points),...(e.supplemented?{'data-native-passage':[e.a_room,e.b_room].sort((a,b)=>a-b).join(':')}:{})}));}
  else if(e.kind==='Stub'){
   const dx=e.b.x-e.a.x,dy=e.b.y-e.a.y,len=Math.hypot(dx,dy),ratio=Math.min(.2,px*15/(len||1));
   for(const [a,b] of [[e.a,{x:e.a.x+dx*ratio,y:e.a.y+dy*ratio}],[e.b,{x:e.b.x-dx*ratio,y:e.b.y-dy*ratio}]])edgeLayer.append(el('line',{x1:a.x,y1:a.y,x2:b.x,y2:b.y,...attrs,'stroke-dasharray':`${px*3} ${px*2}`}));
  }else edgeLayer.append(el('line',{x1:e.a.x,y1:e.a.y,x2:e.b.x,y2:e.b.y,...attrs}));
 }
 svg.append(edgeLayer);
 // Missing native drawing edges are supplemented by obstacleSafeScene above,
 // so they obey exactly the same clearance rule as every other passage.
 // Only edges entirely on this map are drawn. Cross-map steps are linked
 // callouts, never lines joining unrelated coordinate systems.
 const routeLayer=el('g',{'pointer-events':'none',class:prefs.animate?'route-lines animated':'route-lines'});
 for(const e of routeOnMap(route,lookup,state.area).segments){
  const points=passagePoints(drawing,e.from,e.to,lookup);if(!points)continue;
  const attrs={d:svgPath(points),fill:'none',stroke:prefs.routeColor,'stroke-width':px*2.5,'data-route-from':e.from,'data-route-to':e.to,'data-points':JSON.stringify(points)};
  routeLayer.append(el('path',{...attrs,opacity:.3}));
  const flow=el('path',{...attrs,class:'route-flow','stroke-dasharray':`${px*9} ${px*9}`});flow.style.setProperty('--flow-length',`${-px*18}px`);routeLayer.append(flow);
  const longest=points.slice(1).map((b,i)=>[points[i],b]).sort((a,b)=>Math.hypot(b[0].x-b[1].x,b[0].y-b[1].y)-Math.hypot(a[0].x-a[1].x,a[0].y-a[1].y))[0],a=longest[0],b=longest[1];
  const dx=b.x-a.x,dy=b.y-a.y,len=Math.hypot(dx,dy);
  if(len>px*22){const ux=dx/len,uy=dy/len,mx=(a.x+b.x)/2,my=(a.y+b.y)/2;routeLayer.append(el('path',{d:`M ${mx+ux*px*4} ${my+uy*px*4} L ${mx-ux*px*3-uy*px*3} ${my-uy*px*3+ux*px*3} L ${mx-ux*px*3+uy*px*3} ${my-uy*px*3-ux*px*3} Z`,fill:prefs.routeColor}));}
 }
 svg.append(routeLayer);
 }
 const ts=displayTransitions(drawing),byFrom=new Map();
 for(const t of ts){if(!t.bundled&&!$('unbundled-markers').checked&&t.from!==state.room)continue;if(!byFrom.has(t.from))byFrom.set(t.from,[]);byFrom.get(t.from).push(t);}
 const continuations=ts.filter(t=>t.continuation);
 const connectorKey=JSON.stringify([state.room,state.area,continuations.map(t=>[t.id,t.destination,t.important])]);
 if(connectorKey!==lastConnectorKey){
 lastConnectorKey=connectorKey;
 let keyConnectors=$('key-connectors');
 if(!keyConnectors){keyConnectors=document.createElement('section');keyConnectors.id='key-connectors';$('drawing-connections').before(keyConnectors);}
 const important=continuations.filter(t=>t.important);keyConnectors.hidden=!important.length;keyConnectors.replaceChildren();
 if(important.length){const heading=document.createElement('p');heading.textContent=`${important.length} important connections · separate display frames or too crowded for a safe line at this scale. Inspect the other end below.`;const list=document.createElement('div');list.className='connector-scroll';list.setAttribute('aria-label','Important connections');list.tabIndex=0;list.append(...important.map(t=>{const b=card(t);b.dataset.importantConnector=t.connectorKey;return b;}));keyConnectors.append(heading,list);}
 $('drawing-connections').hidden=!continuations.length;
 $('drawing-connections').querySelector('summary').textContent=`${continuations.length} directed linked continuations · separate display frames or no clear line at this zoom`;
 $('drawing-links').replaceChildren(...continuations.map(card));
 }
 if(rebuild){
 for(const r of scene().sheet.rooms){
  const category=hints[r.id]?.kinds.find(k=>prefs.layers[k]),selected=r.id===state.room,on=active(r),size=px*(selected?12:on?10:r.entrance?10:8),attrs={'data-room':r.id,'data-highlight':category||'','data-position-kind':r.reference_kind||'',class:'node',role:'button',tabindex:'-1','aria-label':`${title(data.rooms[r.id])}, room ${r.id}`,fill:selected?'#f7f5df':category?prefs.colors[category]:hunting?.color(r.id)||(on?'#497fa3':'#7c95a7'),stroke:selected?'#fff2c1':on?'#a2c8df':'#9eb4c3','stroke-width':px};
  const shape=on?el('rect',{x:r.cell.x-size/2,y:r.cell.y-size/2,width:size,height:size,rx:px*1.4,...attrs}):el('circle',{cx:r.cell.x,cy:r.cell.y,r:size/2,...attrs});
  const places=(hints[r.id]?.features||[]).filter(f=>prefs.layers[f.key]).map(f=>`${f.entrance?'Entrance to ':''}${f.name} · ${categories[f.key].name}${f.entrance?' · '+command(f.edge)+' → #'+f.to:''}`);
  shape.append(el('title',{},`${r.title} #${r.id} · ${focusName(data,state.area,r.unit)}${places.length?'\n'+places.join('\n'):''}${category?'\n'+hints[r.id].evidence:''}${r.reference_kind==='schematic'?' · Schematic inset: no artwork coordinates':''}`));
  svg.append(shape);
  const huntingInfo=hunting?.roomInfo(r.id);if(huntingInfo)shape.querySelector('title').textContent+='\n'+huntingInfo;
  if(habitatRooms.has(r.id))svg.append(el('circle',{cx:r.cell.x,cy:r.cell.y,r:px*7,fill:'none',stroke:'#d9a4ff','stroke-width':px*2,'pointer-events':'none','data-habitat-room':r.id}));
  if(selected)svg.append(el('circle',{cx:r.cell.x,cy:r.cell.y,r:px*10,fill:'none',stroke:'#fff3c9','stroke-width':px,'pointer-events':'none'}));
 }
 }
 // Transition markers are screen-sized and remain legible when zoomed out.
 const occupied=scene().sheet.rooms.map(r=>{const p=r.cell,radius=byFrom.has(r.id)?22:15;return {x:(p.x-view.x)/px-radius,y:(p.y-view.y)/px-radius,w:radius*2,h:radius*2};});
 occupied.push(...fixedObstacles);
 function offsetLabel(text,x,y,attrs={},fill='#ffce83',background=true){
  if(prefs.hideLabels)return null;
  const placed=placeLabel({x,y},{w:Math.ceil(textWidth(text))+14,h:21},occupied,box);
  if(!placed)return null;occupied.push(placed);
  const {x:lx,y:ly,w:width,h:height}=placed,g=el('g',{'data-label-box':`${lx},${ly},${width},${height}`,...attrs});
  // Keep the non-interactive leader out of the label's hit box: otherwise a
  // click on the group's bounding-box centre can fall in empty space.
  overlay.append(el('line',{x1:x,y1:y,x2:Math.max(lx,Math.min(x,lx+width)),y2:Math.max(ly,Math.min(y,ly+height)),stroke:fill,opacity:.45,'stroke-width':.8,'stroke-dasharray':'2 3','pointer-events':'none'}));
  if(background)g.append(el('rect',{x:lx,y:ly,width,height,rx:3,fill:'#181e25',stroke:fill,'stroke-opacity':.5}));
  g.append(el('text',{x:lx+7,y:ly+14,fill,'font-size':11,'font-family':'system-ui'},text));overlay.append(g);return g;
 }
 const groups=[...byFrom].sort((a,b)=>Number(b[0]===state.room)-Number(a[0]===state.room)||Number(b[1].some(t=>t.crossArea))-Number(a[1].some(t=>t.crossArea))||Number(b[1].some(t=>t.bundled))-Number(a[1].some(t=>t.bundled))||a[0]-b[0]);
 for(const [from,links] of groups){
  const p=lookup[from].cell,x=(p.x-view.x)/px,y=(p.y-view.y)/px;
  if(x<0||y<0||x>box.width||y>box.height)continue;
  const major=links.some(t=>t.role==='area'||t.role==='outside'||t.important),color=major?'#ffc778':'#8db6ce';
  const marker=el('g',{'data-transition-source':from,'data-transition-role':major?'area':'local'});
  if(major)marker.append(el('circle',{cx:x,cy:y,r:9,fill:'transparent',stroke:color,'stroke-width':1.5}));
  marker.append(el('text',{x:x+10,y:y-7,'text-anchor':'middle',fill:color,'font-size':major?12:10,'font-family':'system-ui',stroke:'#0b1119','stroke-width':2,'paint-order':'stroke'},major?'↗':'↳'));activateLabel(marker,links);overlay.append(marker);
  marker.append(el('title',{},links.map(t=>`${t.destination} · ${command(t.edge)} · #${from} → #${t.to}`).join('\n')));
  if(prefs.transitions==='symbols'||(!major&&from!==state.room))continue;
  const named=major?links.filter(t=>t.role==='area'||t.role==='outside'||t.important):links;
  const names=[...new Set(named.map(t=>t.destination))],name=names.length===1?names[0]:`${names.length} destinations`,detail=prefs.transitions==='details'&&named.length===1?` · ${command(named[0].edge)}`:'';
  const full=name+detail,label=full.length>46?full.slice(0,44)+'…':full;
  const link=offsetLabel(label,x,y,{'data-transition-label':from},color);if(link){link.append(el('title',{},full));activateLabel(link,named);}
 }
 // Place labels yield space to transitions. Symbols and color are redundant
 // cues; category switches never remove rooms or their connectivity.
 const labeled=new Set();
 const placeRooms=[...scene().sheet.rooms].sort((a,b)=>Number(hints[b.id]?.features.some(f=>prefs.layers[f.key]&&categories[f.key].icon))-Number(hints[a.id]?.features.some(f=>prefs.layers[f.key]&&categories[f.key].icon))||a.id-b.id);
 for(const r of placeRooms){
  const hint=hints[r.id],x=(r.cell.x-view.x)/px,y=(r.cell.y-view.y)/px;
  if(x<0||y<0||x>box.width||y>box.height)continue;
  if(!active(r))continue;
  for(const feature of markersFor(r.id)){
   const {key,name}=feature;if(!prefs.layers[key])continue;
   // Distinct places stay distinct. Repeated exits into the same place at
   // this source share one marker whose chooser retains all destinations.
   const identity=feature.entrance&&!feature.approach?feature.identity:`${key}:${name}`;
   if(labeled.has(identity))continue;
   let caption=prefs.hideLabels||prefs.labels==='symbols'?'':name;
   if(caption.length>34)caption=caption.slice(0,32)+'…';
   if(caption&&feature.links.length>1)caption+=` · ${feature.links.length} exits`;
   const fullWidth=caption?Math.ceil(textWidth(caption))+34:24;
   let placed=placeLabel({x,y},{w:fullWidth,h:24},occupied,box);
   // A crowded name yields to its compact icon; never obscure a room.
   if(!placed&&caption){caption='';placed=placeLabel({x,y},{w:24,h:24},occupied,box);}
   if(!placed)continue;occupied.push(placed);labeled.add(identity);
   const color=prefs.colors[key],g=el('g',{'data-place-label':r.id,'data-place-name':caption?name:'','data-place-to':feature.to,'data-place-targets':feature.links.map(f=>f.to).join(','),'data-place-kind':key,[caption?'data-label-box':'data-icon-box']:`${placed.x},${placed.y},${placed.w},${placed.h}`});
   overlay.append(el('line',{x1:x,y1:y,x2:Math.max(placed.x,Math.min(x,placed.x+placed.w)),y2:Math.max(placed.y,Math.min(y,placed.y+placed.h)),stroke:color,opacity:.45,'stroke-width':.8,'stroke-dasharray':'2 3','pointer-events':'none'}));
   g.append(el('rect',{x:placed.x,y:placed.y,width:placed.w,height:placed.h,rx:4,fill:'#111a25',stroke:color,'stroke-opacity':.65}));
   const glyph=icon(categories[key],color);glyph.setAttribute('x',placed.x+3);glyph.setAttribute('y',placed.y+3);g.append(glyph);
   if(caption)g.append(el('text',{x:placed.x+26,y:placed.y+16,fill:color,'font-size':11,'font-family':'system-ui'},caption));
   const description=`${feature.entrance?'Entrance to ':''}${name} · ${categories[key].name}${feature.links.length?' · '+feature.links.map(f=>`${f.approach?.length?'Via same-place approach · ':''}${command(f.edge)} · #${f.from} → #${f.to} (${title(data.rooms[f.to])})`).join('; '):''}`;
   g.append(el('title',{},description));
   if(feature.entrance)activateLabel(g,feature.links.map(f=>({from:f.from,to:f.to,edge:f.edge,destination:feature.links.length>1?title(data.rooms[f.to]):name,bundled:true})),feature);
   else{g.style.pointerEvents='all';g.setAttribute('role','button');g.setAttribute('tabindex','0');g.onclick=e=>{e.stopPropagation();if(Date.now()-lastMove>180)go(r.id,{fit:true});};g.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();go(r.id,{fit:true});}};}
   g.setAttribute('aria-label',description);
   g.oncontextmenu=e=>{if(destination===feature.from&&feature.local||feature.links.some(f=>f.to===destination)){e.preventDefault();e.stopPropagation();clearRoute();}};
   // SVG hover titles and keyboard focus expose the full name, even when the
   // visible caption is disabled or collision-suppressed.
   g.onfocus=()=>{const p=document.createElement('p');p.textContent=description;$('place-focus').replaceChildren(p);};
   g.onblur=()=>{$('place-focus').replaceChildren();};
   overlay.append(g);
  }
 }
 for(const [id,name] of [[origin,'START'],[destination,'DESTINATION']]){
  if(!lookup[id]||lookup[id].area!==state.area)continue;const p=lookup[id].cell,x=(p.x-view.x)/px,y=(p.y-view.y)/px;
  const g=el('g',{'data-route-endpoint':name});g.append(el('circle',{cx:x,cy:y,r:12,fill:'none',stroke:prefs.routeColor,'stroke-width':2,'stroke-dasharray':name==='START'?'none':'3 2'}));g.append(el('title',{},`${name} #${id}${name==='DESTINATION'?' · Right-click to clear route':''}`));overlay.append(g);const caption=offsetLabel(name,x,y,name==='DESTINATION'?{'data-route-destination':id}:{},prefs.routeColor);if(caption&&name==='DESTINATION'){caption.style.pointerEvents='all';caption.oncontextmenu=e=>{e.preventDefault();e.stopPropagation();clearRoute();};}
 }
 const p=lookup[state.room].cell,x=(p.x-view.x)/px,y=(p.y-view.y)/px;
 offsetLabel(`SELECTED #${state.room}`,x,y,{},'#fff1c4');
 hunting?.paint({svg:rebuild?svg:null,overlay,el,scene:scene(),drawing,lookup,view,px,box,occupied,textWidth,hideLabels:prefs.hideLabels});
 for(const id of scene().display_layout?.insets||[]){const p=lookup[id].cell;offsetLabel('Schematic inset · '+title(data.rooms[id]),(p.x-view.x)/px,(p.y-view.y)/px,{'data-layout-inset':id},'#d7b3ee');}
 if(prefs.numbers)for(const r of scene().sheet.rooms){const x=(r.cell.x-view.x)/px,y=(r.cell.y-view.y)/px;if(x>=0&&y>=0&&x<=box.width&&y<=box.height)offsetLabel(String(r.id),x,y,{'data-room-number':r.id},'#a9c3d4');}
 editor?.paint({overlay,el,lookup,view,px});
}
function zoom(factor,cx=.5,cy=.5,defer=false){const w=Math.max(4,Math.min(2000,view.w*factor)),f=w/view.w;view.x+=view.w*cx*(1-f);view.y+=view.h*cy*(1-f);view.w=w;view.h*=f;if(defer)requestDraw();else draw(true);}
function search(){const q=$('search').value.trim().toLowerCase();$('search-results').replaceChildren();if(!q)return;const hits=Object.values(data.rooms).filter(r=>String(r.id)===q||`${title(r)} ${hints[r.id]?.features.map(f=>`${f.name} ${categories[f.key].name}`).join(' ')}`.toLowerCase().includes(q)).slice(0,20);for(const r of hits)$('search-results').append(button(`${title(r)} #${r.id}`,()=>{go(r.id,{fit:true});$('search').value='';search();}));if(!hits.length)$('search-results').textContent='No bundled rooms match. This demo is incomplete.';}
function showAbout(){
 $('dialog-title').textContent='Real layout, presentation-only experiment';$('dialog-body').replaceChildren();
 const p=document.createElement('p');p.textContent=`${Object.keys(data.rooms).length} rooms · pinned core ${data.provenance.engine_revision.slice(0,12)} and map snapshot. The core supplies base positions and focus units. The demo aligns Landing streets and catacombs to recorded classic-map coordinates, separates conflicting positions schematically, scales and relocates building groups away from streets, and draws collision-safe connections. Native exits and area ownership are unchanged. These display corrections are local, not an upstream engine patch.`;$('dialog-body').append(p);
 if(data.presentation)p.textContent=`${Object.keys(data.rooms).length} rooms · pinned core ${data.provenance.engine_revision.slice(0,12)}. Shared presentation rules with configurable native/artwork display. No Landing-specific coordinate adapter is applied. Native exits and area ownership are unchanged. This is not Nisugi's latest private layout export.`;
 const corrected=document.createElement('p');corrected.textContent='Catacombs: 201 recorded image anchors, two explicitly schematic insets, one reciprocal bearing-conflict pair. Original engine placement remains available for comparison. No layout rerun on focus changes. Source limitations below describe the unmodified base export.';$('dialog-body').append(corrected);
 if(data.presentation)corrected.textContent='Native mode preserves the pinned engine layout. Artwork mode uses each recorded image as an independent display frame, with explicit schematic detail for missing coordinates. Focus changes never rerun layout.';
 const reference=document.createElement('p');reference.textContent=`Another ${Object.values(data.scenes).filter(s=>s.reference_layout).length} previews use recorded artwork coordinates through the same generic adapter, including Helden Hall, Twilight Hall and House Arcane. Missing coordinates use labeled schematic insets; separate images are not interpreted as floors. This does not mean every layout is visually verified.`;$('dialog-body').append(reference);
 if(data.presentation)reference.textContent=`${Object.values(data.scenes).filter(s=>s.reference_layout).length} previews currently use the shared artwork adapter. Missing coordinates use labeled schematic insets; separate images are not interpreted as floors. These layouts still need your visual review.`;
 const services=document.createElement('p');services.textContent='Service icons use metadata and conservative title hints, not a verified service inventory. Each entrance retains its actual destination and command; a street marker does not rename or reassign its source room. Hide names retains icons and hover/focus descriptions. Crowded names reduce to icons; if even an icon cannot fit safely, room hover and recorded exit links remain available.';$('dialog-body').append(services);
 const ul=document.createElement('ul');for(const t of data.provenance.limitations){const li=document.createElement('li');li.textContent=t;ul.append(li);}$('dialog-body').append(ul);
 const a=document.createElement('a');a.href='data.json';a.textContent='Open data and source hashes';a.target='_blank';a.rel='noopener';$('dialog-body').append(a);$('dialog').showModal();
}
try{
 const loaded=await source.load({signal:cleanup.signal});if(destroyed)return;rawData=loaded.data;config=presentation(rawData);
 const layoutKey=`hydra.map.layout.v1.${nativeSetup?'setup':developerHunting?'editor':'explorer'}.${config.key||config.region||config.title}`;
 let savedLayout=null;try{savedLayout=storage?.getItem(layoutKey);}catch{/* Storage is optional. */}
 const selectedLayout=layoutMode(rawData,{developerHunting:developerHunting||!!nativeSetup,saved:savedLayout});
 data=preparePresentation(rawData,{layout:selectedLayout,classic:selectedLayout==='classic'});lookup=index(data);origin=config.start_room;
 root.querySelector('h1').textContent=config.title;
 if(data.presentation){
  root.querySelector('#region-browser summary').textContent=config.title+' region · areas & unassigned';
  $('region-select').setAttribute('aria-label',config.title+' regional group');$('region-filter').placeholder='Area, guild, shop…';
  root.querySelector('.places .note').textContent='Area ownership ≠ display focus. A building can belong to a town while its interior has its own focus.';
  $('reset-start').textContent='Reset start to '+title(data.rooms[config.start_room]);
  root.querySelectorAll('[data-demo]').forEach(b=>b.remove());
  for(const [i,tour] of config.tours.entries()){const b=button(`${i+1} · ${tour.name}`,()=>go(tour.room,{fit:true}));b.dataset.demo=tour.room;$('about').before(b);}
  $('layout-mode').replaceChildren(...[['native','Pinned engine positions'],['reference','Artwork-guided layout']].map(([value,label])=>{const o=document.createElement('option');o.value=value;o.textContent=label;return o;}));
 }
 $('layout-mode').value=selectedLayout;
 hints=annotations(data,lookup);buildSettings();buildRegionBrowser();$('region-filter').oninput=buildRegionBrowser;
 regional=regionalBrowser(root,{data:rawData,context:loaded.regionContext,warning:loaded.contextWarning,
  browse:id=>{go(id,{select:false});fitRooms(scene().sheet.rooms);},
  highlight:(ids,paint=true)=>{habitatRooms=ids;if(paint)draw();},
  fit:ids=>{if(ids.length)fitRooms(ids.map(id=>lookup[id]));},openExit:id=>go(id)});
 hunting=huntingView(root,{data:rawData,context:loaded.regionContext,corrections,redraw:()=>draw(),fit:ids=>fitRooms(ids.map(id=>lookup[id])),onSelect:developerHunting?id=>editor?.select(id):nativeSetup?id=>setup?.choose(hunting.catalogue.area(state.area).hunts.find(h=>h.id===id)):null,wasPan:()=>Date.now()-lastMove<=180});
 if(nativeSetup)setup=nativeHuntSetup(root,{data:rawData,connection:nativeSetup,fit:ids=>fitRooms(ids.map(id=>lookup[id]))});
 if(developerHunting)editor=huntingEditor(root,{data:rawData,context:loaded.regionContext,storage,files:huntingFiles,redraw:()=>draw(),fit:ids=>fitRooms(ids.map(id=>lookup[id])),camera:()=>({view,rooms:scene().sheet.rooms}),wasPan:()=>Date.now()-lastMove<=180,signal:cleanup.signal});
 overview=regionOverviewView(root,{data:rawData,context:loaded.regionContext,directory:regional,browse:id=>{go(id,{select:false});fitRooms(scene().sheet.rooms);}});
 const params=new URLSearchParams(initialHash.replace(/^#/,'')),requested=Number(params.get('room')||config.start_room);state=initial(data,lookup,lookup[requested]?requested:config.start_room);if(params.get('mode')==='explorer')state.mode='explorer';if(state.room!==origin&&!params.has('browse')&&!params.has('overview'))destination=state.room;recalculate();view=bounds(scene().sheet.rooms);aspect();render();
 if(params.has('overview')){overview.select(state.area);overview.open(params.get('overview')==='places'?'places':'map');}
 const imported=data.assignment_import;
 $('coverage').textContent=imported?`New area export · ${imported.region_rooms} drawable Landing-region rooms · ${imported.region_unassigned} still have no assigned area. Browse them under “Landing region”; region-only groups are provisional display frames. Closed rooms are visible, not routable.`:`Partial demo · ${Object.keys(data.rooms).length} bundled rooms.`;
 if(data.presentation)$('coverage').textContent=`${config.region||config.title} · ${imported.region_rooms} drawable regional rooms · ${imported.region_unassigned} area-unassigned · ${imported.nonregion_assigned_context.length} additional assigned-area context rooms. Automated native area assignments; not a human-approved boundary inventory. Unassigned previews are provisional; closed rooms are visible, not routable.`;
 $('route-metric').onchange=$('route-scripted').onchange=()=>{recalculate();render();};
 $('clear-route').onclick=clearRoute;
 $('route-status').title='Right-click to clear this route';$('route-status').oncontextmenu=e=>{if(destination!==null){e.preventDefault();clearRoute();}};
 $('layout-mode').onchange=()=>{
  const mode=$('layout-mode').value;
  savedLayout=mode;
  data=preparePresentation(rawData,data.presentation?{layout:mode}:{classic:mode==='classic'});lookup=index(data);hints=annotations(data,lookup);placeCache.clear();history=[];views={};view=bounds(scene().sheet.rooms);aspect();render();
  try{storage?.setItem(layoutKey,mode);}catch{/* Rendering does not depend on storage. */}
 };
 $('next-step').onclick=()=>{if(route.ids.length>1){origin=route.ids[1];recalculate();go(origin,{select:false});}};
 $('start-here').onclick=()=>{origin=state.room;recalculate();render();};
 $('reset-start').onclick=()=>{origin=config.start_room;recalculate();go(config.start_room,{select:false});};
 $('view-start').onclick=()=>go(origin,{select:false,fit:true});
 root.querySelectorAll('[data-demo]').forEach(b=>b.onclick=()=>go(Number(b.dataset.demo),{fit:Number(b.dataset.demo)!==config.start_room}));
 $('back').onclick=()=>{const previous=history.pop();if(previous){state=previous.state;view=previous.view;aspect();onNavigate(`room=${state.room}&mode=${state.mode}`);render();}};
 $('fit').onclick=()=>fitRooms(scene().sheet.rooms);$('fit-focus').onclick=()=>fitRooms(scene().sheet.rooms.filter(r=>r.unit===state.unit));
 $('zoom-in').onclick=()=>zoom(.75);$('zoom-out').onclick=()=>zoom(1/.75);
 for(const [id,mode] of [['focus-mode','focus'],['explore-mode','explorer']])$(id).onclick=()=>{state.mode=mode;render();};
 $('town-context').onclick=()=>{go(config.start_room,{select:false});fitRooms(scene().sheet.rooms);};$('search').oninput=search;$('about').onclick=showAbout;$('close-dialog').onclick=()=>$('dialog').close();$('unbundled-markers').onchange=draw;
 on($('canvas'),'wheel',e=>{e.preventDefault();const r=canvasBox||$('canvas').getBoundingClientRect();zoom(Math.exp(e.deltaY*.001),(e.clientX-r.left)/r.width,(e.clientY-r.top)/r.height,true);},{passive:false});
 on(window,'scroll',()=>{canvasBox=null;},{capture:true});on(window,'resize',()=>{canvasBox=null;});
 const hit=e=>{const rect=svgViewport($('map'),view);return rect?nearestRoom(scene().sheet.rooms,{x:e.clientX-rect.left,y:e.clientY-rect.top},view,rect):null;};
 // Picking a setup room takes precedence over that room's service/exit marker.
 // Otherwise e.g. TSC opens its destination menu instead of choosing town rest.
 on($('canvas'),'click',e=>{if(!setup||e.target.closest('button')||Date.now()-lastMove<=180)return;const r=hit(e);if(r&&setup.room(r.id)){e.preventDefault();e.stopImmediatePropagation();}},{capture:true});
 on($('canvas'),'click',e=>{if(e.target.closest('button')||Date.now()-lastMove<=180)return;const r=hit(e);if(r)go(r.id);});
 const rightClick=e=>{if(destination===null)return;const r=hit(e),label=e.target.closest('[data-transition-label],[data-transition-source],[data-route-destination]');if(r?.id===destination||Number(label?.dataset.transitionLabel||label?.dataset.transitionSource||label?.dataset.routeDestination)===destination)clearRoute();};
 // Right-drag is always camera movement, including when a boundary tool is active.
 // Some browsers emit contextmenu on press, before a right-drag can be known.
 // Defer mouse right-click actions to release; keyboard context menus still work.
 on($('canvas'),'contextmenu',e=>{e.preventDefault();e.stopImmediatePropagation();if(e.button!==2&&!drag)rightClick(e);},{capture:true});
 on($('canvas'),'pointerdown',e=>{if(![0,2].includes(e.button)||e.target.closest('button'))return;drag={x:e.clientX,y:e.clientY,view:{...view},moved:false,button:e.button,press:e};});
 on(window,'pointermove',e=>{if(!drag)return;const dx=e.clientX-drag.x,dy=e.clientY-drag.y;if(Math.abs(dx)+Math.abs(dy)>4){drag.moved=true;lastMove=Date.now();view.x=drag.view.x-dx*view.w/lastViewport.width;view.y=drag.view.y-dy*view.h/lastViewport.height;requestDraw();}});
 const endDrag=e=>{const finished=drag;drag=null;if(finished?.moved)requestDraw();else if(finished?.button===2&&e.type==='pointerup')rightClick(finished.press);};
 on(window,'pointerup',endDrag);on(window,'pointercancel',endDrag);on(window,'blur',endDrag);
 resizeObserver=new window.ResizeObserver(resizeCanvas);resizeObserver.observe($('canvas'));

 // Read-only diagnostic surface for regression tests, not a movement API.
 const snapshot=()=>({state:{...state},view:{...view},roomCount:scene().sheet.rooms.length,origin,destination,route:structuredClone(route),preferences:structuredClone(prefs),layout:structuredClone(scene().display_layout||scene().reference_layout||{kind:'native'}),regional:regional.snapshot(),hunting:hunting.snapshot(),editor:editor?.snapshot()});
 return {snapshot,inspect(id){if(!destroyed&&lookup[id]&&Number(id)!==state.room)go(Number(id));},destroy(){
  destroyed=true;setup?.destroy();cleanup.abort();resizeObserver?.disconnect();if(pendingFrame)cancelAnimationFrame(pendingFrame);clearTimeout(toastTimer);root.replaceChildren();
 }};
}catch(error){$('focus-name').textContent='Preview could not load';$('empty').hidden=false;$('empty').textContent=error.message;cleanup.abort();resizeObserver?.disconnect();throw error;}
}
