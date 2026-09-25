import {mountAtlas} from './atlas-view.mjs';
import {offlineSource} from './offline-source.mjs';
import {huntingFiles} from './hunting-files.mjs';
import {loadHuntingCorrections} from './hunting-catalogue.mjs';

// Explorer-only host. No session tokens, WebSocket, or movement commands.
let storage=null;try{storage=localStorage;}catch{/* Persistence is optional. */}
const lifetime=new AbortController();
// Explicit opt-in, not a game capability or an authorization boundary.
const developerHunting=new URLSearchParams(location.hash.slice(1)).get('devhunt')==='1';
const devSuffix=()=>(developerHunting?'&devhunt=1':'');
let atlas=null,search=null;
async function outside(id){
 try{
  search??=fetch('../corpus/search.json',{signal:lifetime.signal}).then(r=>{if(!r.ok)throw Error('World room index unavailable');return r.json();}).catch(e=>{search=null;throw e;});
  const room=(await search).find(r=>r.id===id);
  if(lifetime.signal.aborted)return;
  if(!room)throw Error(`Room #${id} is absent or excluded from this snapshot. No substitute destination was chosen.`);
  location.assign(`../${encodeURIComponent(room.region)}/#room=${id}&browse=1${devSuffix()}`);
 }catch(error){
  if(lifetime.signal.aborted)return;
  document.getElementById('dialog-title').textContent='Destination unavailable';
  document.getElementById('dialog-body').textContent=error.message;
  document.getElementById('dialog').showModal();
 }
}
try{
 let files=null;
 if(developerHunting){try{files=await huntingFiles({signal:lifetime.signal});}catch(error){files={error:error.message};}}
 const corrections=await loadHuntingCorrections({signal:lifetime.signal});
 atlas=await mountAtlas(document.body,{source:offlineSource(new URL('.',location.href)),storage,
  initialHash:location.hash,developerHunting,huntingFiles:files,corrections,onNavigate:hash=>{history.replaceState(null,'',location.pathname+'#'+hash+devSuffix());},onOutside:outside});
 document.title=`Hydra · ${document.querySelector('h1').textContent} map explorer`;
 window.addEventListener('hashchange',()=>{
  const params=new URLSearchParams(location.hash.slice(1));
  if((params.get('devhunt')==='1')!==developerHunting){location.reload();return;}
  const id=Number(params.get('room'));
  if(Number.isSafeInteger(id)&&id>0)atlas.inspect(id);
 },{signal:lifetime.signal});
 window.addEventListener('pagehide',event=>{if(!event.persisted){lifetime.abort();atlas.destroy();}},{signal:lifetime.signal});
}catch(error){console.error(error);const message=document.createElement('p');message.setAttribute('role','alert');message.textContent='Map unavailable: '+error.message;document.body.prepend(message);}
