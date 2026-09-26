import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {followMap, livePosition, mapSource} from '../minimap.mjs';

const hash = 'a'.repeat(64);
const state = (room, session = '1', generation = '1') => ({session, generation, connection: 'connected',
  view: {lifecycle: {kind: 'ready'}, map_location: {map_sha256: hash, room}}});
const tick = () => new Promise(resolve => setImmediate(resolve));

test('only connected, native resolved map IDs qualify; zero is a valid map ID', () => {
  assert.deepEqual(livePosition(state(0)), {hash, room: 0});
  for (const room of [null, undefined, -1, 1.1, 2**32, '228']) assert.ok(livePosition(state(room)).reason);
  const unknown = state(228); delete unknown.view.map_location;
  unknown.view.room = {id: '7000'};
  assert.ok(livePosition(unknown).reason, 'never match game UID in the browser');
  assert.ok(livePosition({...state(228), connection: 'reconnecting'}).reason);
});

test('late loads cannot resurrect a room after movement, disconnect or disposal', async () => {
  const pending = [], shown = [], cleared = [];
  const follower = followMap({load: (position, signal) => new Promise(resolve => pending.push({position, signal, resolve})),
    show: (_, room) => shown.push(room), clear: reason => cleared.push(reason)});
  follower.update(state(228)); await tick();
  follower.update(state(229)); await tick();
  assert.equal(pending[0].signal.aborted, true);
  pending[1].resolve({}); await tick();
  pending[0].resolve({}); await tick();
  assert.deepEqual(shown, [229]);
  follower.update(state(230)); await tick();
  follower.update({...state(230), connection: 'reconnecting'});
  pending[2].resolve({}); await tick();
  assert.deepEqual(shown, [229]);
  assert.match(cleared.at(-1), /connected/);
  follower.update(state(228, '2', '2')); await tick();
  follower.destroy(); pending[3].resolve({}); await tick();
  assert.deepEqual(shown, [229]);
});

test('unchanged vitals do not load again; different characters and generations do', async () => {
  const loaded = [];
  const follower = followMap({load: async p => loaded.push(p.room), show() {}, clear() {}});
  follower.update(state(228)); follower.update(state(228)); await tick();
  assert.equal(loaded.length, 1);
  follower.update(state(228, '2')); await tick();
  follower.update(state(228, '2', '2')); await tick();
  assert.equal(loaded.length, 3);
  follower.destroy();
});

test('snapshot mismatch is explicit and never loads a region', async () => {
  const calls = [];
  const source = mapSource(async url => {
    calls.push(url);
    return {ok: true, json: async () => url.endsWith('manifest.json')
      ? {source_map_sha256: 'b'.repeat(64), regions: []} : []};
  });
  await assert.rejects(source({hash, room: 228}), /versions differ/);
  assert.equal(calls.length, 2);
});

test('only hash-bound region geometry is accepted, and missing rooms stay missing', async () => {
  const text = JSON.stringify({scenes: {}, rooms: {}});
  const manifest = {source_map_sha256: hash, regions: [{slug: 'landing', data_sha256: createHash('sha256').update(text).digest('hex')}]};
  const fetched = [], search = [{id: 228, region: 'landing'}];
  let corrupt = false;
  const fetcher = async url => {
    fetched.push(url);
    return {ok: true, json: async () => url.endsWith('manifest.json') ? manifest : search,
      text: async () => corrupt ? text + ' ' : text};
  };
  const source = mapSource(fetcher);
  await source({hash, room: 228});
  await source({hash, room: 228});
  assert.equal(fetched.length, 3, 'same region cached');
  await assert.rejects(source({hash, room: 229}), /not included/);
  corrupt = true;
  await assert.rejects(mapSource(fetcher)({hash, room: 228}), /integrity/);
});
