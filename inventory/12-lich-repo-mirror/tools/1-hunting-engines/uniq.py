import difflib,sys
a=open('bs.txt',encoding='utf-8',errors='replace').read().split('\n')
b=open('b2.txt',encoding='utf-8',errors='replace').read().split('\n')
sm=difflib.SequenceMatcher(None,a,b,autojunk=False)
out=open('b2_only.txt','w',encoding='utf-8')
for tag,i1,i2,j1,j2 in sm.get_opcodes():
    if tag in ('replace','insert'):
        out.write(f"=== {tag} bs {i1+1}-{i2} b2 {j1+1}-{j2}\n")
        for j in range(j1,j2):
            out.write(f"{j+1}: {b[j]}\n")
