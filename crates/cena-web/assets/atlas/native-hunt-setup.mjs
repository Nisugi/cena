// Map selection -> native TOML configuration. No WebSocket or movement API.
import {restPicker} from './rest-picker.mjs';
export function nativeHuntSetup(root,{data,connection,fit,storage=null,mapPicker,previewRoute}){
 const doc=root.ownerDocument||root,panel=doc.createElement('section');panel.id='native-hunt-setup';
 root.classList.add('native-hunt-mode');root.querySelector('.connections').prepend(panel);
 const make=(tag,text,id)=>{const n=doc.createElement(tag);if(text)n.textContent=text;if(id)n.id=id;return n;};
 const heading=make('h2','Native hunting setup');panel.append(heading);
 const status=make('p','Connecting to the native configuration handler…','native-setup-status');status.setAttribute('role','status');panel.append(status);
 const chosen=make('p','Choose a hunting group on the map or left list.','native-setup-choice');panel.append(chosen);
 const name=make('input',null,'native-setup-name');name.placeholder='New profile name';
 const label=(text,input)=>{const l=make('label',text);l.append(input);panel.append(l);};label('Profile name',name);
 const picks={};let pick=null,selection=null,identity=null,busy=false,restoreMap=null,revision=0;
 const clearErrors=()=>{panel.querySelectorAll('.setup-field-error').forEach(n=>n.remove());panel.querySelectorAll('[aria-invalid]').forEach(n=>{n.removeAttribute('aria-invalid');n.removeAttribute('aria-describedby');});};
 const invalidate=()=>{revision++;clearErrors();saveStatus.textContent='Unsaved changes. Save setup checks and saves your configuration; it never starts hunting.';};
 const finishMap=()=>{pick=null;cancelPick.hidden=true;const restore=restoreMap;restoreMap=null;restore?.();};
 const selectRest=(key,id)=>{picks[key].value=id;picks[key].button.textContent=`${key==='town'?'Town':'Field'} rest: ${picker.describe(id)} — change`;invalidate();status.textContent='Rest location selected. No movement sent.';};
 const picker=restPicker(root,{data,storage,identity:()=>identity,origin:()=>picks.start.value,
  onSelect:selectRest,onMap:(key,id)=>{finishMap();restoreMap=mapPicker?.(id);pick=key;cancelPick.hidden=false;status.textContent=`Choose ${key} rest on the map. Your hunt draft is preserved; choosing a room or cancelling returns you to the hunt.`;},previewRoute});
 for(const [key,text] of [['start','Hunt start'],['town','Town rest'],['field','Field rest (optional)']]){
  const button=make('button',key==='start'?'Pick hunt start on map':`Choose ${text.toLowerCase()}…`,`native-pick-${key}`);
  button.type='button';button.onclick=()=>{finishMap();if(key!=='start'){picker.open(key,picks[key].value);return;}pick=key;cancelPick.hidden=false;status.textContent=`Click a room for ${text.toLowerCase()}. This never moves your character.`;};panel.append(button);
  picks[key]={value:null,button};
 }
 const cancelPick=make('button','Cancel picker · return to hunt','native-cancel-pick');cancelPick.hidden=true;cancelPick.onclick=()=>{finishMap();status.textContent='Room picker cancelled. Your draft is unchanged.';};panel.append(cancelPick);
 const clear=make('button','Disable field rest','native-clear-field');clear.onclick=()=>{finishMap();picks.field.value=null;picks.field.button.textContent='Choose field rest (optional)…';invalidate();};panel.append(clear);
 const targets=make('div',null,'native-setup-targets');panel.append(targets);
 const attacks=make('textarea',null,'native-setup-attacks');attacks.placeholder='Explicit native attack steps, one per line';label('Attack routine (required; nothing guessed)',attacks);
 const townCommands=make('textarea',null,'native-town-commands');label('Town rest commands (one per line)',townCommands);
 const fieldCommands=make('textarea',null,'native-field-commands');label('Field rest commands (no selling round)',fieldCommands);
 const hint=make('p','Field rest requires freshly observed healthy, unencumbered state; otherwise town. This candidate rests at full mind, any encumbrance, or mana below 20%; resumes at mind ≤80% and mana ≥90%. Enter native commands, one per line—not comma-separated Lich script names. Setup does not install or execute Lich scripts.');hint.className='hint';panel.append(hint);
 const ack=make('input',null,'native-setup-ack');ack.type='checkbox';label('I reviewed these rooms, creatures, commands and recovery thresholds.',ack);
 const saveBox=make('section',null,'native-setup-save-box');panel.append(saveBox);
 const save=make('button','Save setup','native-setup-save');saveBox.append(save);
 const saveStatus=make('p','Saves a new configuration only. Nothing starts.','native-save-status');saveStatus.setAttribute('role','status');saveBox.append(saveStatus);
 const advanced=make('details',null,'native-setup-advanced');advanced.append(make('summary','Advanced configuration'));panel.append(advanced);
 const previewButton=make('button','View generated configuration','native-setup-preview');advanced.append(previewButton);
 const reload=make('button','Read saved configuration','native-setup-reload');advanced.append(reload);
 const output=make('pre',null,'native-setup-toml');advanced.append(output);
 const lines=value=>value.split('\n').map(s=>s.trim()).filter(Boolean);
 const until={experience:80,mana:90};
 function config(operation){
  if(operation!=='load'&&(!selection||selection.boundaryStatus==='needs-review'))throw Error('Choose a current hunting boundary; stale corrections need editor review first.');
  return {operation,name:name.value,map_sha256:data.provenance.hashes.map,acknowledged:ack.checked,
   allowed:selection?.roomIds||[],start:picks.start.value||0,town:picks.town.value||0,
   town_commands:lines(townCommands.value),field:picks.field.value?{room:picks.field.value,commands:lines(fieldCommands.value),until}:null,
   targets:[...targets.querySelectorAll('input:checked')].map(n=>n.value),attacks:lines(attacks.value),until,preview:null};
 }
 function required(node,ok,message){if(ok)return;const error=make('p',message,node.id+'-error');error.className='setup-field-error';node.after(error);node.setAttribute('aria-invalid','true');node.setAttribute('aria-describedby',error.id);node.focus();throw Error(message);}
 function validate(){
  clearErrors();required(name,/^[a-z0-9_-]{1,100}$/.test(name.value),'Enter a profile name using lowercase letters, numbers, hyphens or underscores.');
  required(chosen,selection&&selection.boundaryStatus!=='needs-review','Choose a current hunting group; stale boundaries need editor review.');
  required(picks.start.button,selection.roomIds.includes(picks.start.value),'Choose a starting room inside the hunt.');
  required(picks.town.button,!!picks.town.value,'Choose a town rest location.');
  required(targets,!!targets.querySelector('input:checked'),'Select at least one creature to hunt.');
  required(attacks,lines(attacks.value).length>0,'Add at least one attack step before saving this hunting setup.');
  required(ack,ack.checked,'Confirm you reviewed the rooms, creatures, commands and recovery thresholds.');
 }
 async function call(message){
  const response=await fetch('/hunt/setup',{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer '+connection.token},body:JSON.stringify({session:connection.session,message})});
  if(!response.ok){const text=await response.text();let error;try{error=JSON.parse(text).error;}catch{}throw Error(error||text);}return response.json();
 }
 async function act(operation){if(busy)return;busy=true;previewButton.disabled=save.disabled=reload.disabled=true;save.textContent=operation==='save'?'Checking and saving…':'Save setup';
  const submittedRevision=revision;
  try{if(!identity)throw Error('Native setup is unavailable.');
   if(operation!=='load')validate();
   const request=config(operation==='save'?'preview':operation);
   let reply=await call({action:'configure',generation:identity.generation,config:request});
   if(operation!=='load'&&revision!==submittedRevision)throw Error('Setup changed while checking. Nothing saved; click Save setup again when ready.');
   if(operation==='save')reply=await call({action:'configure',generation:identity.generation,config:{...request,operation:'save',preview:reply.toml}});
   output.textContent=reply.toml;
   const message=operation==='preview'?'Generated configuration shown below. No file saved and nothing started.':reply.saved?`Saved ${reply.name}. ${revision===submittedRevision?'Nothing started.':'Newer edits are not saved. Nothing started.'}`:`Reloaded ${reply.name}. This readback does not replace your draft selections. Nothing started.`;
   status.textContent=saveStatus.textContent=message;
  }catch(error){status.textContent=saveStatus.textContent=error.message;}finally{busy=false;previewButton.disabled=save.disabled=reload.disabled=false;save.textContent='Save setup';}
 }
 previewButton.onclick=()=>act('preview');save.onclick=()=>act('save');reload.onclick=()=>act('load');
 panel.addEventListener('input',invalidate);
 call({action:'inspect'}).then(value=>{identity=value;
  if(value.map_sha256!==data.provenance.hashes.map){identity=null;throw Error('Native map and explorer snapshot differ. Load matching map bytes before setup.');}
  heading.textContent=`${value.offline?'OFFLINE FIXTURE · ':''}${value.character} · ${value.instance}`;
  status.textContent='Native configuration only. Choose a hunt; nothing here starts or moves a character.';
 }).catch(error=>{status.textContent=error.message;});
 return {choose(hunt){if(!hunt)return;finishMap();selection=hunt;invalidate();ack.checked=false;picks.start.value=null;
   picks.start.button.textContent='Pick hunt start on map';chosen.textContent=`${hunt.name} · ${hunt.roomIds.length} allowed rooms · ${hunt.boundaryStatus}`;
   name.value=('atlas-'+hunt.name+'-'+new Date().toISOString().replace(/\D/g,'')).toLowerCase().replace(/[^a-z0-9-]/g,'-');
   targets.replaceChildren();for(const creature of hunt.creatures){const l=make('label',creature.name),check=make('input');check.type='checkbox';check.value=creature.name;check.checked=true;l.prepend(check);targets.append(l);}
   fit(hunt.roomIds);
  },room(id){if(!pick)return false;
   if(pick==='start'&&!selection?.roomIds.includes(id)){status.textContent='Hunt start must be inside the chosen hunting rooms.';return true;}
   if(!data.rooms[id]||['closed','gone','virtual'].includes(data.rooms[id].status)){status.textContent='Only known, open rooms can be selected.';return true;}
   const key=pick;if(key==='start'){picks.start.value=id;picks.start.button.textContent=`Hunt start: #${id} — click to change`;invalidate();status.textContent='Hunt start selected. No movement sent.';}
   else{selectRest(key,id);picker.remember(id);}finishMap();return true;
  },destroy(){picker.destroy();panel.remove();root.classList.remove('native-hunt-mode');}};
}
