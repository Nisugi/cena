import {title} from './model.mjs';

// Destination selection only: no native settings or travel authority live here.
export function restRoomIndex(data){
 const areas=new Map();
 for(const scene of Object.values(data.scenes||{}))for(const room of scene.sheet.rooms)areas.set(room.id,scene.label);
 return Object.values(data.rooms).filter(r=>!['closed','gone','virtual'].includes(r.status)&&areas.has(r.id)).map(r=>({
  id:r.id,name:title(r),area:areas.get(r.id),region:r.location||data.presentation?.region||'',
  aliases:r.id===228&&/town square central/i.test(title(r))?'TSC Wehnimers Landing':''
 }));
}
export function findRestRooms(rooms,query){
 const q=query.trim().toLowerCase();
 if(!q)return [];
 if(/^#?\d+$/.test(q))return rooms.filter(r=>r.id===Number(q.replace('#','')));
 const words=q.split(/\s+/);
 return rooms.filter(r=>words.every(w=>`${r.name} ${r.area} ${r.region} ${r.aliases}`.toLowerCase().includes(w))).slice(0,30);
}
export function restBookmarks(storage,identity,hash,validIds){
 const key='hydra.rest-spots.v1.'+JSON.stringify([identity.instance,identity.character.toLowerCase(),hash]);
 let state={favorites:[],recent:[]};
 try{const saved=JSON.parse(storage?.getItem(key));for(const k of ['favorites','recent'])state[k]=[...new Set((Array.isArray(saved?.[k])?saved[k]:[]).filter(id=>validIds.has(id)))].slice(0,12);}catch{}
 const persist=()=>{try{storage?.setItem(key,JSON.stringify(state));return !!storage;}catch{return false;}};
 return {list:kind=>[...state[kind]],remember(id){if(!validIds.has(id))return false;state.recent=[id,...state.recent.filter(r=>r!==id)].slice(0,12);return persist();},
  toggle(id){if(!validIds.has(id))return false;state.favorites=state.favorites.includes(id)?state.favorites.filter(r=>r!==id):[id,...state.favorites].slice(0,12);return persist();}};
}

export function restPicker(root,{data,storage,identity,origin,onSelect,onMap,previewRoute}){
 const doc=root.ownerDocument||root,rooms=restRoomIndex(data),byId=new Map(rooms.map(r=>[r.id,r]));
 const make=(tag,text,id)=>{const n=doc.createElement(tag);if(text)n.textContent=text;if(id)n.id=id;return n;};
 const dialog=make('dialog',null,'native-rest-dialog');dialog.className='rest-picker';dialog.setAttribute('aria-labelledby','native-rest-heading');root.append(dialog);
 const heading=make('h2',null,'native-rest-heading'),close=make('button','Cancel','native-rest-close');close.onclick=()=>dialog.close();
 const help=make('p','Choose a location without leaving your hunt. Suggestions are not a claim of safety or the shortest route.');help.className='hint';
 const searchLabel=make('label','Search this regional snapshot by place name or exact room number');
 const search=make('input',null,'native-rest-search');search.placeholder='Room number + Enter, or search a name';search.type='search';searchLabel.append(search);
 const shortcut=make('p','Know the room number? Type it and press Enter. No search-result click needed.');shortcut.className='hint';
 const lists=make('div',null,'native-rest-results'),detail=make('section',null,'native-rest-detail'),message=make('p',null,'native-rest-message');message.setAttribute('role','status');
 const map=make('button','Pick on map…','native-rest-map');
 const note=make('p','Favorites and recent choices are local to this browser and character, not imported from native profiles. Only rooms in this snapshot can be selected.');note.className='hint';
 dialog.append(close,heading,help,searchLabel,shortcut,lists,detail,message,map,note);
 let kind='town',selected=null,bookmarks=null;
 function describe(id){const r=byId.get(id);return r?`${r.name} · ${r.area} · #${id}`:`Room #${id}`;}
 function show(id){
  selected=id;detail.replaceChildren();message.textContent='';const r=byId.get(id);if(!r)return;
  detail.append(make('h3',r.name),make('p',`${r.area}${r.region?' · '+r.region:''} · Room #${id}`));
  const use=make('button',`Use #${id} for ${kind} rest`,'native-rest-use');use.onclick=()=>{bookmarks?.remember(id);dialog.close();onSelect(kind,id);};
  const favorite=make('button',bookmarks?.list('favorites').includes(id)?'Remove favorite':'Save favorite','native-rest-favorite');favorite.disabled=!bookmarks;
  favorite.onclick=()=>{const saved=bookmarks.toggle(id);render();show(id);if(!saved)message.textContent='Browser storage unavailable; this favorite lasts only for this page.';};
  const preview=make('button','Preview route','native-rest-route');preview.onclick=()=>{
   const from=origin();if(!from){message.textContent='Pick a hunt starting room first so the route has a known origin.';return;}
   const route=previewRoute(from,id);
   message.textContent=route.status==='found'?`Preview from #${from}: ${route.edges.length} exits to #${id}. Eligible recorded links only; no movement sent. The visible map shows its part of the route.`:'No eligible route in this snapshot. This does not prove the destination is inaccessible. No movement sent.';
  };
  detail.append(use,favorite,preview);
 }
 function render(){
  lists.replaceChildren();
  const group=(name,ids)=>{if(!ids.length)return;lists.append(make('h3',name));for(const id of ids){const b=make('button',describe(id));b.dataset.restRoom=id;b.onclick=()=>show(id);lists.append(b);}};
  if(search.value.trim()){
   const hits=findRestRooms(rooms,search.value);group('Search results',hits.map(r=>r.id));
   if(!hits.length)lists.append(make('p','No selectable room found in this snapshot. Check the name or room number; closed rooms are excluded.'));
   if(hits.length===30)lists.append(make('p','Showing up to 30 matches. Refine the search to narrow them.'));
  }else{
   if(kind==='town'&&byId.get(228)?.aliases)group('Suggested · Wehnimer’s Landing', [228]);
   if(kind==='field'&&byId.has(origin()))group('Near your hunt · starting room (check suitability)',[origin()]);
   group('Favorites',bookmarks?.list('favorites')||[]);group('Recent choices',bookmarks?.list('recent')||[]);
  }
 }
 search.oninput=()=>{selected=null;detail.replaceChildren();message.textContent='';render();
  if(/^#?\d+$/.test(search.value.trim())){const hits=findRestRooms(rooms,search.value);if(hits.length===1){lists.replaceChildren();show(hits[0].id);}}
 };
 search.onkeydown=event=>{if(event.key!=='Enter')return;event.preventDefault();
  const hits=findRestRooms(rooms,search.value);
  if(hits.length===1){const id=hits[0].id;bookmarks?.remember(id);dialog.close();onSelect(kind,id);}
  else message.textContent=hits.length?'Several rooms match. Choose a result or enter its exact room number.':'Enter a known, open room number or choose a matching location.';
 };
 map.onclick=()=>{const id=selected||(kind==='town'&&byId.get(228)?.aliases?228:origin());dialog.close();onMap(kind,id);};
 return {open(key,current){kind=key;selected=null;heading.textContent=`Choose ${kind} rest`;
   const who=identity();bookmarks=who?restBookmarks(storage,who,data.provenance.hashes.map,new Set(byId.keys())):null;
   search.value='';detail.replaceChildren();message.textContent='';render();if(byId.has(current))show(current);dialog.showModal();search.focus();
  },describe,remember(id){bookmarks?.remember(id);},destroy(){dialog.remove();}};
}
