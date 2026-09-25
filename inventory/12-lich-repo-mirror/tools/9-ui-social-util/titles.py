import re
t = open('E:/Cena/reference/lich_repo_mirror/lib/imagination.lic', 'rb').read().decode('utf-8', 'replace').splitlines()[214]
m = re.search(r'You see \(\?:\\b\(\?:(.*?)\)\\b', t)
titles = m.group(1).split('|')
print(len(titles), 'titles; first', titles[:5], 'last', titles[-3:])
