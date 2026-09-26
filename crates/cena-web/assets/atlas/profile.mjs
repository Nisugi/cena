// Data-only variation: all towns use the same renderer, graph and marker rules.
const landing={title:'Wehnimer’s Landing',region:"Wehnimer's Landing",start_room:228,
 home_label:'Landing',underground_label:'Catacombs',town_focus:'Town streets',
 primary_tabs:['town','catacombs','access'],tab_rooms:{town:228,catacombs:7501}};

const hasRoom=(scene,id)=>scene?.sheet?.rooms?.some(r=>r.id===id);
// Editing needs the readable reference geometry by default. This is a display
// preference only; maps with no artwork keep their native geometry.
export function layoutMode(data,{developerHunting=false,saved=null}={}){
 const allowed=data.presentation?['native','reference']:['native','classic'];
 if(allowed.includes(saved))return saved;
 if(!data.presentation)return 'classic';
 if(developerHunting)return 'reference';
 return allowed.includes(data.presentation.layout_policy)?data.presentation.layout_policy:'native';
}
export function presentation(data){
 if(!data.presentation){
  if(!data.rooms?.[landing.start_room]||!hasRoom(data.scenes?.town,landing.start_room))throw Error('Invalid Landing presentation start_room');
  return landing;
 }
 const p=data.presentation;
 const start=Number(p.start_room),town=data.scenes?.town;
 if(!Number.isSafeInteger(start)||!data.rooms?.[start]||!hasRoom(town,start))throw Error('Invalid regional presentation start_room');
 const underground=!!p.underground&&!!data.scenes?.catacombs?.sheet?.rooms?.length;
 const primaryTabs=(Array.isArray(p.primary_tabs)?p.primary_tabs:['town',...(underground?['catacombs']:[])])
  .filter((area,i,tabs)=>tabs.indexOf(area)===i&&!!data.scenes?.[area]&&(area!=='catacombs'||underground));
 if(!primaryTabs.includes('town'))primaryTabs.unshift('town');
 const homeLabel=p.primary_label||p.home_label||p.title||town.label;
 const undergroundLabel=underground?(p.underground_label||data.scenes.catacombs.label):null;
 return {...p,start_room:start,home_label:homeLabel,town_focus:p.town_focus||town.units?.[0]?.name||homeLabel,
  underground_label:undergroundLabel,primary_tabs:primaryTabs,
  tab_rooms:{town:start,...(underground?{catacombs:data.scenes.catacombs.sheet.rooms[0].id}:{})}};
}
