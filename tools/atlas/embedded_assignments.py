"""Read ownership from decoded gs.map metadata, never from an older sidecar.

The dotted key is a presentation identity compatible with mapper's plate_key;
it is not an additional assignment. Missing metadata stays missing. Conflicting
metadata and key collisions fail closed rather than silently merging areas.
"""


def area_key(name):
    parts, word = [], ''
    for ch in name.strip().lower():
        if ch.isalnum():
            word += ch
        elif word:
            parts.append(word)
            word = ''
    if word:
        parts.append(word)
    return '.'.join(parts)


def embedded_assignments(graph):
    result, keys = {}, {}
    for rid, room in graph.items():
        if room['id'] != rid:
            raise ValueError(f'Room identity mismatch: {rid}')
        meta = room.get('meta', [])
        if not isinstance(meta, list) or any(not isinstance(m, str) for m in meta):
            raise ValueError(f'Malformed metadata: {rid}')

        def one(prefix):
            values = [m[len(prefix):] for m in meta if m.startswith(prefix)]
            if len(values) > 1 or any(not v or v != v.strip() for v in values):
                raise ValueError(f'Ambiguous or malformed {prefix} metadata: {rid}')
            return values[0] if values else ''

        area, region, status = one('area:'), one('region:'), one('map:status:') or 'live'
        if status not in ('live', 'closed', 'gone'):
            raise ValueError(f'Unknown status: {rid}: {status}')
        if 'map:virtual room' in meta:
            status = 'virtual'
        key = area_key(area)
        if area and (not key or keys.get(key, area) != area):
            raise ValueError(f'Area identity collision: {area}')
        if area:
            keys[key] = area
        result[rid] = dict(room_id=str(rid), uid=str((room.get('uid') or [''])[0]),
            title=(room.get('title') or [''])[0], location=room.get('location') or '',
            area=area, area_key=key, region=region,
            region_src='native-meta' if region else '', status=status)
    return result
