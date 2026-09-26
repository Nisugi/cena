// Read-only character projection. No socket, command API, UID matching or routing.
import {index, title, focusName, transitions} from './model.mjs';
import {cameraDrawing} from './display-layout.mjs';

export function livePosition(state) {
  const value = state?.view?.map_location;
  if (state?.connection !== 'connected' || state?.view?.lifecycle?.kind !== 'ready')
    return {reason: 'Waiting for a connected character.'};
  if (!value) return {reason: 'Native map unavailable. Set CENA_MAP and restart Hydra.'};
  if (!/^[a-f0-9]{64}$/.test(value.map_sha256 || '')) return {reason: 'Invalid map identity.'};
  if (!Number.isInteger(value.room) || value.room < 0 || value.room > 0xffffffff)
    return {reason: 'Location unknown or ambiguous; no position guessed.'};
  return {room: value.room, hash: value.map_sha256};
}

// Bounded cache and latest-request-wins fence. A disconnect invalidates even
// an already completing fetch, so stale rooms cannot resurrect a live marker.
export function followMap({load, show, clear}) {
  let epoch = 0, key = '', disposed = false, controller;
  return {
    update(state) {
      if (disposed) return;
      const position = livePosition(state);
      const next = JSON.stringify([state.session, state.generation, position]);
      if (next === key) return;
      key = next;
      const current = ++epoch;
      controller?.abort();
      controller = new AbortController();
      const signal = controller.signal;
      if (position.reason) { clear(position.reason); return; }
      clear('Locating on the map…');
      Promise.resolve().then(() => load(position, signal)).then(result => {
        if (!disposed && current === epoch) show(result, position.room);
      }).catch(error => {
        if (!disposed && current === epoch) clear(error.message);
      });
    },
    destroy() { disposed = true; ++epoch; controller?.abort(); clear('Map closed.'); },
  };
}

export function mapSource(fetcher = fetch) {
  let catalogue;
  const cached = new Map();
  const json = async (url, signal) => {
    const response = await fetcher(url, {signal});
    if (!response.ok) throw Error('Map data unavailable. Move again or reopen the character page to retry.');
    return response.json();
  };
  return async (position, signal) => {
    if (!catalogue) {
      const [manifest, search] = await Promise.all([
        json('/atlas/corpus/manifest.json', signal), json('/atlas/corpus/search.json', signal),
      ]);
      catalogue = {manifest, search: new Map(search.map(row => [row.id, row]))};
    }
    if (position.hash !== catalogue.manifest.source_map_sha256)
      throw Error('Map versions differ. Rebuild the explorer from this CENA_MAP before following the character.');
    const entry = catalogue.search.get(position.room);
    if (!entry) throw Error('This native room is not included in the explorer snapshot.');
    const region = catalogue.manifest.regions.find(row => row.slug === entry.region);
    if (!region || !/^[a-z0-9-]+$/.test(region.slug)) throw Error('Region unavailable.');
    let bundle = cached.get(region.slug);
    if (!bundle) {
      const response = await fetcher(`/atlas/${region.slug}/data.json`, {signal});
      if (!response.ok) throw Error('Region unavailable.');
      const text = await response.text();
      const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
      const hash = [...new Uint8Array(digest)].map(b => b.toString(16).padStart(2, '0')).join('');
      if (hash !== region.data_sha256) throw Error('Region snapshot failed its integrity check.');
      const data = JSON.parse(text);
      bundle = {data, lookup: index(data), search: catalogue.search, slug: region.slug};
      cached.set(region.slug, bundle);
      if (cached.size > 2) cached.delete(cached.keys().next().value);
    }
    return bundle;
  };
}

export function mountMinimap(root, {fetcher = fetch} = {}) {
  const document = root.ownerDocument;
  const status = root.querySelector('[data-map-status]');
  const svg = root.querySelector('svg');
  const links = root.querySelector('[data-map-transitions]');
  const explorer = root.querySelector('[data-map-explorer]');
  let frame, marker, camera;
  const make = (tag, attrs = {}, text) => {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
    if (text !== undefined) node.textContent = text;
    return node;
  };
  const href = (bundle, id) => {
    const target = bundle.search.get(Number(id));
    return target ? `/atlas/${target.region}/#room=${id}&browse=1` : null;
  };
  const follow = followMap({load: mapSource(fetcher),
    clear(reason) {
      status.textContent = reason;
      // Keep reusable geometry but hide it until a new valid position lands.
      svg.toggleAttribute('hidden', true);
      links.hidden = true;
      explorer.hidden = true;
    },
    show(bundle, id) {
      const {data, lookup} = bundle, room = lookup[id];
      if (!room) throw Error('No native drawing for this room.');
      const scene = data.scenes[room.area];
      const nextFrame = `${bundle.slug}:${room.area}:${room.unit}`;
      if (nextFrame !== frame) {
        frame = nextFrame;
        camera = null;
        svg.replaceChildren();
        links.replaceChildren();
        const exits = transitions(data, lookup, room.area, room.unit);
        const sources = new Set(exits.map(exit => exit.from));
        // Same obstacle-avoidance policy as the explorer. Unroutable drawing
        // edges stay listed as transitions; never fabricate a through-room line.
        const drawing = cameraDrawing(scene, data.rooms, 1.1);
        for (const edge of drawing.sheet.edges) {
          if (!edge.points) continue;
          svg.append(make('polyline', {points: edge.points.map(p => `${p.x},${p.y}`).join(' '),
            class: 'mini-edge', 'vector-effect': 'non-scaling-stroke'}));
        }
        for (const node of scene.sheet.rooms) {
          const active = node.unit === room.unit, destination = href(bundle, node.id);
          const link = make(destination ? 'a' : 'g', destination ? {href: destination, target: '_blank', rel: 'noopener'} : {});
          const label = `${title(data.rooms[node.id])} #${node.id}${sources.has(node.id) ? ' · transition' : ''} · inspect only`;
          link.setAttribute('aria-label', label);
          link.append(make('title', {}, label));
          link.append(make('circle', {cx: node.cell.x, cy: node.cell.y, r: 2.4, class: 'mini-hit'}));
          link.append(make(active ? 'rect' : 'circle', active ? {
            x: node.cell.x - .7, y: node.cell.y - .7, width: 1.4, height: 1.4, class: 'mini-room',
          } : {cx: node.cell.x, cy: node.cell.y, r: .55, class: 'mini-context'}));
          if (sources.has(node.id)) link.append(make('circle', {cx: node.cell.x, cy: node.cell.y,
            r: 1.4, class: 'mini-transition', 'vector-effect': 'non-scaling-stroke'}));
          svg.append(link);
        }
        for (const exit of exits) {
          const li = document.createElement('li'), url = href(bundle, exit.to);
          const label = document.createElement(url ? 'a' : 'span');
          label.textContent = `↗ ${exit.destination} · #${exit.from} → #${exit.to}${url ? '' : ' (not bundled)'}`;
          if (url) { label.href = url; label.target = '_blank'; label.rel = 'noopener'; }
          li.append(label); links.append(li);
        }
        marker = make('circle', {r: 1.8, class: 'mini-current', 'vector-effect': 'non-scaling-stroke'});
        marker.append(make('title', {}, 'Your character · native resolved position'));
        svg.append(marker);
      }
      const {x, y} = room.cell;
      // Dead zone: adjacent steps don't drag the map under the pointer.
      if (!camera || Math.abs(x - camera.x) > 24 || Math.abs(y - camera.y) > 16) camera = {x, y};
      svg.setAttribute('viewBox', `${camera.x - 40} ${camera.y - 28} 80 56`);
      marker.setAttribute('cx', x); marker.setAttribute('cy', y);
      marker.setAttribute('data-current-room', id);
      status.textContent = `${focusName(data, room.area, room.unit)} · ${title(data.rooms[id])} #${id}`;
      explorer.href = href(bundle, id);
      svg.toggleAttribute('hidden', false); links.hidden = false; explorer.hidden = false;
    },
  });
  return follow;
}
