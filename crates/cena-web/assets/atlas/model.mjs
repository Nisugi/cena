// Presentation policy only. No topology inference, layout or travel commands.
import {presentation} from './profile.mjs';
export const title = r => (r?.title?.[0] || `Room #${r?.id ?? '?'}`).replace(/^\[|\]$/g, '');
export const command = e => e.cmd || (e.steps ? 'Scripted passage' : e.routine ? 'Routine' : 'Special / conditional exit');
export function index(data) {
  const result = {};
  for (const [area, scene] of Object.entries(data.scenes))
    for (const room of scene.sheet.rooms) result[room.id] = {...room, area};
  return result;
}
export function focusName(data, area, unit) {
  return unit === 0 ? (area === 'town' ? presentation(data).town_focus : area === 'catacombs' ? (data.presentation?presentation(data).underground_label:'Catacomb network') : data.scenes[area].label) : data.scenes[area].units[unit].name;
}
export function destinationName(data, lookup, id) {
  const room=lookup[id];
  if(!room)return title(data.outside[id]);
  if(room.area==='catacombs')return data.presentation?presentation(data).underground_label:'Landing Catacombs';
  if(room.area==='access')return title(data.rooms[id]);
  if(room.area!=='town')return data.scenes[room.area].label.replace(/ · unassigned group #\d+$/, '');
  const name=focusName(data,room.area,room.unit);
  if(room.unit===0||name==="Wehnimer's"||name==='Building')
    return presentation(data).home_label+' · '+title(data.rooms[id]).replace(/^Wehnimer's, /,'');
  return name;
}
export function transitions(data, lookup, area, unit) {
  const rows = [];
  for (const rid of data.scenes[area].units[unit].rooms) {
    for (const [ordinal, edge] of data.rooms[rid].exits.entries()) {
      const dest = lookup[edge.to];
      if (dest?.area === area && dest.unit === unit) continue;
      rows.push({id: `${rid}:${ordinal}`, from: rid, to: edge.to, edge, bundled: !!dest,
        destination: destinationName(data,lookup,edge.to),
        targetArea: dest?.area ?? null, targetUnit: dest?.unit ?? null,
        crossArea: !!dest && dest.area !== area,
        role: transitionRole(data,lookup,rid,edge.to)});
    }
  }
  return rows.sort((a,b) => Number(b.crossArea)-Number(a.crossArea) || Number(b.bundled)-Number(a.bundled) || a.from-b.from || a.to-b.to);
}
// Drawing frames are not geographic boundaries. In particular, a connected
// region-only fallback is neither an approved area nor proof of a building.
export function transitionRole(data,lookup,from,to){
 const a=lookup[from],b=lookup[to];
 if(!b)return 'outside';
 if(a.area===b.area)return 'doorway';
 if(a.area==='catacombs'||b.area==='catacombs')return 'area';
 const source=data.rooms[from]?.assignment,target=data.rooms[to]?.assignment;
 if(source?.region&&target?.region&&source.region!==target.region)return 'area';
 if(source?.area&&target?.area)return source.area===target.area?'doorway':'area';
 return 'unassigned';
}
export function initial(data, lookup, id=presentation(data).start_room) {
  const room = lookup[id];
  if (!room) throw Error('Preview room is not bundled');
  return {room: id, area: room.area, unit: room.unit, mode: 'focus'};
}
export function visit(state, lookup, id) {
  const room = lookup[id];
  if (!room) return null;
  return {...state, room: Number(id), area: room.area, unit: room.unit};
}
export function bounds(rooms) {
  if (!rooms.length) throw Error('Cannot fit an empty focus');
  const xs=rooms.map(r=>r.cell.x), ys=rooms.map(r=>r.cell.y);
  const left=Math.min(...xs), top=Math.min(...ys), right=Math.max(...xs), bottom=Math.max(...ys);
  return {x:left-5,y:top-5,w:Math.max(right-left+10,18),h:Math.max(bottom-top+10,14)};
}
