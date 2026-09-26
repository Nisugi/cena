import {huntingCatalogue} from './hunting-catalogue.mjs';
import {placeLabel} from './labels.mjs';

export function huntingView(root,{data,context,redraw,fit,onSelect=null,corrections,wasPan=()=>false}){
 const catalogue=huntingCatalogue(data,context,corrections);
 const doc=root.ownerDocument||root,$=id=>root.querySelector('#'+id),models=new Map();
 let model={area:null,sections:[],byRoom:new Map(),hunts:[],byHuntRoom:new Map()};
 let editing=null,editedHunts=null,editedByRoom=new Map();
 const hunts=()=>editedHunts||model.hunts;
 const section=doc.createElement(onSelect?'section':'details');section.id='hunting-legend';if(!onSelect)section.open=true;
 const heading=doc.createElement(onSelect?'h3':'summary');heading.textContent=onSelect?'Hunting groups':'Hunting highlights & place names';section.append(heading);
 const controls=doc.createElement('div');controls.className='hunting-controls';
 let enabled=true,labels=true,selected=null,active=false,previousLegend=true;const hidden=new Set();
 for(const [name,key] of [['Hunting colors','hunting-colors'],['Map labels','hunting-labels']]){
  const label=doc.createElement('label'),check=doc.createElement('input');check.type='checkbox';check.checked=true;check.id=key;
  check.onchange=()=>{if(key==='hunting-colors')enabled=check.checked;else labels=check.checked;redraw();};label.append(check,doc.createTextNode(name));controls.append(label);
 }
 section.append(controls);const note=doc.createElement('p');note.className='hint';note.textContent='Hunting boundaries include valid local corrections. Generated boundaries remain unreviewed. Creature records stay separate—an added room is not a new spawn claim. Levels are references, not live occupants.';section.append(note);
 const list=doc.createElement('div');list.className='hunting-list';section.append(list);
 const warnings=doc.createElement('details');warnings.id='hunting-boundary-warnings';warnings.hidden=true;list.before(warnings);
 const detail=doc.createElement('div');detail.id='hunting-detail';detail.setAttribute('aria-live','polite');section.append(detail);
 $('legend').before(section);
 function choose(s){selected=s.id;for(const b of list.querySelectorAll('[data-hunting-section]'))b.setAttribute('aria-pressed',b.dataset.huntingSection===s.id);if(onSelect){onSelect(s.id);redraw();return;}detail.replaceChildren();
  const p=doc.createElement('p');p.textContent=`${s.name} · ${s.roomIds.length} boundary rooms · ${s.boundaryStatus||'editing preview'}. ${s.inheritedCorrections?'Includes saved member boundaries; the combined destination has not yet been reviewed. ':''}${s.evidence}`;detail.append(p);
  const b=doc.createElement('button');b.textContent='Fit this place';b.onclick=()=>fit(s.roomIds);detail.append(b);redraw();
 }
 function buildList(){const scroll=list.scrollTop,focused=list.contains(doc.activeElement)?doc.activeElement.dataset.huntingSection:null;list.replaceChildren();detail.replaceChildren();
 const unmatched=model.unmatchedCorrections||[],changed=model.hunts.filter(h=>h.boundaryStatus==='needs-review');
 warnings.replaceChildren();warnings.hidden=!unmatched.length&&!changed.length;
 if(!warnings.hidden){
  const summary=doc.createElement('summary');summary.textContent=`${unmatched.length+changed.length} saved boundaries need review`;warnings.append(summary);
  const note=doc.createElement('p');note.textContent='Saved files remain intact. These corrections are not silently reassigned or used as approved hunt settings.';warnings.append(note);
  for(const r of unmatched){const p=doc.createElement('p');p.textContent=`${r.name} (${r.hunt}): no matching generated group. Saved selection: ${r.baseRooms.length-r.removed.length+r.added.length} rooms.`;warnings.append(p);}
  for(const h of changed){const p=doc.createElement('p');p.textContent=`${h.name}: ${h.boundaryIssues.join('; ')}. Current generated rooms shown until reviewed in the boundary editor.`;warnings.append(p);}
 }
 for(const s of hunts()){
  const row=doc.createElement('div');row.className='hunting-place';row.style.setProperty('--place-color',s.color);
  const check=doc.createElement('input');check.type='checkbox';check.checked=!hidden.has(s.id);check.setAttribute('aria-label',`Show ${s.name} overlay`);check.onchange=()=>{if(check.checked)hidden.delete(s.id);else hidden.add(s.id);redraw();};
  const b=doc.createElement('button');b.dataset.huntingSection=s.id;b.setAttribute('aria-pressed',String(selected===s.id));b.onclick=()=>choose(s);
  const name=doc.createElement('strong');name.textContent=s.name;const summary=doc.createElement('small');summary.textContent=(s.level?`Reference levels ${s.level}`:'Creature coverage unknown')+` · ${s.roomIds.length} ${editing?'boundary-preview':s.boundaryStatus||'generated'} rooms`;
  if(s.provisional)b.classList.add('provisional');b.append(name,summary);row.append(check,b);
  const creatures=doc.createElement('p');creatures.textContent=s.creatures.length?s.creatures.map(c=>`${c.name} ${c.level??'?'}${c.undead?' U':''}`).join(' · '):'No matched habitat records';row.append(creatures);list.append(row);
 }
 list.scrollTop=scroll;
 if(focused)[...list.querySelectorAll('[data-hunting-section]')].find(b=>b.dataset.huntingSection===focused)?.focus({preventScroll:true});
 }
 const visible=()=>active?hunts().filter(s=>!hidden.has(s.id)):[];
 function roomInfo(id){const s=active?model.byRoom.get(id):null;return s?`${s.name}${s.provisional?' (provisional grouping)':''} · ${s.creatures.filter(c=>c.roomIds.includes(id)).map(c=>{
  const sources=[...new Set(c.associations.filter(a=>a.group===model.area&&a.roomIds.some(r=>Number(r)===id)).map(a=>a.basis==='room_tag_match'?'room tag':'habitat UID'))];
  return `${c.name} · Lv ${c.level??'?'} · ${sources.join(' + ')}`;
 }).join('; ')||'Creature coverage unknown'}`:'';}
 return {update(area){
   if(area!==model.area){
    if(!models.has(area))models.set(area,catalogue.area(area));
    model=models.get(area);editing=null;editedHunts=null;editedByRoom.clear();selected=null;hidden.clear();buildList();
   }
   const next=model.sections.length>0||!!model.unmatchedCorrections?.length;
   if(next&&!active){previousLegend=$('legend').open;$('legend').open=false;}
   if(!next&&active)$('legend').open=previousLegend;
   active=next;section.hidden=!active;root.querySelector('.places').classList.toggle('hunting-mode',active);
  },get active(){return active;},roomInfo,
  setEditing(preview){
   if(preview?.area!==model.area)preview=null;
   if(editing===preview)return;
   editing=preview;editedHunts=preview?model.hunts.map(s=>({...s,roomIds:preview.memberships.get(s.id)||s.roomIds})):null;
   editedByRoom=new Map();
   if(preview){
    selected=preview.selected;
    if(selected)hidden.delete(selected);
    for(const s of [...editedHunts].sort((a,b)=>Number(b.id===selected)-Number(a.id===selected)))for(const id of s.roomIds){
     if(!editedByRoom.has(id))editedByRoom.set(id,[]);editedByRoom.get(id).push(s);
    }
   }
   note.textContent=preview?'Choose here or click a group name on the map. The toolbar edits that same group. Boundaries can overlap; creature records stay unchanged.':'Hunting boundaries include valid local corrections. Generated boundaries remain unreviewed; changed corrections need review. Creature records stay separate—an added room is not a new spawn claim.';
   buildList();
  },
  color(id){if(!active||!enabled)return null;const s=editing?editedByRoom.get(id)?.find(s=>!hidden.has(s.id)):model.byHuntRoom.get(id);return s&&!hidden.has(s.id)?s.color:null;},
  paint({svg,overlay,el,scene,drawing,lookup,view,px,box,occupied,textWidth,hideLabels}){
   if(!active)return;
   if(svg&&enabled){
    const layer=el('g',{'pointer-events':'none','data-hunting-washes':'true'});
    for(const s of visible()){
     const ids=new Set(s.roomIds),alpha=selected===s.id ? .22 : editing ? .04 : .09;
     // Only existing safe edge paths; never hulls, bounding rectangles or
     // straight lines across separate islands or schematic frames.
     for(const e of drawing.sheet.edges)if(e.points&&ids.has(e.a_room)&&ids.has(e.b_room))layer.append(el('path',{d:e.points.map((p,i)=>`${i?'L':'M'} ${p.x} ${p.y}`).join(' '),fill:'none',stroke:s.color,'stroke-width':px*16,opacity:alpha,'stroke-linejoin':'round','stroke-linecap':'round','data-hunting-group':s.id,'data-hunting-edge':[e.a_room,e.b_room].sort((a,b)=>a-b).join(':')}));
     for(const id of ids){const p=lookup[id].cell;layer.append(el('circle',{cx:p.x,cy:p.y,r:px*9,fill:s.color,opacity:alpha,'data-hunting-room':id,'data-hunting-group':s.id}));}
    }
    svg.prepend(layer);
   }
   if(!labels||hideLabels)return;
   // A named service entrance can already identify this exact place. Keep
   // its actionable marker and our legend/color, without a second name badge.
   // Both name AND membership must agree; unrelated namesakes stay distinct.
   const namedPlaces=[...overlay.querySelectorAll('[data-place-name]')].filter(n=>n.dataset.placeName);
   const names=new Set(model.hunts.map(s=>s.name));
   const placeNames=model.sections.filter(s=>s.labelEligible&&!names.has(s.name)&&!model.sectionsHidden?.has(s.id)).map(s=>({...s,color:'#9aabb7',level:null,creatures:[],placeOnly:true}));
   for(const s of [...visible(),...placeNames].sort((a,b)=>Number(b.id===selected)-Number(a.id===selected))){
    if(namedPlaces.some(n=>n.dataset.placeName.toLowerCase()===s.name.toLowerCase()&&
     [Number(n.dataset.placeLabel),...n.dataset.placeTargets.split(',').filter(Boolean).map(Number)].some(id=>s.roomIds.includes(id))))continue;
    const rooms=s.roomIds.map(id=>lookup[id]);const points=rooms.map(r=>({x:(r.cell.x-view.x)/px,y:(r.cell.y-view.y)/px}));
    const onScreen=points.filter(p=>p.x>=0&&p.y>=0&&p.x<=box.width&&p.y<=box.height);if(!onScreen.length)continue;
    const center={x:onScreen.reduce((n,p)=>n+p.x,0)/onScreen.length,y:onScreen.reduce((n,p)=>n+p.y,0)/onScreen.length};
    const anchor=onScreen.reduce((a,b)=>Math.hypot(a.x-center.x,a.y-center.y)<=Math.hypot(b.x-center.x,b.y-center.y)?a:b);
    const width=Math.max(...points.map(p=>p.x))-Math.min(...points.map(p=>p.x)),height=Math.max(...points.map(p=>p.y))-Math.min(...points.map(p=>p.y));
    const heading=(s.mapName||s.name)+(s.level?' · Lv '+s.level:'');
    const subtitle=(width>180||height>180)?s.creatures.map(c=>`${c.name} (${c.level??'?'})`).join(' · '):'';
    const short=subtitle.length>62?subtitle.slice(0,60)+'…':subtitle;
    const headingWidth=textWidth(heading)*12/11;
    const w=Math.ceil(Math.max(headingWidth,short?textWidth(short):0))+18,h=short?40:25;
    let placed=placeLabel(anchor,{w,h},occupied,box),line=short;
    if(!placed&&line){line='';placed=placeLabel(anchor,{w:Math.ceil(headingWidth)+18,h:25},occupied,box);}
    if(!placed)continue;occupied.push(placed);
    const g=el('g',{[s.placeOnly?'data-place-context-label':'data-hunting-label']:s.id,'data-label-box':`${placed.x},${placed.y},${placed.w},${placed.h}`,...(!s.placeOnly?{role:'button',tabindex:'0','aria-pressed':String(s.id===selected),'aria-label':`${s.name}, ${s.level?'reference levels '+s.level:'habitat unknown'}; ${onSelect?'select hunting group':'show place details'}`}:{})});
    overlay.append(el('line',{x1:anchor.x,y1:anchor.y,x2:Math.max(placed.x,Math.min(anchor.x,placed.x+placed.w)),y2:Math.max(placed.y,Math.min(anchor.y,placed.y+placed.h)),stroke:s.color,opacity:.45,'stroke-dasharray':'2 4','pointer-events':'none'}));
    g.append(el('rect',{x:placed.x,y:placed.y,width:placed.w,height:placed.h,rx:4,fill:s.id===selected?'#233747':'#101b27','fill-opacity':.94,stroke:s.color,'stroke-width':s.id===selected?2:1,'stroke-opacity':s.id===selected?1:.5,...(s.provisional?{'stroke-dasharray':'3 3'}:{})}));
    g.append(el('text',{x:placed.x+9,y:placed.y+16,fill:s.color,'font-size':12,'font-family':'system-ui'},heading));
    if(line)g.append(el('text',{x:placed.x+9,y:placed.y+31,fill:'#b8c8d5','font-size':11,'font-family':'system-ui'},line));
    g.append(el('title',{},`${s.name}\n${s.creatures.map(c=>`${c.name} · Lv ${c.level??'?'} · ${c.roomIds.length} matched rooms`).join('\n')||'Habitat unknown'}\n${s.evidence}`));
    if(!s.placeOnly){g.style.pointerEvents='all';g.style.cursor='pointer';g.onclick=e=>{e.stopPropagation();if(!wasPan())choose(s);};g.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();choose(s);}};}overlay.append(g);
   }
  },catalogue,snapshot(){return {area:model.area,active,enabled,labels,selected,sections:model.sections.map(s=>({id:s.id,name:s.name,rooms:s.roomIds,level:s.level,unknown:s.unknown,provisional:s.provisional})),hunts:model.hunts.map(h=>({id:h.id,name:h.name,rooms:h.roomIds,status:h.boundaryStatus,creatures:h.creatures.map(c=>c.id)}))};}
 };
}
