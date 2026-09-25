import re
srcs=['E:/Cena/reference/scripts/scripts/bigshot.lic','C:/Gemstone/scripts/scripts/bigshot.lic','E:/Gemstone/dev/lich5-docker/scripts/bigshot.lic','C:/Gemstone/lich-5/scripts/bigshot.lic']
known=set()
for s in srcs:
    for l in open(s,'rb').read().decode('utf-8','replace').splitlines():
        known.add(re.sub(r'\s+',' ',l.strip()))
b=open('E:/Cena/reference/lich_repo_mirror/lib/bigshit2.lic','rb').read().decode('utf-8','replace').splitlines()
out=open('b2_novel.txt','w',encoding='utf-8')
n=0
for i,l in enumerate(b,1):
    k=re.sub(r'\s+',' ',l.strip())
    if k and k not in known and not k.startswith('#') and k not in ('end','}','else','begin','rescue'):
        out.write(f"{i}: {l}\n"); n+=1
print(n)
