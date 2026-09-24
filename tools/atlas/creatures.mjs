import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {readRubyData,rangeContains} from './ruby-data.mjs';

const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const validUid=n=>Number.isSafeInteger(n)&&n>=0;
const validSpan=v=>validUid(v)||(v&&typeof v==='object'&&validUid(v.min)&&validUid(v.max)&&v.min<=v.max);

// Read data literals only. An unsupported template remains a disclosed catalogue
// gap; never run Ruby, strip executable syntax, or guess a missing habitat.
export function loadCreatureCatalogue(directory){
  const dir=directory instanceof URL?fileURLToPath(directory):path.resolve(directory);
  const entries=[],failures=[],inventory=[];
  for(const name of fs.readdirSync(dir).filter(n=>n.endsWith('.rb')).sort()){
    const sourcePath=path.join(dir,name),bytes=fs.readFileSync(sourcePath),sha256=hash(bytes);
    inventory.push({name,sha256});
    try{
      const record=readRubyData(bytes.toString('utf8'));
      if(!record||typeof record.name!=='string'||!Array.isArray(record.areas))throw Error('Missing creature name or habitat list');
      entries.push({id:name.slice(0,-3),record,source:{path:'Lich5/lib/gemstone/creatures/'+name,sha256,
        captured:'2026-09-22',scope:'Installed Lich creature template; offline snapshot, not live observations or a fresh wiki verification'}});
    }catch(error){failures.push({path:'Lich5/lib/gemstone/creatures/'+name,sha256,error:error.message});}
  }
  return {entries,failures,source:{path:'Lich5/lib/gemstone/creatures',sha256:hash(JSON.stringify(inventory)),fileCount:inventory.length,
    scope:'Sorted template filename/content-hash inventory. Individual records retain their own hashes.'}};
}

// Membership comes only from UID overlap. Area labels are display text, not keys:
// they cannot establish exact rooms and are deliberately not used as a fallback.
export function creaturesForRegion(catalogue,nativeRooms,displayRooms){
  const native=new Map(nativeRooms.map(r=>[String(r.id),r]));
  const rooms=displayRooms.map(r=>{
    const raw=native.get(r.id);if(!raw)throw Error(`Missing native room for creature association: ${r.id}`);
    return {id:r.id,group:r.group,uids:(raw.uid||[]).filter(validUid)};
  });
  const creatures=[],covered=new Set(),groups=new Map();
  for(const r of rooms){const g=groups.get(r.group)||{id:r.group,rooms:0,matchedRooms:0,creatures:0,roomsWithoutUid:0};
    g.rooms++;if(!r.uids.length)g.roomsWithoutUid++;groups.set(r.group,g);}
  let recordsWithoutHabitatUids=0,invalidHabitatEntries=0;
  for(const entry of catalogue.entries){
    const all=(entry.record.areas||[]).flatMap(a=>Array.isArray(a.uids)?a.uids:[]);
    const spans=all.filter(validSpan);invalidHabitatEntries+=all.length-spans.length;
    if(!spans.length){recordsWithoutHabitatUids++;continue;}
    const byGroup=new Map();
    for(const r of rooms){
      if(!r.uids.some(uid=>spans.some(span=>rangeContains(span,uid))))continue;
      const ids=byGroup.get(r.group)||[];ids.push(r.id);byGroup.set(r.group,ids);covered.add(r.id);
    }
    if(!byGroup.size)continue;
    const associations=[...byGroup].map(([group,roomIds])=>{
      groups.get(group).creatures++;
      return {group,roomIds,basis:'room_uid_match',note:'Installed creature-template habitat UIDs overlap these native room records. Reference habitat evidence only; not live occupants, guaranteed spawns, or a complete list.'};
    });
    creatures.push({...entry,associations,wiki:null});
  }
  for(const r of rooms)if(covered.has(r.id))groups.get(r.group).matchedRooms++;
  return {creatures,coverage:{basis:'room_uid_match',source:catalogue.source,
    matchedCreatures:creatures.length,matchedRooms:covered.size,totalRooms:rooms.length,
    roomsWithoutUid:rooms.filter(r=>!r.uids.length).length,roomsWithoutAssociation:rooms.length-covered.size,
    groups:[...groups.values()],catalogue:{parsedRecords:catalogue.entries.length,
      recordsWithoutHabitatUids,invalidHabitatEntries,unsupportedRecords:catalogue.failures},
    note:'No association means unknown, not creature-free. Missing habitat UIDs and unsupported templates are not filled by area-name guesses. Levels and stats are installed reference values; wiki links are provided, not represented as fresh verification.'}};
}
