// One effective membership calculation for the editor and ordinary explorer.
// Corrections describe chosen boundaries, never new creature-spawn evidence.
import {huntingSections} from './hunting-sections.mjs';
import {huntingDestinations} from './hunting-destinations.mjs';
import {assessCorrection,selectionSource,emptyCorrections,parseCorrections} from './hunting-corrections.mjs';

export function huntingCatalogue(data,context,document=emptyCorrections()){
 const records=parseCorrections(JSON.stringify(document)).records,areas=new Map();
 return {withCorrections(document){return huntingCatalogue(data,context,document);},area(id){
  if(areas.has(id))return areas.get(id);
  const model=huntingSections(data,context,id);
  const raw=model.hunts.map(h=>{
   const source=selectionSource(data,id,h),record=records.find(r=>r.area===id&&r.hunt===h.id);
   const assessment=assessCorrection(record,source);
   return {...h,roomIds:assessment.rooms,source,record:record||null,
    boundaryStatus:assessment.status==='saved'?'locally-reviewed':assessment.status==='review'?'needs-review':'generated',
    boundaryIssues:assessment.issues,reviewRooms:assessment.reviewRooms};
  });
  const {hunts,known,sectionsHidden}=huntingDestinations(data,id,raw,records);
  const byHuntRoom=new Map();for(const hunt of hunts)for(const room of hunt.roomIds)if(!byHuntRoom.has(room))byHuntRoom.set(room,hunt);
  const unmatchedCorrections=records.filter(r=>r.area===id&&!known.has(r.hunt));
  const result={...model,hunts,byHuntRoom,unmatchedCorrections,sectionsHidden};areas.set(id,result);return result;
 }};
}

export async function loadHuntingCorrections({fetcher=fetch,signal}={}){
 const r=await fetcher('/atlas/hunting-corrections',{signal});
 if(r.status===404)return emptyCorrections();
 if(!r.ok)throw Error('Saved hunting boundaries could not be read. No correction was silently discarded.');
 return parseCorrections(JSON.stringify(await r.json()));
}
