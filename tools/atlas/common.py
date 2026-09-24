"""Offline bundle utilities; no game access or ownership inference."""
from collections import defaultdict
import hashlib
import json
from pathlib import Path

def read(path):
    return json.loads(path.read_text(encoding='utf-8'))

def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, separators=(',', ':')) + '\n', encoding='utf-8', newline='\n')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def plain_edge(edge):
    cmd = edge.get('cmd', '').lower()
    return bool(cmd) and not any(s in cmd for s in ('transport', 'urchin ', 'portmaster', 'teleport', 'travel', 'wagon'))

def components(ids, graph):
    """Undirected drawing groups only: no inferred travel or area assignment."""
    adjacent = defaultdict(set)
    for rid in ids:
        for edge in graph[rid].get('exits', []):
            if edge['to'] in ids and plain_edge(edge):
                adjacent[rid].add(edge['to'])
                adjacent[edge['to']].add(rid)
    seen = set()
    for root in sorted(ids):
        if root in seen:
            continue
        queue = [root]
        seen.add(root)
        for rid in queue:
            for dest in sorted(adjacent[rid] - seen):
                seen.add(dest)
                queue.append(dest)
        yield sorted(queue)
