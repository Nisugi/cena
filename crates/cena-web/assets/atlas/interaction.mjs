// Pick the nearest room within a screen-sized target, not the last SVG circle
// painted. This avoids overlapping invisible hit areas stealing nearby clicks.
export function nearestRoom(rooms,point,view,viewport,radius=14){
 const scale=viewport.width/view.w;
 return rooms.map(room=>({room,x:(room.cell.x-view.x)*scale,y:(room.cell.y-view.y)*scale}))
  .filter(p=>p.x>=0&&p.y>=0&&p.x<=viewport.width&&p.y<=viewport.height)
  .map(p=>({...p,d:Math.hypot(point.x-p.x,point.y-p.y)}))
  .filter(p=>p.d<=radius).sort((a,b)=>a.d-b.d||a.room.id-b.room.id)[0]?.room||null;
}
