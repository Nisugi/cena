import {parseCorrections} from './hunting-corrections.mjs';

export function correctionDate(now=new Date()){
 return `${now.getFullYear()}-${String(now.getMonth()+1).padStart(2,'0')}-${String(now.getDate()).padStart(2,'0')}`;
}
// Explicit local developer host; a normal viewer answers 404 and stays read-only.
export async function huntingFiles({fetcher=fetch,signal}={}){
 const endpoint='/dev/hunting-corrections',response=await fetcher(endpoint,{signal});
 if(response.status===404)return null;
 if(!response.ok)throw Error('Cannot load correction files; file autosave is unavailable');
 const loaded=await response.json(),document=parseCorrections(JSON.stringify(loaded.document));
 if(!Number.isSafeInteger(loaded.revision)||typeof loaded.directory!=='string')throw Error('Invalid correction-file response');
 let revision=loaded.revision,queue=Promise.resolve();
 return {document,directory:loaded.directory,save(records){
  const job=queue.then(async()=>{
   const saved=[];
   for(const record of records){
    const result=await fetcher(endpoint,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({record,revision,date:correctionDate()}),signal});
    if(!result.ok){let message='File autosave failed ('+result.status+')';try{message=(await result.json()).error||message;}catch{}throw Error(message);}
    const value=await result.json();if(!Number.isSafeInteger(value.revision)||typeof value.path!=='string')throw Error('Invalid file-save acknowledgement');
    revision=value.revision;saved.push(value);
   }
   return saved;
  });
  queue=job.catch(()=>{});return job;
 }};
}
