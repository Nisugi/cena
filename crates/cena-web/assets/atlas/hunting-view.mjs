import {huntingSections} from './hunting-sections.mjs';
import {placeLabel} from './labels.mjs';

export function huntingView(root,{data,context,redraw,fit}){
 const doc=root.ownerDocument||root,$=id=>root.querySelector('#'+id),models=new Map();
 let model={area:null,sections:[],byRoom:new Map(),hunts:[],byHuntRoom:new Map()};
 const section=doc.createElement('details');section.id='hunting-legend';section.open=true;
 const heading=doc.createElement('summary');heading.textContent='Hunting highlights & place names';section.append(heading);
 const controls=doc.createElement('div');controls.className='hunting-controls';
 let enabled=true,labels=true,selected=null,active=false,previousLegend=true;const hidden=new Set();
 for(const [name,key] of [['Hunting colors','hunting-colors'],['Map labels','hunting-labels']]){
  const label=doc.createElement('label'),check=doc.createElement('input');check.type='checkbox';check.checked=true;check.id=key;
  check.onchange=()=>{if(key==='hunting-colors')enabled=check.checked;else labels=check.checked;redraw();};label.append(check,doc.createTextNode(name));controls.append(label);
 }
 section.append(controls);const note=doc.createElement('p');note.className='hint';note.textContent='Only habitat-matched rooms receive hunting colors. Plain labels identify other places. No match means unknown, not safe or creature-free. Levels are installed references, not live occupants.';section.append(note);
 const list=doc.createElement('div');list.className='hunting-list';section.append(list);
 const detail=doc.createElement('div');detail.id='hunting-detail';detail.setAttribute('aria-live','polite');section.append(detail);
 $('legend').before(section);
 function choose(s){selected=s.id;for(const b of list.querySelectorAll('[data-hunting-section]'))b.setAttribute('aria-pressed',b.dataset.huntingSection===s.id);detail.replaceChildren();
  const p=doc.createElement('p');p.textContent=`${s.name} · ${s.roomIds.length} habitat-matched rooms · ${s.place}. ${s.evidence}`;detail.append(p);
  const b=doc.createElement('button');b.textContent='Fit this place';b.onclick=()=>fit(s.roomIds);detail.append(b);redraw();
 }
 function buildList(){list.replaceChildren();detail.replaceChildren();
 for(const s of model.hunts){
  const row=doc.createElement('div');row.className='hunting-place';row.style.setProperty('--place-color',s.color);
  const check=doc.createElement('input');check.type='checkbox';check.checked=true;check.setAttribute('aria-label',`Show ${s.name} overlay`);check.onchange=()=>{if(check.checked)hidden.delete(s.id);else hidden.add(s.id);redraw();};
  const b=doc.createElement('button');b.dataset.huntingSection=s.id;b.setAttribute('aria-pressed','false');b.onclick=()=>choose(s);
  const name=doc.createElement('strong');name.textContent=s.name;const summary=doc.createElement('small');summary.textContent=(s.level?`Reference levels ${s.level}`:'Creature coverage unknown')+` · ${s.roomIds.length} matched rooms`;
  if(s.provisional)b.classList.add('provisional');b.append(name,summary);row.append(check,b);
  const creatures=doc.createElement('p');creatures.textContent=s.creatures.length?s.creatures.map(c=>`${c.name} ${c.level??'?'}${c.undead?' U':''}`).join(' · '):'No matched habitat records';row.append(creatures);list.append(row);
 }}
 const visible=()=>active?model.hunts.filter(s=>!hidden.has(s.id)):[];
 function roomInfo(id){const s=active?model.byRoom.get(id):null;return s?`${s.name}${s.provisional?' (provisional grouping)':''} · ${s.creatures.filter(c=>c.roomIds.includes(id)).map(c=>`${c.name} · Lv ${c.level??'?'}`).join('; ')||'Creature coverage unknown'}`:'';}
 return {update(area){
   if(area!==model.area){
    if(!models.has(area))models.set(area,huntingSections(data,context,area));
    model=models.get(area);selected=null;hidden.clear();buildList();
   }
   const next=model.sections.length>0;
   if(next&&!active){previousLegend=$('legend').open;$('legend').open=false;}
   if(!next&&active)$('legend').open=previousLegend;
   active=next;section.hidden=!active;root.querySelector('.places').classList.toggle('hunting-mode',active);
  },get active(){return active;},roomInfo,
  color(id){const s=active&&enabled?model.byHuntRoom.get(id):null;return s&&!hidden.has(s.id)?s.color:null;},
  paint({svg,overlay,el,scene,drawing,lookup,view,px,box,occupied,textWidth,hideLabels}){
   if(!active)return;
   if(svg&&enabled){
    const layer=el('g',{'pointer-events':'none','data-hunting-washes':'true'});
    for(const s of visible()){
     const ids=new Set(s.roomIds),alpha=selected===s.id ? .22 : .09;
     // Only existing safe edge paths; never hulls, bounding rectangles or
     // straight lines across separate islands or schematic frames.
     for(const e of drawing.sheet.edges)if(e.points&&ids.has(e.a_room)&&ids.has(e.b_room))layer.append(el('path',{d:e.points.map((p,i)=>`${i?'L':'M'} ${p.x} ${p.y}`).join(' '),fill:'none',stroke:s.color,'stroke-width':px*16,opacity:alpha,'stroke-linejoin':'round','stroke-linecap':'round'}));
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
   const placeNames=model.sections.filter(s=>s.labelEligible&&!names.has(s.name)).map(s=>({...s,color:'#9aabb7',level:null,creatures:[],placeOnly:true}));
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
    const g=el('g',{[s.placeOnly?'data-place-context-label':'data-hunting-label']:s.id,'data-label-box':`${placed.x},${placed.y},${placed.w},${placed.h}`,...(!s.placeOnly?{role:'button',tabindex:'0','aria-label':`${s.name}, ${s.level?'reference levels '+s.level:'habitat unknown'}; show place details`}:{})});
    overlay.append(el('line',{x1:anchor.x,y1:anchor.y,x2:Math.max(placed.x,Math.min(anchor.x,placed.x+placed.w)),y2:Math.max(placed.y,Math.min(anchor.y,placed.y+placed.h)),stroke:s.color,opacity:.45,'stroke-dasharray':'2 4','pointer-events':'none'}));
    g.append(el('rect',{x:placed.x,y:placed.y,width:placed.w,height:placed.h,rx:4,fill:'#101b27','fill-opacity':.94,stroke:s.color,'stroke-opacity':.5,...(s.provisional?{'stroke-dasharray':'3 3'}:{})}));
    g.append(el('text',{x:placed.x+9,y:placed.y+16,fill:s.color,'font-size':12,'font-family':'system-ui'},heading));
    if(line)g.append(el('text',{x:placed.x+9,y:placed.y+31,fill:'#b8c8d5','font-size':11,'font-family':'system-ui'},line));
    g.append(el('title',{},`${s.name}\n${s.creatures.map(c=>`${c.name} · Lv ${c.level??'?'} · ${c.roomIds.length} matched rooms`).join('\n')||'Habitat unknown'}\n${s.evidence}`));
    if(!s.placeOnly){g.style.pointerEvents='all';g.style.cursor='pointer';g.onclick=e=>{e.stopPropagation();choose(s);};g.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();choose(s);}};}overlay.append(g);
   }
  },snapshot(){return {area:model.area,active,enabled,labels,selected,sections:model.sections.map(s=>({id:s.id,name:s.name,rooms:s.roomIds,level:s.level,unknown:s.unknown,provisional:s.provisional})),hunts:model.hunts.map(h=>({id:h.id,name:h.name,rooms:h.roomIds,creatures:h.creatures.map(c=>c.id)}))};}
 };
}
