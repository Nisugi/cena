// Actual offline server + CSP. No credentials or game session.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {mkdir} from 'node:fs/promises';
import {resolve} from 'node:path';
const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
const server=spawn(process.env.ATLAS_PREVIEW_BIN||resolve('target/debug/examples/atlas_preview'),[],{stdio:['ignore','pipe','inherit']});
let browser;
try{
 const base=await new Promise((resolve,reject)=>{
  const timeout=setTimeout(()=>reject(Error('Preview timeout')),15000);
  server.once('error',reject);server.once('exit',code=>reject(Error('Preview exited '+code)));
  server.stdout.on('data',chunk=>{const match=String(chunk).match(/http:\/\/127\.0\.0\.1:\d+/);if(match){clearTimeout(timeout);resolve(match[0]);}});
 });
 browser=await chromium.launch({headless:true,executablePath:process.env.BROWSER_EXECUTABLE_PATH||undefined});
 const page=await browser.newPage({viewport:{width:1700,height:1100}}),errors=[],requests=[];
 page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>requests.push([r.method(),r.url()]));
 page.on('console',m=>{if(m.type()==='error'&&!m.text().startsWith('Failed to load resource:'))errors.push(m.text());});
 page.on('websocket',()=>errors.push('Unexpected game socket'));page.on('dialog',d=>d.accept());
 // Editing must reopen the readable artwork layout, not the compressed native
 // town skeleton. A deliberate comparison choice survives reload, separately
 // from the ordinary viewer. Layout switching never edits membership.
 const darkstone=base+'/atlas/landing/#room=6952&browse=1';
 await page.goto(darkstone);await page.locator('#room-name').filter({hasText:'#6952'}).waitFor({timeout:60000});
 assert.equal(await page.locator('#layout-mode').inputValue(),'native');
 await page.goto(darkstone+'&devhunt=1');await page.locator('#hunting-editor').waitFor({timeout:60000});
 assert.equal(await page.locator('#layout-mode').inputValue(),'reference','Developer editing defaults to artwork-guided geometry');
 assert.ok(await page.locator('#map [data-position-kind="artwork"]').count()>250);
 const darkstoneMembers=()=>page.locator('[data-hunt-edit-room]').evaluateAll(ns=>ns.map(n=>n.dataset.huntEditRoom).sort());
 const originalDarkstoneMembers=await darkstoneMembers();assert.equal(originalDarkstoneMembers.length,0,'No implicit first-group edit target');
 assert.ok(await page.locator('#hunt-edit-add').isDisabled());
 await page.locator('#layout-mode').selectOption('native');assert.deepEqual(await darkstoneMembers(),originalDarkstoneMembers);
 await page.reload();await page.locator('#hunting-editor').waitFor({timeout:60000});
 assert.equal(await page.locator('#layout-mode').inputValue(),'native');
 await page.locator('#layout-mode').selectOption('reference');assert.deepEqual(await darkstoneMembers(),originalDarkstoneMembers);
 await page.reload();await page.locator('#hunting-editor').waitFor({timeout:60000});
 assert.equal(await page.locator('#layout-mode').inputValue(),'reference');
 assert.ok(await page.locator('#map [data-position-kind="artwork"]').count()>250);
 // Reported room #7058 is now recovered from its native creature tag. Remove
 // then restore it: its room color AND safe connector must follow the draft.
 // The box-add regression below also covers rooms outside the source baseline.
 await page.locator('[data-hunting-section="dungeon-3:roa_ter"]').click();
 await page.locator('#hunt-edit-fit').click();await page.locator('#hunt-edit-remove').click();
 await page.locator('#canvas').scrollIntoViewIfNeeded();
 const addedRoom=await page.locator('#map [data-room="7058"]').boundingBox();
 await page.mouse.click(addedRoom.x+addedRoom.width/2,addedRoom.y+addedRoom.height/2);
 assert.equal(await page.locator('[data-hunting-group="dungeon-3:roa_ter"][data-hunting-room="7058"]').count(),0);
 const originalFill=await page.locator('#map [data-room="7058"]').getAttribute('fill');
 await page.locator('#hunt-edit-add').click();
 await page.locator('#canvas').scrollIntoViewIfNeeded();
 await page.mouse.click(addedRoom.x+addedRoom.width/2,addedRoom.y+addedRoom.height/2);
 assert.equal(await page.locator('[data-hunt-edit-room="7058"]').getAttribute('data-membership'),'selected');
 assert.equal(await page.locator('#map [data-room="7058"]').getAttribute('fill'),await page.locator('#map [data-room="7057"]').getAttribute('fill'),'Added room adopts selected hunting color');
 assert.equal(await page.locator('[data-hunting-group="dungeon-3:roa_ter"][data-hunting-room="7058"]').count(),1);
 assert.equal(await page.locator('[data-hunting-group="dungeon-3:roa_ter"][data-hunting-edge="7057:7058"]').count(),1);
 await page.locator('[data-hunting-section="ward"]').click();
 assert.equal(await page.locator('[data-hunting-group="dungeon-3:roa_ter"][data-hunting-room="7058"]').count(),1,'Other edited hunts remain visible');
 await page.locator('[data-hunting-section="dungeon-3:roa_ter"]').click();
 await page.locator('#hunt-edit-undo').click();
 assert.equal(await page.locator('#map [data-room="7058"]').getAttribute('fill'),originalFill);
 assert.equal(await page.locator('[data-hunting-group="dungeon-3:roa_ter"][data-hunting-room="7058"]').count(),0);
 await page.locator('#hunt-edit-undo').click();
 await page.goto(darkstone);await page.locator('#room-name').filter({hasText:'#6952'}).waitFor({timeout:60000});
 assert.equal(await page.locator('#layout-mode').inputValue(),'native','Developer layout choice cannot change the ordinary viewer');
 const url=base+'/atlas/landing/#room=6385&browse=1';
 const ready=()=>page.locator('#room-name').filter({hasText:'#6385'}).waitFor({timeout:60000});
 await page.goto(url);await ready();assert.equal(await page.locator('#hunting-editor').count(),0);
 await page.goto(url+'&devhunt=1');await ready();await page.locator('[data-hunting-section="smokey"]').click();
 const members=()=>page.locator('[data-hunt-edit-room]:not([data-membership=removed])').evaluateAll(nodes=>nodes.map(n=>Number(n.dataset.huntEditRoom)).sort((a,b)=>a-b));
 const initial=await members();assert.equal(initial.length,10);
 await page.locator('#search').fill('6385');await page.locator('#search-results button').filter({hasText:'#6385'}).first().click();
 const roomTitle=await page.locator('#room-name').textContent();let route=await page.locator('#route-status').textContent();
 async function position(id){await page.locator('#canvas').scrollIntoViewIfNeeded();return page.locator(`#map [data-room="${id}"]`).evaluate(n=>{const b=n.getBoundingClientRect();return {x:b.x+b.width/2,y:b.y+b.height/2};});}
 await page.locator('#hunt-edit-fit').click();await page.locator('#hunt-edit-remove').click();
 // Right-drag remains a camera gesture in every editing mode, never a selection.
 for(const mode of ['browse','add','remove']){
  await page.locator('#hunt-edit-'+mode).click();await page.locator('#canvas').scrollIntoViewIfNeeded();
  const before=await page.locator('#map').getAttribute('viewBox'),selected=await members();
  const viewport=await page.locator('#canvas').boundingBox();
  const start=await position(6385);
  await page.mouse.move(start.x,start.y);
  await page.mouse.down({button:'right'});await page.mouse.move(start.x+65,start.y+30,{steps:5});await page.mouse.up({button:'right'});
  await page.waitForTimeout(100);
  assert.notEqual(await page.locator('#map').getAttribute('viewBox'),before,'Right-drag must pan during '+mode);
  assert.deepEqual(await members(),selected,'Panning cannot edit membership');
  assert.equal(await page.locator('#route-status').textContent(),route,'Panning cannot clear the route');
  const afterRight=await page.locator('#map').getAttribute('viewBox');
  await page.mouse.move(viewport.x+40,viewport.y+40);await page.mouse.down();await page.mouse.move(viewport.x+70,viewport.y+65,{steps:5});await page.mouse.up();
  await page.waitForTimeout(100);
  assert.notEqual(await page.locator('#map').getAttribute('viewBox'),afterRight,'Unmodified left-drag must pan during '+mode);
  assert.deepEqual(await members(),selected);
 }
 await page.locator('#hunt-edit-fit').click();await page.locator('#hunt-edit-remove').click();
 const destination=await position(6385);await page.mouse.click(destination.x,destination.y,{button:'right'});
 route=await page.locator('#route-status').textContent();assert.ok(!route.includes(' → '),'Plain right-click still clears the destination');
 let p=await position(6385);await page.mouse.click(p.x,p.y);
 assert.equal((await members()).length,9);assert.equal(await page.locator('[data-hunt-edit-room="6385"]').getAttribute('data-membership'),'removed');
 assert.equal(await page.locator('[data-hunting-group="smokey"][data-hunting-room="6385"]').count(),0,'Removed room leaves the hunting wash immediately');
 assert.equal(await page.locator('#room-name').textContent(),roomTitle);assert.equal(await page.locator('#route-status').textContent(),route);
 await page.locator('#hunt-edit-undo').click();assert.deepEqual(await members(),initial);
 await page.locator('#hunt-edit-remove').click();
 const box=await page.locator('#canvas').boundingBox();
 await page.keyboard.down('Shift');await page.mouse.move(box.x+3,box.y+3);await page.mouse.down();await page.mouse.move(box.x+box.width-3,box.y+box.height-3,{steps:6});await page.mouse.up();await page.keyboard.up('Shift');
 assert.equal((await members()).length,0);await page.locator('#hunt-edit-undo').click();assert.deepEqual(await members(),initial);
 // Adding a box is a single undo transaction, including previously unmatched rooms.
 await page.locator('#hunt-edit-add').click();
 await page.keyboard.down('Shift');await page.mouse.move(box.x+3,box.y+3);await page.mouse.down();await page.mouse.move(box.x+box.width-3,box.y+box.height-3,{steps:5});await page.mouse.up();await page.keyboard.up('Shift');
 assert.ok((await members()).length>initial.length);await page.locator('#hunt-edit-undo').click();
 await page.locator('#hunt-edit-advanced > summary').click();
 await page.locator('#hunt-edit-reset').click();assert.deepEqual(await members(),initial);
 await page.locator('#hunt-edit-remove').click();p=await position(6385);await page.mouse.click(p.x,p.y);
 await page.locator('#hunt-edit-notes').fill('Browser regression correction');await page.locator('#hunt-edit-save').click();
 assert.match(await page.locator('#hunt-edit-message').textContent(),/Saved locally/);
 const saved=await page.evaluate(()=>localStorage.getItem('hydra.hunting.corrections.v1'));
 const record=JSON.parse(saved).records[0];assert.deepEqual(record.removed,[6385]);
 assert.deepEqual(record.added,[]);assert.equal(record.notes,'Browser regression correction');
 // Overlapping selections remain independent; return restores the first draft.
 await page.locator('[data-hunting-section="blackened"]').click();assert.equal((await members()).length,10);
 await page.locator('[data-hunting-section="smokey"]').click();assert.equal((await members()).length,9);
 assert.equal(await page.locator('[data-hunting-group="smokey"][data-hunting-room="6385"]').count(),0,'Saved visual correction reloads');
 // Drafts also survive browsing another area within the same mounted explorer.
 await page.locator('#hunt-edit-notes').fill('Unsaved area-switch draft');
 await page.locator('#search').fill('228');await page.locator('#search-results button').filter({hasText:'#228'}).first().click();
 await page.locator('#search').fill('6385');await page.locator('#search-results button').filter({hasText:'#6385'}).first().click();
 assert.equal(await page.locator('#hunt-edit-notes').inputValue(),'Unsaved area-switch draft');
 await page.locator('#hunt-edit-notes').fill(record.notes);await page.locator('#hunt-edit-fit').click();
 await page.locator('#hunt-edit-browse').click();p=await position(6386);await page.mouse.click(p.x,p.y);
 assert.match(page.url(),/devhunt=1/);
 await page.reload();await page.locator('#hunting-editor').waitFor({timeout:60000});
 await page.locator('[data-hunting-section="smokey"]').click();assert.equal((await members()).length,9);
 // Portable export carries only bounded correction data.
 await page.locator('#hunt-edit-advanced > summary').click();
 const downloadEvent=page.waitForEvent('download');await page.locator('#hunt-edit-export').click();const download=await downloadEvent;
 const stream=await download.createReadStream(),chunks=[];for await(const chunk of stream)chunks.push(chunk);
 const exported=JSON.parse(Buffer.concat(chunks));assert.deepEqual(exported.records,[record]);
 // Stale map hashes withhold edits, then explicit review + save rebinds them.
 await page.evaluate(()=>{const key='hydra.hunting.corrections.v1',d=JSON.parse(localStorage.getItem(key));d.records[0].mapSha256='b'.repeat(64);localStorage.setItem(key,JSON.stringify(d));});
 await page.reload();await page.locator('#hunting-editor').waitFor({timeout:60000});
 await page.locator('[data-hunting-section="smokey"]').click();
 assert.match(await page.locator('#hunt-edit-review-status').textContent(),/REVIEW REQUIRED/);assert.equal((await members()).length,10);
 assert.ok(await page.locator('#hunt-edit-save').isDisabled());await page.locator('#hunt-edit-review').click();assert.equal((await members()).length,9);
 await page.locator('#hunt-edit-save').click();
 // Import round trip and malformed input: no silent clobbering.
 await page.locator('#hunt-edit-import').setInputFiles({name:'corrections.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(exported))});
 await page.locator('#hunt-edit-message').filter({hasText:'Imported.'}).waitFor();
 await page.locator('#hunt-edit-import').setInputFiles({name:'broken.json',mimeType:'application/json',buffer:Buffer.from('{bad')});
 await page.waitForFunction(()=>!document.getElementById('hunt-edit-message').textContent.startsWith('Imported.'));
 assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('hydra.hunting.corrections.v1')).records[0].mapSha256),record.mapSha256);
 await page.locator('#hunt-edit-save').click();await page.locator('#hunt-route-tools > summary').click();await page.locator('#clear-route').click();
 await page.locator('#hunt-edit-fit').click();await page.locator('#hunting-editor').scrollIntoViewIfNeeded();
 await mkdir('target/atlas-evidence',{recursive:true});await page.screenshot({path:'target/atlas-evidence/hunting-editor.png',fullPage:true});
 assert.deepEqual(errors,[]);assert.ok(requests.every(([method,url])=>method==='GET'&&url.startsWith(base)));
 console.log('PASS developer hunting editor: default-off, add/remove/box/undo/reset, isolated routes, persistence, overlap independence, hash review, export/import, no game commands.');
}finally{await browser?.close();server.kill('SIGTERM');}
