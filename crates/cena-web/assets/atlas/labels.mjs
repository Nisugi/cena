// Screen-space placement, independent of map geometry. Every room and marker
// is an obstacle, including collapsed context dots. No straight-right default.
export function overlaps(a,b,pad=2){return a.x<b.x+b.w+pad&&a.x+a.w+pad>b.x&&a.y<b.y+b.h+pad&&a.y+a.h+pad>b.y;}
export function placeLabel(anchor,size,obstacles,viewport){
  for(const gap of [18,30,46,66,90]){
    for(const [x,y] of [
      [anchor.x+gap,anchor.y-gap-size.h],
      [anchor.x-gap-size.w,anchor.y-gap-size.h],
      [anchor.x+gap,anchor.y+gap],
      [anchor.x-gap-size.w,anchor.y+gap],
      [anchor.x-size.w/2,anchor.y-gap-size.h],
      [anchor.x-size.w/2,anchor.y+gap]
    ]){
      const candidate={x,y,w:size.w,h:size.h};
      if(x<4||y<4||x+size.w>viewport.width-4||y+size.h>viewport.height-4)continue;
      if(obstacles.some(b=>overlaps(candidate,b)))continue;
      return candidate;
    }
  }
  return null;
}
