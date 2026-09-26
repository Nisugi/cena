import re
bs = open('bs.txt', encoding='utf-8').read().split('\n')
b2 = open('b2.txt', encoding='utf-8').read().split('\n')


def words(line):
    start = line.index('(?:') + 3
    end = line.index(').*?)', start)
    body = line[start:end]
    out = set()
    for w in body.split('|'):
        w = w.replace('!?', '').lstrip('!')
        out.add(w)
    return out


wbs = words(bs[3196])
w2 = words(b2[3021])
print('bigshot', len(wbs), 'bigshit2', len(w2))
print('b2 only:', sorted(w2 - wbs))
print('bs only:', sorted(wbs - w2))
