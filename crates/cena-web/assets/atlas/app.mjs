import {mountAtlas} from './atlas-view.mjs';
import {offlineSource} from './offline-source.mjs';

// Explorer-only host. No session tokens, WebSocket, or movement commands.
let storage=null;try{storage=localStorage;}catch{/* Persistence is optional. */}
const lifetime=new AbortController();
let atlas=null,search=null;
async function outside(id){
 try{
  search??=fetch('../corpus/search.json',{signal:lifetime.signal}).then(r=>{if(!r.ok)throw Error('World room index unavailable');return r.json();}).catch(e=>{search=null;throw e;});
  const room=(await search).find(r=>r.id===id);
  if(lifetime.signal.aborted)return;
  if(!room)throw Error(`Room #${id} is absent or excluded from this snapshot. No substitute destination was chosen.`);
  location.assign(`../${encodeURIComponent(room.region)}/#room=${id}&browse=1`);
 }catch(error){
  if(lifetime.signal.aborted)return;
  document.getElementById('dialog-title').textContent='Destination unavailable';
  document.getElementById('dialog-body').textContent=error.message;
  document.getElementById('dialog').showModal();
 }
}
try{
 atlas=await mountAtlas(document.body,{source:offlineSource(new URL('.',location.href)),storage,
  initialHash:location.hash,onNavigate:hash=>{history.replaceState(null,'',location.pathname+'#'+hash);},onOutside:outside});
 document.title=`Hydra · ${document.querySelector('h1').textContent} map explorer`;
 window.addEventListener('hashchange',()=>{
  const id=Number(new URLSearchParams(location.hash.slice(1)).get('room'));
  if(Number.isSafeInteger(id)&&id>0)atlas.inspect(id);
 },{signal:lifetime.signal});
 window.addEventListener('pagehide',event=>{if(!event.persisted){lifetime.abort();atlas.destroy();}},{signal:lifetime.signal});
}catch(error){console.error(error);}
