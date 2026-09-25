// Map selection -> native TOML configuration. No WebSocket or movement API.
export function nativeHuntSetup(root,{data,connection,fit}){
 const doc=root.ownerDocument||root,panel=doc.createElement('section');panel.id='native-hunt-setup';
 root.classList.add('native-hunt-mode');root.querySelector('.connections').prepend(panel);
 const make=(tag,text,id)=>{const n=doc.createElement(tag);if(text)n.textContent=text;if(id)n.id=id;return n;};
 const heading=make('h2','Native hunting setup');panel.append(heading);
 const status=make('p','Connecting to the native configuration handler…','native-setup-status');status.setAttribute('role','status');panel.append(status);
 const chosen=make('p','Choose a hunting group on the map or left list.','native-setup-choice');panel.append(chosen);
 const name=make('input',null,'native-setup-name');name.placeholder='New profile name';
 const label=(text,input)=>{const l=make('label',text);l.append(input);panel.append(l);};label('Profile name',name);
 const picks={};let pick=null,selection=null,identity=null,preview=null,busy=false;
 const invalidate=()=>{preview=null;save.disabled=true;};
 for(const [key,text] of [['start','Hunt start'],['town','Town rest'],['field','Field rest (optional)']]){
  const button=make('button',`Pick ${text.toLowerCase()} on map`,`native-pick-${key}`);
  button.type='button';button.onclick=()=>{pick=key;status.textContent=`Click a room for ${text.toLowerCase()}. This never moves your character.`;};panel.append(button);
  picks[key]={value:null,button};
 }
 const cancelPick=make('button','Cancel room picker','native-cancel-pick');cancelPick.onclick=()=>{pick=null;status.textContent='Room picker cancelled. Browse normally.';};panel.append(cancelPick);
 const clear=make('button','Disable field rest','native-clear-field');clear.onclick=()=>{pick=null;picks.field.value=null;picks.field.button.textContent='Pick field rest (optional) on map';invalidate();};panel.append(clear);
 const targets=make('div',null,'native-setup-targets');panel.append(targets);
 const attacks=make('textarea',null,'native-setup-attacks');attacks.placeholder='Explicit native attack steps, one per line';label('Attack routine (required; nothing guessed)',attacks);
 const townCommands=make('textarea',null,'native-town-commands');label('Town rest commands (one per line)',townCommands);
 const fieldCommands=make('textarea',null,'native-field-commands');label('Field rest commands (no selling round)',fieldCommands);
 const hint=make('p','Field rest requires freshly observed healthy, unencumbered state; otherwise town. This candidate rests at full mind, any encumbrance, or mana below 20%; resumes at mind ≤80% and mana ≥90%. Healing/banking scripts are not installed by setup.');hint.className='hint';panel.append(hint);
 const ack=make('input',null,'native-setup-ack');ack.type='checkbox';label('I reviewed these rooms, creatures, commands and recovery thresholds.',ack);
 const previewButton=make('button','Preview native profile','native-setup-preview');panel.append(previewButton);
 const save=make('button','Save new profile — do not start','native-setup-save');save.disabled=true;panel.append(save);
 const reload=make('button','Reload named profile','native-setup-reload');panel.append(reload);
 const output=make('pre',null,'native-setup-toml');panel.append(output);
 const lines=value=>value.split('\n').map(s=>s.trim()).filter(Boolean);
 const until={experience:80,mana:90};
 function config(operation){
  if(operation!=='load'&&(!selection||selection.boundaryStatus==='needs-review'))throw Error('Choose a current hunting boundary; stale corrections need editor review first.');
  return {operation,name:name.value,map_sha256:data.provenance.hashes.map,acknowledged:ack.checked,
   allowed:selection?.roomIds||[],start:picks.start.value||0,town:picks.town.value||0,
   town_commands:lines(townCommands.value),field:picks.field.value?{room:picks.field.value,commands:lines(fieldCommands.value),until}:null,
   targets:[...targets.querySelectorAll('input:checked')].map(n=>n.value),attacks:lines(attacks.value),until,preview};
 }
 async function call(message){
  const response=await fetch('/hunt/setup',{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer '+connection.token},body:JSON.stringify({session:connection.session,message})});
  if(!response.ok)throw Error(await response.text());return response.json();
 }
 async function act(operation){if(busy)return;busy=true;previewButton.disabled=save.disabled=reload.disabled=true;
  try{if(!identity)throw Error('Native setup is unavailable.');
   const reply=await call({action:'configure',generation:identity.generation,config:config(operation)});
   output.textContent=reply.toml;
   if(operation==='preview'){preview=reply.toml;status.textContent='Review the effective TOML below. Save creates a new profile; it does not start hunting.';}
   else{preview=null;status.textContent=reply.saved?`Saved and verified ${reply.name}. Nothing started. Operator command for later: ;hunt ${reply.name}`:`Reloaded ${reply.name} through native inheritance. This readback does not replace your draft selections. Nothing started.`;}
  }catch(error){invalidate();status.textContent=error.message;}finally{busy=false;previewButton.disabled=reload.disabled=false;save.disabled=!preview;}
 }
 previewButton.onclick=()=>act('preview');save.onclick=()=>act('save');reload.onclick=()=>act('load');
 panel.addEventListener('input',invalidate);
 call({action:'inspect'}).then(value=>{identity=value;
  if(value.map_sha256!==data.provenance.hashes.map){identity=null;throw Error('Native map and explorer snapshot differ. Load matching map bytes before setup.');}
  heading.textContent=`${value.offline?'OFFLINE FIXTURE · ':''}${value.character} · ${value.instance}`;
  status.textContent='Native configuration only. Choose a hunt; nothing here starts or moves a character.';
 }).catch(error=>{status.textContent=error.message;});
 return {choose(hunt){selection=hunt;pick=null;invalidate();ack.checked=false;picks.start.value=null;
   picks.start.button.textContent='Pick hunt start on map';chosen.textContent=`${hunt.name} · ${hunt.roomIds.length} allowed rooms · ${hunt.boundaryStatus}`;
   name.value=('atlas-'+hunt.name+'-'+new Date().toISOString().replace(/\D/g,'')).toLowerCase().replace(/[^a-z0-9-]/g,'-');
   targets.replaceChildren();for(const creature of hunt.creatures){const l=make('label',creature.name),check=make('input');check.type='checkbox';check.value=creature.name;check.checked=true;l.prepend(check);targets.append(l);}
   fit(hunt.roomIds);
  },room(id){if(!pick)return false;
   if(pick==='start'&&!selection?.roomIds.includes(id)){status.textContent='Hunt start must be inside the chosen hunting rooms.';return true;}
   if(data.rooms[id]?.status==='closed'){status.textContent='Closed rooms cannot be selected.';return true;}
   picks[pick].value=id;picks[pick].button.textContent=`${pick}: #${id} — click to change`;pick=null;invalidate();status.textContent='Room selected. No movement sent.';return true;
  },destroy(){panel.remove();root.classList.remove('native-hunt-mode');}};
}
