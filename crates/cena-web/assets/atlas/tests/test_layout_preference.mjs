import test from 'node:test';
import assert from 'node:assert/strict';
import {layoutMode} from '../profile.mjs';

test('developer editor restores reference geometry without changing viewer defaults',()=>{
 const native={presentation:{layout_policy:'native'}};
 assert.equal(layoutMode(native),'native');
 assert.equal(layoutMode(native,{developerHunting:true}),'reference');
 assert.equal(layoutMode({presentation:{layout_policy:'reference'}}),'reference');
 assert.equal(layoutMode({}, {developerHunting:true}),'classic');
});
test('only compatible saved modes override the default',()=>{
 const data={presentation:{layout_policy:'native'}};
 for(const saved of ['native','reference'])assert.equal(layoutMode(data,{developerHunting:true,saved}),saved);
 for(const saved of [null,undefined,'','classic','original','garbage'])assert.equal(layoutMode(data,{developerHunting:true,saved}),'reference');
 assert.equal(layoutMode({}, {saved:'native'}),'native');
 assert.equal(layoutMode({}, {saved:'reference'}),'classic');
});
