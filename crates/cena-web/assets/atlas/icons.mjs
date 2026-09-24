// Small monochrome SVGs: no emoji/font dependency or remote assets.
const paths={
 coins:['M4 7c0-4 12-4 12 0s-12 4-12 0Z','M4 7v5c0 4 12 4 12 0V7','M8 16v3c0 4 12 4 12 0v-6','M17 9c5 0 5 7-1 7'],
 hide:['M7 3 4 6l3 4-3 7 4 4 4-3 4 3 4-4-3-7 3-4-3-3-5 3Z'],
 gem:['M3 8 7 3h10l4 5-9 13Z','M3 8h18M7 3l5 18 5-18'],
 scales:['M12 3v18M7 21h10M4 7h16','M4 7 1 14h6ZM20 7l-3 7h6Z'],
 shield:['M12 2 21 6v7c0 5-9 9-9 9s-9-4-9-9V6Z','M12 6v11M8 10h8'],
 key:['M10 9a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z','M10 9h12M17 9v4M21 9v3'],
 cross:['M9 3h6v6h6v6h-6v6H9v-6H3V9h6Z'],
 leaf:['M20 3C4 2 1 11 6 17c6 5 16 1 14-14Z','M4 21 16 8M10 15v-5'],
 flask:['M9 2h6M10 2v7l-7 11c-1 2 19 2 18 0L14 9V2','M7 14h10M10 18h1M14 17h1']
};
export function icon(category,color='currentColor'){
 const ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg');
 for(const [k,v] of Object.entries({viewBox:'0 0 24 24',width:18,height:18,fill:'none',stroke:color,'stroke-width':1.7,'stroke-linecap':'round','stroke-linejoin':'round','aria-hidden':'true'}))svg.setAttribute(k,v);
 if(paths[category.icon])for(const d of paths[category.icon]){const p=document.createElementNS(ns,'path');p.setAttribute('d',d);svg.append(p);}
 else{const text=document.createElementNS(ns,'text');for(const [k,v] of Object.entries({x:12,y:18,'text-anchor':'middle',fill:color,stroke:'none','font-size':20}))text.setAttribute(k,v);text.textContent=category.symbol;svg.append(text);}
 return svg;
}
