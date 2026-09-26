// A deliberately small data-only reader for the installed creature literals.
// Never evaluates Ruby. Calls, interpolation, constants and other syntax fail.
export function readRubyData(source){
 const lex=/\s+|#[^\n]*|"(?:\\.|[^"\\])*"|\.\.\.?|[+-]?\d+(?:\.\d+)?|[A-Za-z_][A-Za-z_0-9]*|[{}\[\]():,]/y;
 let offset=0,tokens=[];
 while(offset<source.length){
  // Ruby block-comment markers must begin at column zero. Handle them during
  // lexing, not with source-wide replacements that could alter quoted data.
  if((offset===0||source[offset-1]==='\n')&&source.startsWith('=begin',offset)){
   const start=/^=begin(?:[ \t][^\r\n]*)?(?:\r?\n|$)/.exec(source.slice(offset));
   if(start){const end=/^=end(?:[ \t][^\r\n]*)?(?:\r?\n|$)/gm;end.lastIndex=offset+start[0].length;
    if(!end.exec(source))throw Error(`Unterminated Ruby block comment at offset ${offset}`);
    offset=end.lastIndex;continue;}
  }
  lex.lastIndex=offset;const m=lex.exec(source);if(!m)throw Error(`Unsupported Ruby data at offset ${offset}`);offset=lex.lastIndex;if(!/^\s|^#/.test(m[0]))tokens.push(m[0]);
 }
 let at=0;
 const take=()=>tokens[at++],expect=t=>{if(take()!==t)throw Error(`Expected ${t} at token ${at}`);};
 function value(){let v=atom();if(tokens[at]==='..'||tokens[at]==='...'){const exclusive=take()==='...';const end=atom();if(typeof v!=='number'||typeof end!=='number')throw Error('Only numeric ranges supported');v={min:v,max:end,excludeEnd:exclusive};}return v;}
 function atom(){const t=take();
  if(t==='nil')return null;if(t==='true')return true;if(t==='false')return false;
  if(t==='('){const v=value();expect(')');return v;}
  if(t===':'){const s=take();if(!/^[A-Za-z_]\w*$/.test(s||''))throw Error('Invalid symbol');return s;}
  if(t==='['){const a=[];while(tokens[at]!==']'){a.push(value());if(tokens[at]!==',')break;take();}expect(']');return a;}
  if(t==='{'){const o=Object.create(null);while(tokens[at]!=='}'){const k=take();if(!/^[A-Za-z_]\w*$/.test(k||''))throw Error('Expected hash label');expect(':');if(Object.hasOwn(o,k))throw Error(`Duplicate key ${k}`);o[k]=value();if(tokens[at]!==',')break;take();}expect('}');return o;}
  if(t?.startsWith('"')){if(t.includes('#{')||t.includes('#@')||t.includes('#$'))throw Error('Interpolation is not data');
   // Ruby permits literal newlines in quoted strings; JSON requires escaping
   // control characters. Preserve their values without evaluating Ruby escapes.
   return JSON.parse(t.replace(/[\u0000-\u001f]/g,c=>JSON.stringify(c).slice(1,-1)));}
  if(/^[+-]?\d+(\.\d+)?$/.test(t||'')){const n=Number(t);if(!Number.isFinite(n)||Math.abs(n)>Number.MAX_SAFE_INTEGER)throw Error('Numeric precision loss');return n;}
  throw Error(`Unsupported Ruby value ${t}`);
 }
 const result=value();if(at!==tokens.length)throw Error('Trailing Ruby code is not data');return result;
}

export function rangeContains(value,number){
 if(typeof value==='number')return value===number;
 return value!==null&&typeof value==='object'&&typeof value.min==='number'&&typeof value.max==='number'
   &&number>=value.min&&(value.excludeEnd?number<value.max:number<=value.max);
}
