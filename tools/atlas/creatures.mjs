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

// Two independent room-level sources, never area-name or adjacency guesses.
// Tags must equal a unique catalogue name (case/outer whitespace aside).
const normalizedName=s=>typeof s==='string'?s.trim().toLowerCase():'';
export function creaturesForRegion(catalogue,nativeRooms,displayRooms){
  const names=new Map();
  for(const e of catalogue.entries){const name=normalizedName(e.record.name);if(name)names.set(name,(names.get(name)||0)+1);}
  const native=new Map(nativeRooms.map(r=>[String(r.id),r]));
  const rooms=displayRooms.map(r=>{
    const raw=native.get(r.id);if(!raw)throw Error(`Missing native room for creature association: ${r.id}`);
    return {id:r.id,group:r.group,uids:(raw.uid||[]).filter(validUid),tags:new Set((raw.tags||[]).map(normalizedName).filter(Boolean))};
  });
  const creatures=[],covered=new Set(),groups=new Map();
  const byBasis={room_uid_match:new Set(),room_tag_match:new Set()};
  for(const r of rooms){const g=groups.get(r.group)||{id:r.group,rooms:0,matchedRooms:0,creatures:0,roomsWithoutUid:0};
    g.rooms++;if(!r.uids.length)g.roomsWithoutUid++;groups.set(r.group,g);}
  let recordsWithoutHabitatUids=0,invalidHabitatEntries=0;
  for(const entry of catalogue.entries){
    const all=(entry.record.areas||[]).flatMap(a=>Array.isArray(a.uids)?a.uids:[]);
    const spans=all.filter(validSpan);invalidHabitatEntries+=all.length-spans.length;
    if(!spans.length)recordsWithoutHabitatUids++;
    const name=normalizedName(entry.record.name),byGroup=new Map();
    for(const r of rooms){
      const uid=r.uids.some(uid=>spans.some(span=>rangeContains(span,uid)));
      const tag=names.get(name)===1&&r.tags.has(name);
      if(!uid&&!tag)continue;
      const evidence=byGroup.get(r.group)||{room_uid_match:[],room_tag_match:[]};
      for(const [basis,matched] of [['room_uid_match',uid],['room_tag_match',tag]])if(matched){
        evidence[basis].push(r.id);byBasis[basis].add(r.id);
      }
      byGroup.set(r.group,evidence);covered.add(r.id);
    }
    if(!byGroup.size)continue;
    const associations=[...byGroup].flatMap(([group,evidence])=>{
      groups.get(group).creatures++;
      return Object.entries(evidence).filter(([,ids])=>ids.length).map(([basis,roomIds])=>({group,roomIds,basis,
        note:(basis==='room_uid_match'?'Installed creature-template habitat UIDs overlap these native room records.':
          'Native room tags exactly match a unique creature-template name, ignoring case and outer whitespace; not a habitat UID match.')+
          ' Reference habitat evidence only; not live occupants, guaranteed spawns, or a complete list.'}));
    });
    creatures.push({...entry,associations,wiki:null});
  }
  for(const r of rooms)if(covered.has(r.id))groups.get(r.group).matchedRooms++;
  return {creatures,coverage:{basis:'room_uid_or_tag_match',source:catalogue.source,
    matchedRoomsByBasis:Object.fromEntries(Object.entries(byBasis).map(([basis,ids])=>[basis,ids.size])),
    tagOnlyRooms:[...byBasis.room_tag_match].filter(id=>!byBasis.room_uid_match.has(id)).length,
    matchedCreatures:creatures.length,matchedRooms:covered.size,totalRooms:rooms.length,
    roomsWithoutUid:rooms.filter(r=>!r.uids.length).length,roomsWithoutAssociation:rooms.length-covered.size,
    groups:[...groups.values()],catalogue:{parsedRecords:catalogue.entries.length,
      recordsWithoutHabitatUids,invalidHabitatEntries,ambiguousTagNames:[...names].filter(([,count])=>count>1).map(([name])=>name),unsupportedRecords:catalogue.failures},
    note:'No association means unknown, not creature-free. UID overlap and exact unique creature-name room tags are separate evidence sources; counts can overlap. No area-name, fuzzy-name or adjacency guesses. Levels and stats are installed reference values; wiki links are provided, not represented as fresh verification.'}};
}
