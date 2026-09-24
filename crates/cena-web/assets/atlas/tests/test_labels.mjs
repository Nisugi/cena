import assert from 'node:assert/strict';
import {placeLabel,overlaps} from '../labels.mjs';
const viewport={width:600,height:400},anchor={x:200,y:200},size={w:140,h:21};
const obstacles=[{x:194,y:194,w:12,h:12},{x:218,y:173,w:145,h:28}];
const placed=placeLabel(anchor,size,obstacles,viewport);
assert.ok(placed);assert.ok(obstacles.every(b=>!overlaps(placed,b)));
assert.notEqual(placed.y,anchor.y-size.h/2);
assert.equal(placeLabel(anchor,size,[{x:0,y:0,w:600,h:400}],viewport),null);
for(let y=0;y<400;y+=25)for(let x=0;x<600;x+=25){
 const p=placeLabel({x,y},size,obstacles,viewport);
 if(p){assert.ok(p.x>=4&&p.y>=4&&p.x+p.w<=596&&p.y+p.h<=396);assert.ok(obstacles.every(b=>!overlaps(p,b)));}
}
console.log('PASS: labels avoid rooms, existing labels and viewport edges; crowded labels are omitted, never forced over rooms.');
