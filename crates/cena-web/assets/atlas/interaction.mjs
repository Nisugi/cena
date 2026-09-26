// Pick the nearest room within a screen-sized target, not the last SVG circle
// painted. This avoids overlapping invisible hit areas stealing nearby clicks.
export function nearestRoom(rooms,point,view,viewport,radius=14){
 const scale=viewport.width/view.w,vertical=viewport.height/view.h;
 return rooms.map(room=>({room,x:(room.cell.x-view.x)*scale,y:(room.cell.y-view.y)*vertical}))
  .filter(p=>p.x>=0&&p.y>=0&&p.x<=viewport.width&&p.y<=viewport.height)
  .map(p=>({...p,d:Math.hypot(point.x-p.x,point.y-p.y)}))
  .filter(p=>p.d<=radius).sort((a,b)=>a.d-b.d||a.room.id-b.room.id)[0]?.room||null;
}

// SVG can letterbox inside its CSS box (including canvas borders). Hit tests
// must use the rendered transform, not assume the parent's width is its scale.
export function svgViewport(svg,view){
 const m=svg.getScreenCTM();
 if(!m||m.a<=0||m.d<=0)return null;
 return {left:m.e+view.x*m.a,top:m.f+view.y*m.d,width:view.w*m.a,height:view.h*m.d};
}
