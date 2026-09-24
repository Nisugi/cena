// Native loopback server, real CSP and assets. Never attaches a game session.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {mkdir} from 'node:fs/promises';
import {resolve} from 'node:path';

const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
const executable=process.env.ATLAS_PREVIEW_BIN||resolve('target/debug/examples/atlas_preview');
const server=spawn(executable,[],{stdio:['ignore','pipe','inherit']});
let browser;
try{
 const url=await new Promise((resolve,reject)=>{
  const timer=setTimeout(()=>reject(Error('Preview server timeout')),15000);
  server.once('error',reject);server.once('exit',code=>reject(Error('Preview server exited '+code)));
  server.stdout.on('data',chunk=>{const match=String(chunk).match(/http:\/\/127\.0\.0\.1:\d+\/atlas\/corpus\//);if(match){clearTimeout(timer);resolve(match[0]);}});
 });
 const base=new URL(url).origin;
 const headers=(await fetch(url)).headers;
 assert.match(headers.get('content-security-policy'),/script-src 'self'/);
 assert.ok(!headers.get('content-security-policy').includes('unsafe-inline'));
 assert.equal(headers.get('x-content-type-options'),'nosniff');
 assert.equal((await fetch(url+'?bad=1')).status,403);
 assert.equal((await fetch(base+'/atlas/landing/.env')).status,404);
 assert.equal((await fetch(url,{headers:{Origin:'https://evil.example'}})).status,403);
 browser=await chromium.launch({headless:true,executablePath:process.env.BROWSER_EXECUTABLE_PATH||undefined});
 const page=await browser.newPage({viewport:{width:1600,height:1050}}),errors=[],requests=[];
 page.on('pageerror',e=>errors.push(e.message));
 page.on('console',m=>{if(m.type()==='error'&&!m.text().startsWith('Failed to load resource:'))errors.push(m.text());});
 page.on('response',r=>{if(r.status()>=400&&!r.url().endsWith('/favicon.ico'))errors.push(`${r.status()} ${r.url()}`);});
 page.on('request',r=>requests.push({method:r.method(),url:r.url()}));
 page.on('websocket',()=>errors.push('Explorer opened a WebSocket'));
 await page.goto(url);
 await page.locator('#world-map [data-world-region]').first().waitFor();
 assert.ok(await page.locator('#world-map [data-world-region]').count()>5);
 await page.locator('#world-query').fill('228');
 await page.locator('#world-results a').filter({hasText:/228/}).first().waitFor();
 await page.goto(base+'/atlas/landing/#room=228&browse=1');
 await page.locator('#room-name').filter({hasText:'#228'}).waitFor({timeout:60000});
 assert.ok(await page.locator('#map .node').count()>100);
 assert.match(await page.locator('#coverage').textContent(),/Automated/);
 // Camera motion must retain the expensive room/connector DOM.
 const pan=await page.evaluate(async()=>{
  const canvas=document.getElementById('canvas'),node=document.querySelector('#map .node'),cards=[...document.querySelectorAll('#drawing-links button')];
  const before=document.getElementById('map').getAttribute('viewBox'),box=canvas.getBoundingClientRect();
  canvas.dispatchEvent(new PointerEvent('pointerdown',{button:0,clientX:box.x+100,clientY:box.y+100,bubbles:true}));
  for(let i=1;i<=8;i++){
   window.dispatchEvent(new PointerEvent('pointermove',{clientX:box.x+100+i*8,clientY:box.y+100+i,bubbles:true}));
   await new Promise(resolve=>requestAnimationFrame(resolve));
  }
  window.dispatchEvent(new PointerEvent('pointerup'));
  await new Promise(resolve=>requestAnimationFrame(resolve));
  return {retained:node.isConnected&&cards.every(n=>n.isConnected),moved:before!==document.getElementById('map').getAttribute('viewBox')};
 });
 assert.deepEqual(pan,{retained:true,moved:true});
 // Route preview crosses the catacomb boundary using actual directed exits.
 await page.goto(base+'/atlas/landing/#room=7501');
 await page.locator('#room-name').filter({hasText:'#7501'}).waitFor({timeout:60000});
 assert.match(await page.locator('#route-status').textContent(),/Preview start/);
 await page.locator('#clear-route').click();
 assert.ok(!/ → /.test(await page.locator('#route-status').textContent()));
 await page.locator('#zoom-in').click();await page.locator('#zoom-out').click();
 if(!await page.locator('#legend').evaluate(e=>e.open))await page.locator('#legend > summary').click();
 await page.locator('#hide-labels').check();
 await page.locator('#about').click();
 assert.match(await page.locator('#dialog-body').textContent(),/Native mode/);
 await page.locator('#close-dialog').click();
 const graph=await page.evaluate(()=>fetch('data.json').then(r=>r.json()));
 const search=await (await fetch(base+'/atlas/corpus/search.json')).json();
 const outside=Object.values(graph.rooms).flatMap(r=>r.exits.map(e=>({from:r.id,to:e.to})))
  .find(e=>!graph.rooms[e.to]&&search.some(r=>r.id===e.to));
 assert.ok(outside,'Fixture needs a real cross-region exit');
 await page.goto(base+`/atlas/landing/#room=${outside.from}&browse=1`);
 await page.locator('#room-name').filter({hasText:'#'+outside.from}).waitFor({timeout:60000});
 await page.locator('#exits button').filter({hasText:'#'+outside.to}).click();
 await page.locator('#room-name').filter({hasText:'#'+outside.to}).waitFor({timeout:60000});
 assert.ok(!new URL(page.url()).pathname.includes('/landing/'));
 for(const slug of ['mist-harbor','icemule']){
  await page.goto(base+`/atlas/${slug}/`);
  await page.waitForFunction(()=>document.getElementById('room-name').textContent.includes('#'),null,{timeout:60000});
  assert.ok(await page.locator('#map .node').count()>10);
 }
 await mkdir('target/atlas-evidence',{recursive:true});
 await page.screenshot({path:'target/atlas-evidence/icemule.png',fullPage:true});
 assert.deepEqual(errors,[]);
 assert.ok(requests.every(r=>r.method==='GET'&&r.url.startsWith(base)),'Read-only same-origin requests only');
 console.log('PASS native explorer: CSP, allowlist, world/local browsing, preview routing, cross-region exit, no game commands.');
}finally{await browser?.close();server.kill('SIGTERM');}
