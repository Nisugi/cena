// Freeze reference data separately from map ownership and exact UID matches.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {loadCreatureCatalogue,creaturesForRegion} from './creatures.mjs';

const [mode,input,output]=process.argv.slice(2);
if(mode==='freeze'){
 const catalogue=loadCreatureCatalogue(input);
 // Only fields consumed by the explorer, not arbitrary template contents.
 const fields=['name','areas','level','family','type','undead','boss','max_hp','size','url'];
 for(const e of catalogue.entries)e.record=Object.fromEntries(fields.filter(k=>k in e.record).map(k=>[k,e.record[k]]));
 fs.writeFileSync(output,JSON.stringify(catalogue)+'\n',{flag:'wx'});
}else if(mode==='match'){
 const catalogue=JSON.parse(fs.readFileSync(input,'utf8'));
 for(const slug of fs.readdirSync(output).sort()){
  const directory=path.join(output,slug),dataPath=path.join(directory,'data.json');
  if(!fs.existsSync(dataPath))continue;
  const bytes=fs.readFileSync(dataPath),data=JSON.parse(bytes),seen=new Set(),display=[];
  for(const [group,scene] of Object.entries(data.scenes))for(const room of scene.sheet.rooms){
   const id=String(room.id);if(seen.has(id))throw Error('Duplicate displayed room '+id);
   seen.add(id);display.push({id,group});
  }
  if(seen.size!==Object.keys(data.rooms).length||Object.keys(data.rooms).some(id=>!seen.has(id)))throw Error('Incomplete displayed membership');
  const result=creaturesForRegion(catalogue,Object.values(data.rooms),display);
  const sidecar={schema:'hydra-region-context-v1',data_sha256:crypto.createHash('sha256').update(bytes).digest('hex'),
   engine_revision:data.provenance.engine_revision,note:'Offline habitat reference. Does not alter topology, area assignments or travel.',
   coverage:result.coverage,creatures:result.creatures.map(({id,record,source,associations})=>({id,...record,areas:undefined,source,associations}))};
  fs.writeFileSync(path.join(directory,'region-data.json'),JSON.stringify(sidecar)+'\n');
 }
}else throw Error('Usage: context.mjs freeze TEMPLATE_DIR NEW_CATALOGUE | match CATALOGUE BUNDLE_DIR');
