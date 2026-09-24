import {routeTo} from './navigation.mjs';

// Explicit offline adapter. Never supplies live location or executes travel.
export function offlineSource(base,{fetcher=fetch}={}){
 return {async load({signal}={}){
  const response=await fetcher(new URL('data.json',base),{signal});
  if(!response.ok)throw Error('Cannot load native scenes');
    const text=await response.text(),data=JSON.parse(text);
    let regionContext=null,contextWarning='';
    // Every regional bundle may supply the same hash-bound sidecar. Failure
    // is disclosed, never allowed to attach stale room associations.
    try{
      const extra=await fetcher(new URL('region-data.json',base),{signal});
      if(!extra.ok)throw Error('Creature reference sidecar unavailable');
      const candidate=await extra.json();
      const digest=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text));
      const hash=[...new Uint8Array(digest)].map(b=>b.toString(16).padStart(2,'0')).join('');
      if(candidate.schema!=='hydra-region-context-v1'||candidate.data_sha256!==hash)throw Error('Creature references do not match this map snapshot; rebuild region context');
      regionContext=candidate;
    }catch(error){if(signal?.aborted)throw error;contextWarning=error.message;}
    return {data,regionContext,contextWarning};
 },previewRoute:routeTo};
}
