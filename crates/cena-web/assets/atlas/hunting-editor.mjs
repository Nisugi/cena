import {huntingCatalogue} from './hunting-catalogue.mjs';
import {nearestRoom,svgViewport} from './interaction.mjs';
import {correctionDate} from './hunting-files.mjs';
import {correctionStore,correctionSchema,selectionKey,selectionDraft,parseCorrections,rectangleRooms,maxCorrectionBytes} from './hunting-corrections.mjs';

// Optional mount-local developer tool. No movement API or map database writes.
export function huntingEditor(root,{data,context,storage,files=null,redraw,fit,camera,wasPan,signal}){
 const doc=root.ownerDocument,win=doc.defaultView,canvas=root.querySelector('#canvas');
 const autosave=!!files?.save;
 // In file mode the disk snapshot is authoritative, independent of browser port.
 let memory=autosave?JSON.stringify(files.document):null;
 const store=correctionStore(autosave?{getItem:()=>memory,setItem:(_key,value)=>{memory=value;}}:storage),drafts=new Map(),models=new Map(),selectedByArea=new Map();
 let area=null,model=null,current=null,gesture=null,mode='browse',destroyed=false,saveTimer=null,saving=false,importing=false,previewCache=null,saveError='';
 const node=(tag,text,id)=>{const n=doc.createElement(tag);if(text)n.textContent=text;if(id)n.id=id;return n;};
 root.classList.add('hunting-dev');
 const panel=node('section',null,'hunting-editor');panel.setAttribute('aria-label','Hunting boundary editor');
 const heading=node('div');heading.className='hunt-edit-heading';
 const title=node('strong','Choose a hunting group','hunt-edit-title');
 const status=node('span',null,'hunt-edit-status');status.setAttribute('aria-live','polite');
 heading.append(title,status);panel.append(heading);
 const tools=node('div');tools.className='hunt-edit-tools';tools.setAttribute('role','group');tools.setAttribute('aria-label','Hunting boundary tools');panel.append(tools);
 const instruction=node('p',null,'hunt-edit-instruction');instruction.setAttribute('aria-live','polite');panel.append(instruction);
 const feedback=node('p',null,'hunt-edit-feedback');feedback.setAttribute('role','status');panel.append(feedback);
 canvas.before(panel);
 const advanced=node('details',null,'hunt-edit-advanced');advanced.append(node('summary','Review, notes & backups'));
 const reviewStatus=node('p',null,'hunt-edit-review-status');advanced.append(reviewStatus);
 const modeButtons=[];
 function button(label,id,action,parent=tools){const b=node('button',label,id);b.type='button';b.onclick=()=>run(action);parent.append(b);return b;}
 function run(fn){if(importing)return;try{fn();}catch(e){message.textContent=e.message;feedback.textContent=e.message;}previewCache=null;refresh();redraw();scheduleSave();}
 function scheduleSave(){if(!autosave||destroyed)return;win.clearTimeout(saveTimer);saveTimer=win.setTimeout(()=>flush(),450);}
 async function flush(explicit=false){
  if(saving||importing||destroyed)return;saving=true;win.clearTimeout(saveTimer);let succeeded=true;
  try{
   for(const draft of drafts.values()){
    if(!draft.dirty&&!(explicit&&draft===current)||draft.needsReview||!explicit&&draft.assessment.status==='review')continue;
    const record=draft.record();message.textContent='Saving correction file…';refresh();
    const [saved]=await files.save([record]);if(destroyed)return;
    store.save([record]);draft.saved(record,true);saveError='';message.textContent='Saved to '+saved.path;
   }
  }catch(error){succeeded=false;if(!destroyed){saveError=error.message;message.textContent='NOT SAVED: '+error.message;}}
  finally{saving=false;if(!destroyed){refresh();if(succeeded&&[...drafts.values()].some(d=>d.dirty&&!d.needsReview&&d.assessment.status!=='review'))scheduleSave();}}
 }
 for(const [key,label] of [['browse','Inspect / pan'],['add','+ Add rooms'],['remove','− Remove rooms']]){
  const b=button(label,'hunt-edit-'+key,()=>{mode=key;gesture=null;});modeButtons.push([key,b]);
 }
 const undo=button('Undo','hunt-edit-undo',()=>{current?.undo();feedback.textContent='Undid the last change to '+current.source.name+'.';});
 const reset=button('Reset to generated','hunt-edit-reset',()=>{if(win.confirm('Reset this draft to generated membership? Undo is available until you save.')){current?.reset();feedback.textContent='Reset '+current.source.name+' to generated rooms.';}},advanced);
 const fitButton=button('Fit group','hunt-edit-fit',()=>{if(current?.rooms.size)fit([...current.rooms]);});
 const review=button('Review surviving saved rooms','hunt-edit-review',()=>{
  if(win.confirm('Load surviving rooms from the old selection for review? Missing/closed rooms cannot be restored. Inspect the result, then Save to accept it against this snapshot.'))current?.review();
 },advanced);
 const notes=node('textarea',null,'hunt-edit-notes');notes.maxLength=4000;notes.rows=2;notes.setAttribute('aria-label','Hunting correction notes');notes.placeholder='Review notes (optional)';advanced.append(notes);
 const save=button('Save locally','hunt-edit-save',()=>{
  if(!current)return;if(autosave){void flush(true);return;}store.save([current.record()]);current.saved();message.textContent='Saved locally. Export JSON for a portable backup.';
 },autosave?advanced:tools);
 if(autosave)save.textContent='Save now / accept review';
 button('Export JSON','hunt-edit-export',()=>{
  const records=new Map(store.document.records.map(r=>[selectionKey(r.area,r.hunt),r]));
  for(const [key,draft] of drafts)if(draft.dirty)records.set(key,draft.record());
  const output=parseCorrections(JSON.stringify({schema:correctionSchema,records:[...records.values()]}));
  const url=win.URL.createObjectURL(new win.Blob([JSON.stringify(output,null,2)+'\n'],{type:'application/json'}));
  const link=node('a');link.href=url;link.download=`${area||'hydra'}-${correctionDate()}.hunting.json`;panel.append(link);link.click();link.remove();
  win.setTimeout(()=>win.URL.revokeObjectURL(url),1000);
  message.textContent='Exported saved records plus working drafts. Export does not mark drafts as locally saved.';
 },advanced);
 const imports=node('details');imports.append(node('summary','Import existing corrections (optional)'));advanced.append(imports);
 const file=node('input',null,'hunt-edit-import');file.type='file';file.accept='.json,application/json';file.setAttribute('aria-label','Import hunting corrections JSON');imports.append(file);
 const message=node('p',files?.error||store.error||(autosave?'Autosave enabled · '+files.directory:'Browser storage only · export for a portable file'), 'hunt-edit-message');message.setAttribute('role','status');advanced.append(message);
 advanced.append(node('p','Cyan ○ selected · green ○ added · red × removed.'));
 const help=node('details');help.append(node('summary','Editing & storage help'));
 help.append(node('p','Choose a group by its map label or group list. Add and Remove affect only that group. Clicking another group returns to Inspect without editing any rooms. Drag normally (left or right button) to pan. In Add or Remove, hold Shift and left-drag for a box. Escape returns to Inspect. Overlapping hunts are allowed; the selected hunt takes color priority. Habitat evidence and map connections never change.'));
 help.append(node('p',autosave?'Edits automatically create/update an area-and-date-named JSON file. No picker is needed. Older daily files are retained. Changed map sources still need explicit review.':'Browser-local working copy only. Export JSON to move to a different browser/localhost port. Existing saved entries without a matching hunt are retained for export.'));advanced.append(help);
 root.querySelector('#hunting-legend').after(advanced);
 // Editing is a bounded workspace, not a long document. Peripheral tools
 // remain available in the independently scrolling inspector; map + actions
 // never leave the screen when the hunting-group list scrolls.
 const side=root.querySelector('.connections'),inspector=root.querySelector('.inspector');
 root.querySelector('.map-tools').prepend(root.querySelector('#layout-controls'));
 function drawer(label,id,children,open=false){const d=node('details',null,id);d.open=open;d.append(node('summary',label),...children.filter(Boolean));return d;}
 const routeDrawer=drawer('Route preview (optional)','hunt-route-tools',[root.querySelector('#route-panel')]);
 const displayDrawer=drawer('Map display & focus','hunt-display-tools',[
  root.querySelector('#layout-note'),root.querySelector('#section-focus'),root.querySelector('.hunting-controls'),
  root.querySelector('#legend'),root.querySelector('.section-label'),root.querySelector('#units'),root.querySelector('.places .note')]);
 const roomDrawer=drawer('Room details','hunt-room-details',[inspector],true);
 side.prepend(advanced,roomDrawer,displayDrawer,routeDrawer,root.querySelector('#habitat-panel'));
 const about=root.querySelector('#about');if(about)root.querySelector('header .status').append(about);
 const sideButton=node('button','Details','hunt-inspector-toggle');sideButton.setAttribute('aria-expanded','false');
 sideButton.onclick=()=>{const open=root.classList.toggle('hunt-inspector-open');sideButton.setAttribute('aria-expanded',String(open));};
 const closeSide=node('button','Close details','hunt-inspector-close');
 closeSide.onclick=()=>{root.classList.remove('hunt-inspector-open');sideButton.setAttribute('aria-expanded','false');sideButton.focus();};
 side.prepend(closeSide);
 tools.append(sideButton);
 const dirty=()=>[...drafts.values()].filter(d=>d.dirty).length;
 function draftFor(hunt){
  const key=selectionKey(area,hunt.id);
  if(!drafts.has(key))drafts.set(key,selectionDraft(hunt.source,store.find(area,hunt.id)||hunt.draftRecord,
   {issues:hunt.boundaryIssues,rooms:hunt.reviewRooms}));
  return drafts.get(key);
 }
 function selectDraft(id=selectedByArea.get(area)){
  const hunt=model?.hunts.find(h=>h.id===id);current=null;
  if(hunt)current=draftFor(hunt);
  if(hunt)selectedByArea.set(area,hunt.id);
  previewCache=null;
  feedback.textContent='';
  notes.value=current?.notes||'';mode='browse';gesture=null;refresh();
 }
 function refresh(){
  const locked=!current||current.needsReview||importing;
  for(const [key,b] of modeButtons){b.setAttribute('aria-pressed',mode===key);b.disabled=key!=='browse'&&locked;}
  undo.disabled=locked||!current.canUndo;reset.disabled=locked;save.disabled=locked||!!store.error||!!files?.error||saving;fitButton.disabled=!current||!current.rooms.size;
  review.hidden=!current?.needsReview;notes.disabled=!current||importing;
  canvas.classList.toggle('hunt-editing',mode!=='browse'&&!locked);
  panel.dataset.mode=locked?'browse':mode;
  panel.dataset.hunt=current?.source.hunt||'';
  panel.style.setProperty('--hunt-color',model?.hunts.find(h=>h.id===current?.source.hunt)?.color||'#73dfef');
  title.textContent=current?'Editing '+current.source.name:'Choose a hunting group';
  const pending=dirty();
  const failure=saveError||files?.error||store.error;
  status.textContent=current?`${current.rooms.size} rooms · ${failure?'NOT SAVED: '+failure:saving?'Saving…':pending?`${pending} unsaved group(s)`:autosave?'Autosave on':current.assessment.status==='base'?'Generated baseline':'Saved in this browser'}`:'No edit target selected';
  instruction.textContent=!current?'Click a hunting group in the list or a name on the map. Then choose Add or Remove.':current.needsReview?'This saved group needs review before editing. Open “Review, notes & backups”.':mode==='browse'?'Inspecting '+current.source.name+'. Choose Add or Remove to edit. Drag to pan; scroll to zoom.':`${mode==='add'?'ADD to':'REMOVE from'} ${current.source.name}: click a room or Shift-drag a box. Drag normally to pan. Esc stops editing.`;
  reviewStatus.textContent=current?`${current.source.baseRooms.length} generated rooms. ${current.assessment.status==='base'?'Generated baseline (unreviewed).':'Saved local correction.'}`:'';
  if(current?.needsReview){advanced.open=true;reviewStatus.textContent+=' REVIEW REQUIRED: '+current.assessment.issues.join('; ')+'. Missing/unavailable: '+(current.assessment.missing.join(', ')||'none')+'. Saved correction is withheld; generated rooms shown.';}
  const unmatched=model?.unmatchedCorrections.length||0;
  if(unmatched)reviewStatus.textContent+=` ${unmatched} saved selection(s) have no current matching hunt; retained for export.`;
 }
 notes.oninput=()=>{if(current)current.notes=notes.value;refresh();scheduleSave();};
 file.onchange=async()=>{
  if(importing)return;importing=true;refresh();
  try{
   const selected=file.files[0];if(!selected)return;if(selected.size>maxCorrectionBytes)throw Error('Correction file is too large');
   const incoming=parseCorrections(await selected.text());if(destroyed)return;
   if(dirty())throw Error('Save or export your drafts before importing; unsaved work has not been replaced.');
   if(!win.confirm(`Import ${incoming.records.length} selection(s)? Matching saved entries will be replaced. Other saved entries are retained. Export first if you need the previous versions.`))return;
   if(files?.error)throw Error(files.error);
   if(saving)throw Error('Wait for the current autosave to finish before importing.');
   if(autosave)await files.save(incoming.records);
   if(destroyed)return;store.save(incoming.records);drafts.clear();models.clear();model=huntingCatalogue(data,context,store.document).area(area);models.set(area,model);selectDraft();message.textContent='Imported. Source mismatches are withheld pending review.';
  }catch(e){if(!destroyed)message.textContent=e.message;}finally{importing=false;file.value='';if(!destroyed){refresh();redraw();}}
 };
 const on=(target,event,fn,options={})=>target.addEventListener(event,fn,{...options,signal});
 const editing=()=>mode!=='browse'&&current&&!current.needsReview&&!importing;
 function editRooms(ids,operation=mode){
  const before=new Set(current.rooms);current.edit(ids,operation);
  const added=[...current.rooms].filter(id=>!before.has(id)),removed=[...before].filter(id=>!current.rooms.has(id));
  feedback.textContent=added.length?`Added ${added.length===1?'room #'+added[0]:added.length+' rooms'} to ${current.source.name}.`:removed.length?`Removed ${removed.length===1?'room #'+removed[0]:removed.length+' rooms'} from ${current.source.name}.`:`No change to ${current.source.name}.`;
 }
 const point=(e,box)=>({x:e.clientX-box.left,y:e.clientY-box.top});
 on(canvas,'pointerdown',e=>{
  if(!editing()||!e.shiftKey||e.button!==0||!e.isPrimary&&e.pointerType||e.target.closest('[data-hunting-label],button'))return;
  e.preventDefault();e.stopImmediatePropagation();
  const c=camera(),box=svgViewport(root.querySelector('#map'),c.view);if(!box)return;gesture={start:point(e,box),end:point(e,box),box,view:{...c.view},rooms:c.rooms,pointer:e.pointerId,draft:current,operation:mode};
 },{capture:true});
 on(win,'pointermove',e=>{if(!gesture||e.pointerId!==gesture.pointer)return;e.preventDefault();e.stopImmediatePropagation();gesture.end=point(e,gesture.box);paintBox();},{capture:true});
 on(win,'pointerup',e=>{
  if(!gesture||e.pointerId!==gesture.pointer)return;e.preventDefault();e.stopImmediatePropagation();
  const g=gesture;gesture=null;g.end=point(e,g.box);
  const dragged=Math.hypot(g.end.x-g.start.x,g.end.y-g.start.y)>4;
  const hit=nearestRoom(g.rooms,g.end,g.view,g.box);
  const ids=dragged?rectangleRooms(g.rooms,g.start,g.end,g.view,g.box):hit?[hit.id]:[];
  if(current===g.draft)run(()=>editRooms(ids,g.operation));
 },{capture:true});
 // Capture before route, place-label or transition click handlers.
 on(canvas,'click',e=>{
  if(e.target.closest('[data-hunting-label]'))return;
  if(editing()){e.preventDefault();e.stopImmediatePropagation();if(e.shiftKey||wasPan()||e.target.closest('#labels,button'))return;const c=camera(),box=svgViewport(root.querySelector('#map'),c.view);if(!box)return;const r=nearestRoom(c.rooms,point(e,box),c.view,box);if(r)run(()=>editRooms([r.id]));}
 },{capture:true});
 on(canvas,'dblclick',e=>{if(editing()){e.preventDefault();e.stopImmediatePropagation();}},{capture:true});
 on(canvas,'wheel',e=>{if(gesture){e.preventDefault();e.stopImmediatePropagation();}},{capture:true,passive:false});
 const cancel=()=>{gesture=null;paintBox();};
 on(win,'blur',cancel);on(win,'pointercancel',cancel);
 on(win,'keydown',e=>{if(e.key==='Escape'){gesture=null;mode='browse';refresh();redraw();}});
 on(win,'beforeunload',e=>{if(dirty()){e.preventDefault();e.returnValue='';}});
 signal.addEventListener('abort',()=>{destroyed=true;win.clearTimeout(saveTimer);root.classList.remove('hunting-dev');},{once:true});
 function paintBox(){
  root.querySelector('[data-hunt-edit-box]')?.remove();if(!gesture)return;
  const labels=root.querySelector('#labels'),m=labels.getScreenCTM();if(!m)return;
  const local=p=>({x:(p.x+gesture.box.left-m.e)/m.a,y:(p.y+gesture.box.top-m.f)/m.d});
  const a=local(gesture.start),b=local(gesture.end),n=doc.createElementNS('http://www.w3.org/2000/svg','rect');
  for(const [k,v] of Object.entries({'data-hunt-edit-box':'true',x:Math.min(a.x,b.x),y:Math.min(a.y,b.y),width:Math.abs(a.x-b.x),height:Math.abs(a.y-b.y),fill:'#73dfca','fill-opacity':.15,stroke:'#73dfca','stroke-dasharray':'4 3','pointer-events':'none'}))n.setAttribute(k,v);
  labels.append(n);
 }
 return {
  select(id){if(importing||!model?.hunts.some(h=>h.id===id))return;run(()=>selectDraft(id));},
  preview(){
   if(!model)return null;
   return previewCache??={area,selected:current?.source.hunt,memberships:new Map(model.hunts.map(h=>[h.id,[...draftFor(h).rooms]]))};
  },
  update(next){if(next===area)return;area=next;if(!models.has(area))models.set(area,huntingCatalogue(data,context,store.document).area(area));model=models.get(area);
   selectDraft();
  },
  paint({overlay,el,lookup,view,px}){
   if(!current)return;const selected=current.rooms,base=new Set(current.source.baseRooms),layer=el('g',{'data-hunt-edit-overlay':'true','pointer-events':'none'});
   for(const id of new Set([...selected,...base])){
    const room=lookup[id];if(!room||room.area!==area)continue;const x=(room.cell.x-view.x)/px,y=(room.cell.y-view.y)/px,removed=!selected.has(id),added=!base.has(id);
    const g=el('g',{'data-hunt-edit-room':id,'data-membership':removed?'removed':added?'added':'selected'});
    if(removed)g.append(el('path',{d:`M ${x-6} ${y-6} L ${x+6} ${y+6} M ${x-6} ${y+6} L ${x+6} ${y-6}`,stroke:'#ff879a','stroke-width':2}));
    else g.append(el('circle',{cx:x,cy:y,r:added?11:9,fill:'none',stroke:added?'#8fff8f':'#73dfef','stroke-width':2,...(added?{'stroke-dasharray':'3 2'}:{})}));
    layer.append(g);
   }
   overlay.append(layer);paintBox();
  },
  snapshot(){return {area,hunt:current?.source.hunt,rooms:current?[...current.rooms]:[],mode,dirty:dirty(),needsReview:!!current?.needsReview};}
 };
}
