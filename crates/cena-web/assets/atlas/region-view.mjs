import {title,command} from './model.mjs';
import {regionModel} from './region-model.mjs';

// Optional browsing/reference UI. It never changes map membership or exits.
export function regionalBrowser(root,{data,context,warning='',browse,highlight,fit,openExit}){
 const doc=root.ownerDocument||root,$=id=>root.querySelector('#'+id),model=regionModel(data,context);
 const node=(tag,text,cls)=>{const n=doc.createElement(tag);if(text)n.textContent=text;if(cls)n.className=cls;return n;};
 const button=(text,fn)=>{const n=node('button',text);n.onclick=fn;return n;};
 const refs=node('details');refs.id='habitat-panel';refs.append(node('summary','Creature references'),node('div'));$('layout-controls').before(refs);
 const boundary=node('details');boundary.id='regional-boundary';boundary.append(node('summary'),node('div','', 'regional-exit-list'));$('all-access').before(boundary);
 let selected=null,current=null;
 function activate(id){selected=id;highlight(id?model.habitat(id,current):new Set());updateReferences();}
 function details(c){
  $('dialog-title').textContent=c.name+' · installed reference';const body=$('dialog-body');body.replaceChildren();
  body.append(node('p',[`Level ${c.level??'unknown'}`,c.family,c.type,c.undead?'Undead':null,c.max_hp?`HP ${c.max_hp}`:null,c.size,c.boss?'Boss':null].filter(Boolean).join(' · ')));
  body.append(node('p','Habitat UID matches and exact creature-name room tags are separate reference sources—not live occupants, guaranteed spawns, a safe route, or an approved hunting selection. A room can match both sources. Unknown rooms are not creature-free.'));
  for(const a of c.associations){const s=data.scenes[a.group],basis=a.basis==='room_tag_match'?'Exact room tags':a.basis==='room_uid_match'?'Habitat UIDs':'Unknown evidence';
   body.append(button(`${s.label} · ${basis} · ${a.roomIds.length} matched rooms`,()=>{$('dialog').close();browse(model.areas.find(x=>x.key===a.group).entryRoom);activate(c.id);fit(a.roomIds.map(Number));}),node('p',a.note,'hint'));
  }
  if(c.associations.some(a=>a.basis==='room_tag_match'))body.append(node('p',`Room-tag source: bundled native rooms · data SHA-256: ${context.data_sha256}`,'source-note'));
  if(c.url){try{const url=new URL(c.url);if(url.protocol==='https:'&&url.hostname==='gswiki.play.net'){const a=node('a','Open wiki reference (not freshly verified)');a.href=url.href;a.target='_blank';a.rel='noopener';body.append(a);}}catch{}}
  body.append(node('p',`Source: ${c.source.path}\nSHA-256: ${c.source.sha256}`,'source-note'));
  $('dialog').showModal();
 }
 function updateReferences(){
  const list=model.byArea.get(current)||[],target=refs.lastElementChild;target.replaceChildren();
  refs.firstElementChild.textContent=`Creature references · ${list.length} matched types${selected?' · highlighting '+(model.creatures.get(selected)?.name||selected):''}`;
  target.append(node('p',warning||(!context?'No creature reference sidecar for this region yet.':list.length?'Select a creature to outline its matched rooms in violet. Other rooms and all connections stay visible.':'No habitat UID or exact creature-name room-tag matches for this frame. This is unknown, not a creature-free area.'),'hint'));
  if(selected){target.append(button('Clear habitat highlight',()=>activate(null)),button('Fit matched rooms',()=>fit([...model.habitat(selected,current)])));}
  const chips=node('div',null,'creature-chips');
  for(const c of list){const wrap=node('span'),b=button(`${c.name} · Lv ${c.level??'?'}${c.undead?' · undead':''} · ${c.roomIds.length} rooms`,()=>activate(selected===c.id?null:c.id));b.dataset.creature=c.id;b.setAttribute('aria-pressed',selected===c.id);wrap.append(b,button('ⓘ',()=>details(c)));wrap.lastElementChild.setAttribute('aria-label',`Reference details for ${c.name}`);chips.append(wrap);}
  target.append(chips);
 }
 function mountDirectory(body,{controlsHost=body,close,select,onFilter=()=>{}}){
  const c=context?.coverage,assigned=model.areas.filter(a=>!a.provisional).length;
  body.append(node('p',`${Object.keys(data.rooms).length.toLocaleString()} bundled rooms · ${assigned} assigned/context frames · ${model.areas.length-assigned} provisional frames. Browse changes the viewed map, not your route destination.`));
  body.append(node('p',c?`${c.matchedCreatures} creature references match ${c.matchedRooms.toLocaleString()} rooms through habitat UIDs or exact room tags${c.tagOnlyRooms!==undefined?` (${c.tagOnlyRooms} rooms covered only by tags)`:''}. ${c.roomsWithoutAssociation.toLocaleString()} rooms have unknown habitat coverage. Levels are installed reference values. Geography, hunting selections and map plates remain independent.`:warning||'Creature reference data is not bundled for this region.','hint'));
  const controls=node('div',null,'region-controls'),q=node('input'),kind=node('select'),min=node('input'),max=node('input');
  q.type='search';q.placeholder='Area, place, creature or room number…';q.id='directory-query';q.setAttribute('aria-label','Search region');
  for(const [value,label] of [['assigned','Assigned / context areas'],['habitat','Any frame with creature references'],['provisional','Provisional · unassigned areas'],['all','All display frames']]){const o=node('option',label);o.value=value;kind.append(o);}kind.id='directory-kind';kind.setAttribute('aria-label','Area classification');
  for(const [input,name] of [[min,'Minimum reference level'],[max,'Maximum reference level']]){input.type='number';input.min='0';input.placeholder=name;input.setAttribute('aria-label',name);}
  controls.append(q,kind,min,max);controlsHost.append(controls);const count=node('p'),results=node('div',null,'region-results');results.id='directory-results';body.append(count,results);
  let selectedArea=null;
  const draw=()=>{
   const hits=model.search({query:q.value,kind:kind.value,min:min.value===''?null:Number(min.value),max:max.value===''?null:Number(max.value)});
   onFilter(hits);count.textContent=`${hits.length} places match${hits.length>100?' · showing first 100; refine the search':''}`;results.replaceChildren();
   for(const a of hits.slice(0,100)){
    const card=node('section',null,'region-card');card.dataset.regionArea=a.key;
    const choose=button(a.label,()=>select(a.key));choose.dataset.selectArea=a.key;choose.setAttribute('aria-pressed',a.key===selectedArea);
    const open=button('Open map',()=>{select(a.key);close();browse(a.entryRoom);});open.dataset.browseArea=a.key;open.setAttribute('aria-label','Open map: '+a.label);
    card.append(choose,open,node('p',`${a.roomIds.length} rooms · ${a.exits.length} outgoing boundary records · ${a.provisional?'PROVISIONAL: no assigned area':'Imported area / context frame'}`,'hint'));
    if(a.creatures.length){const names=node('div',null,'region-creatures');for(const c of a.creatures){const b=button(`${c.name} (${c.level??'?'})`,()=>{select(a.key);close();browse(a.entryRoom);activate(c.id);refs.open=true;});b.dataset.browseCreature=c.id;names.append(b);}card.append(names);}
    else card.append(node('p','Habitat coverage unknown','hint'));
    results.append(card);
   }
  };
  for(const input of [q,kind,min,max])input.oninput=draw;draw();
  return {select(key){selectedArea=key;results.querySelectorAll('[data-select-area]').forEach(b=>b.setAttribute('aria-pressed',b.dataset.selectArea===key));},query:()=>q.value,focus:()=>q.focus()};
 }
 return {model,mountDirectory,update(area){
  const changed=current!==area;current=area;if(changed)selected=null;
  highlight(selected?model.habitat(selected,current):new Set(),false);updateReferences();
  const exits=model.areas.find(a=>a.key===area)?.exits||[];
  boundary.firstElementChild.textContent=`Whole-map exits · ${exits.length} directed records`;
  boundary.lastElementChild.replaceChildren(...exits.map(e=>{
   const b=button(`↗ ${e.area?data.scenes[e.area].label:title(data.outside[e.to])} · ${command(e.edge)} · #${e.from} → #${e.to}${e.area?'':' · not bundled'}`,()=>openExit(e.to));b.dataset.boundaryFrom=e.from;b.dataset.boundaryTo=e.to;return b;
  }));
 },snapshot(){return {selected,matched:[...(selected?model.habitat(selected,current):[])],coverage:context?.coverage||null};}};
}
