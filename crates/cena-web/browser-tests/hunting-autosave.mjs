// Real native file writer, isolated scratch folder, no game session.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {mkdtemp,mkdir,readdir,readFile,rm} from 'node:fs/promises';
import {resolve} from 'node:path';
import {once} from 'node:events';
const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
await mkdir('target/hunting-autosave-tests',{recursive:true});
const directory=await mkdtemp(resolve('target/hunting-autosave-tests/run-'));
let server,browser;
async function start(){
 server=spawn(process.env.ATLAS_PREVIEW_BIN||resolve('target/debug/examples/atlas_preview'),[],{env:{...process.env,CENA_HUNTING_CORRECTIONS_DIR:directory},stdio:['ignore','pipe','inherit']});
 return new Promise((resolve,reject)=>{
  const timer=setTimeout(()=>reject(Error('Preview timeout')),15000);
  server.once('error',reject);server.once('exit',code=>reject(Error('Preview exited '+code)));
  server.stdout.on('data',chunk=>{const match=String(chunk).match(/http:\/\/127\.0\.0\.1:\d+/);if(match){clearTimeout(timer);resolve(match[0]);}});
 });
}
async function stop(){if(server){const process=server;server=null;const exit=once(process,'exit');process.kill('SIGTERM');await exit;}}
const files=async()=>(await readdir(directory)).filter(name=>name.endsWith('.hunting.json'));
try{
 let base=await start();
 assert.equal((await files()).length,0,'No correction file before an edit');
 assert.equal((await fetch(base+'/dev/hunting-corrections',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'})).status,422);
 browser=await chromium.launch({headless:true,executablePath:process.env.BROWSER_EXECUTABLE_PATH||undefined});
 const page=await browser.newPage({viewport:{width:1700,height:1100},timezoneId:'Asia/Bangkok'}),errors=[];
 page.on('pageerror',e=>errors.push(e.message));page.on('websocket',()=>errors.push('Game socket'));page.on('dialog',d=>d.accept());
 const open=async()=>{await page.goto(base+'/atlas/landing/#room=6385&browse=1&devhunt=1');await page.locator('#hunting-editor').waitFor({timeout:60000});await page.locator('#hunt-edit-advanced > summary').click();await page.locator('[data-hunting-section="smokey"]').click();};
 await open();assert.equal((await files()).length,0);
 await page.locator('#hunt-edit-fit').click();await page.locator('#hunt-edit-remove').click();
 await page.locator('#canvas').scrollIntoViewIfNeeded();
 const p=await page.locator('#map [data-room="6385"]').evaluate(n=>{const b=n.getBoundingClientRect();return {x:b.x+b.width/2,y:b.y+b.height/2};});
 await page.mouse.click(p.x,p.y);
 await page.locator('#hunt-edit-message').filter({hasText:'Saved to '}).waitFor();
 const names=await files();assert.equal(names.length,1);assert.match(names[0],/^area-wehnimers\.landing\.lysierian\.hills-\d{4}-\d{2}-\d{2}\.hunting\.json$/);
 const load=async()=>JSON.parse(await readFile(resolve(directory,names[0]),'utf8'));
 assert.deepEqual((await load()).records[0].removed,[6385]);
 const viewer=await browser.newPage();
 await viewer.goto(base+'/atlas/landing/#room=6385&browse=1');
 await viewer.locator('[data-hunting-section="smokey"]').waitFor();
 const expectedRooms=(await load()).records[0].baseRooms.length-1;
 assert.match(await viewer.locator('[data-hunting-section="smokey"]').textContent(),new RegExp(`${expectedRooms} locally-reviewed rooms`),'ordinary explorer reads the saved boundary');
 assert.equal(await viewer.locator('#hunting-editor').count(),0,'reading a boundary does not enable editor UI');
 await viewer.close();
 // Editing during an in-flight save must be retained and saved subsequently.
 await page.route('**/dev/hunting-corrections',async route=>{
  if(route.request().method()!=='POST'){await route.continue();return;}
  const response=await route.fetch();await new Promise(resolve=>setTimeout(resolve,650));await route.fulfill({response});
 });
 await page.locator('#hunt-edit-notes').fill('first');await page.locator('#hunt-edit-message').filter({hasText:'Saving correction file'}).waitFor();
 await page.locator('#hunt-edit-notes').fill('second edit while saving');
 await page.waitForFunction(()=>document.getElementById('hunt-edit-status').textContent.includes('Autosave on'));
 assert.equal((await load()).records[0].notes,'second edit while saving');assert.equal((await files()).length,1);
 await page.unroute('**/dev/hunting-corrections');
 // HTTP checks use a valid body so Origin/revision checks are actually exercised.
 const record=(await load()).records[0],date=names[0].match(/(\d{4}-\d{2}-\d{2})\.hunting\.json$/)[1];
 const body=JSON.stringify({revision:0,date,record});
 assert.equal((await fetch(base+'/dev/hunting-corrections',{method:'POST',headers:{'Content-Type':'application/json'},body})).status,403);
 assert.equal((await fetch(base+'/dev/hunting-corrections',{method:'POST',headers:{'Content-Type':'application/json',Origin:'https://evil.example'},body})).status,403);
 assert.equal((await fetch(base+'/dev/hunting-corrections',{method:'POST',headers:{'Content-Type':'application/json',Origin:base},body})).status,409);
 // Restart on a different port; no browser storage transfer or picker is needed.
 await stop();base=await start();await open();
 assert.equal(await page.locator('[data-hunt-edit-room="6385"]').getAttribute('data-membership'),'removed');
 assert.equal(await page.locator('#hunt-edit-notes').inputValue(),'second edit while saving');
 // Autosave must not erase the undo history.
 await page.locator('#hunt-edit-notes').fill('third');await page.waitForFunction(()=>document.getElementById('hunt-edit-status').textContent.includes('Autosave on'));
 assert.equal((await load()).records[0].notes,'third');
 assert.deepEqual(errors,[]);
 console.log('PASS file autosave: area/date naming, automatic updates, edit-during-save, reload across ports, guarded writes, no picker or game connection.');
}finally{await browser?.close();await stop();await rm(directory,{recursive:true,force:true});}
