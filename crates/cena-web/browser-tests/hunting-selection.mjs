// The user's selection path, not an internal editor-only selector. Offline only.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {resolve} from 'node:path';
import {mkdir} from 'node:fs/promises';
const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
const server=spawn(process.env.ATLAS_PREVIEW_BIN||resolve('target/debug/examples/atlas_preview'),[],{stdio:['ignore','pipe','inherit']});
let browser;
try{
 const base=await new Promise((resolve,reject)=>{
  const timer=setTimeout(()=>reject(Error('Preview timeout')),15000);
  server.once('error',reject);server.once('exit',c=>reject(Error('Preview exited '+c)));
  server.stdout.on('data',chunk=>{const match=String(chunk).match(/http:\/\/127\.0\.0\.1:\d+/);if(match){clearTimeout(timer);resolve(match[0]);}});
 });
 browser=await chromium.launch({headless:true,executablePath:process.env.BROWSER_EXECUTABLE_PATH||undefined});
 const page=await browser.newPage({viewport:{width:1700,height:1100}}),errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 await page.goto(base+'/atlas/landing/#room=6952&browse=1&devhunt=1');
 await page.locator('#hunting-editor').waitFor({timeout:60000});
 assert.equal(await page.locator('#hunting-editor').getAttribute('data-hunt'),'');
 assert.ok(await page.locator('#hunt-edit-add').isDisabled());
 const canvasBefore=await page.locator('#canvas').boundingBox();
 await page.locator('.hunting-list').evaluate(n=>{n.scrollTop=n.scrollHeight;});
 assert.deepEqual(await page.locator('#canvas').boundingBox(),canvasBefore,'Scrolling the group list must not move the map');
 assert.equal(await page.evaluate(()=>window.scrollY),0);
 await page.locator('[data-hunting-section="dungeon-3:roa_ter"]').click();
 assert.match(await page.locator('#hunt-edit-title').textContent(),/Editing Roa'ter/,'Clicking a hunting group must change the actual edit target');
 await page.locator('#fit').click();
 await page.locator('#hunt-edit-add').click();
 await page.locator('#canvas').scrollIntoViewIfNeeded();
 const p=await page.locator('#map [data-room="7001"]').boundingBox();
 await page.mouse.click(p.x+p.width/2,p.y+p.height/2);
 await page.locator('#hunt-edit-save').click();
 const saved=JSON.parse(await page.evaluate(()=>localStorage.getItem('hydra.hunting.corrections.v1')));
 assert.equal(saved.records.length,1);
 assert.equal(saved.records[0].hunt,'dungeon-3:roa_ter');
 assert.deepEqual(saved.records[0].added,[7001]);
 assert.match(await page.locator('#hunt-edit-feedback').textContent(),/Added room #7001 to Roa'ter/);
 // Labels are selection controls even while Add is active, not room edits.
 const label=page.locator('[data-hunting-label="ward"]');
 await label.click();
 assert.equal(await page.locator('#hunting-editor').getAttribute('data-hunt'),'ward');
 assert.equal(await page.locator('#hunt-edit-browse').getAttribute('aria-pressed'),'true');
 assert.equal(await page.locator('[data-hunting-section="ward"]').getAttribute('aria-pressed'),'true');
 assert.equal(await page.locator('[data-hunting-section][aria-pressed="true"]').count(),1);
 assert.ok(!(await page.locator('#hunt-edit-status').textContent()).includes('unsaved'),'Selecting a map label must not edit any room');
 await page.locator('#hunt-edit-remove').click();
 const roam=page.locator('[data-hunting-label="dungeon-3:roa_ter"]');
 await roam.focus();await roam.press('Enter');
 assert.equal(await page.locator('#hunting-editor').getAttribute('data-hunt'),'dungeon-3:roa_ter');
 assert.equal(await page.locator('#hunt-edit-browse').getAttribute('aria-pressed'),'true');
 assert.deepEqual(JSON.parse(await page.evaluate(()=>localStorage.getItem('hydra.hunting.corrections.v1'))),saved);
 // Smaller desktop windows still keep list, map and actions on one screen.
 await page.setViewportSize({width:1280,height:800});
 await page.waitForTimeout(150);
 const layout=await page.evaluate(()=>{
  const bounds=id=>{const r=document.getElementById(id).getBoundingClientRect();return {top:r.top,bottom:r.bottom,height:r.height};};
  return {canvas:bounds('canvas'),tools:bounds('hunting-editor'),height:innerHeight,scroll:document.documentElement.scrollHeight};
 });
 assert.ok(layout.canvas.height>300,JSON.stringify(layout));
 assert.ok(layout.tools.top>=0&&layout.tools.bottom<=layout.canvas.top);
 assert.ok(layout.canvas.bottom<=layout.height+1);
 assert.ok(layout.scroll<=layout.height+1,'Editor must not require document scrolling');
 await mkdir('target/atlas-evidence',{recursive:true});
 await page.screenshot({path:'target/atlas-evidence/hunting-workspace.png'});
 await page.setViewportSize({width:1000,height:800});
 await page.locator('#hunt-inspector-toggle').click();
 assert.ok(await page.locator('#hunt-inspector-close').isVisible());
 await page.locator('#hunt-inspector-close').click();
 assert.equal(await page.locator('#hunt-inspector-toggle').getAttribute('aria-expanded'),'false');
 assert.equal(await page.evaluate(()=>window.scrollY),0);
 // The consolidated destination is editable and saves under its canonical ID.
 await page.locator('[data-hunting-section="destination:dark-cavern"]').click();
 assert.match(await page.locator('#hunt-edit-title').textContent(),/Dark Cavern/);
 await page.locator('#hunt-edit-save').click();
 const canonical=JSON.parse(await page.evaluate(()=>localStorage.getItem('hydra.hunting.corrections.v1')));
 assert.ok(canonical.records.some(r=>r.hunt==='destination:dark-cavern'));
 assert.deepEqual(canonical.records.find(r=>r.hunt==='dungeon-3:roa_ter'),saved.records[0],'Saving the cavern preserves the other hunt');
 await page.reload();
 await page.locator('[data-hunting-section="destination:dark-cavern"]').click();
 assert.match(await page.locator('#hunt-edit-status').textContent(),/Saved in this browser/);
 assert.deepEqual(JSON.parse(await page.evaluate(()=>localStorage.getItem('hydra.hunting.corrections.v1'))),canonical,'Reload preserves the canonical boundary');
 assert.deepEqual(errors,[]);
 console.log('PASS hunting selection: visible group selection is the edit target; Add cannot write another group.');
}finally{await browser?.close();server.kill('SIGTERM');}
