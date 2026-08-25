import test from 'node:test';
import assert from 'node:assert/strict';
import {
  createPlayerSession,
  createPlayerSessionState,
  persistPlayerSession,
  reconcilePlayerSession,
  reducePlayerSession,
  restorePlayerSession,
  serializePlayerSession,
} from '../playerSession.js';

const queue = [
  { id: 'one', src: '/one.wav', title: 'One', artist: 'Artist', durable: true },
  { id: 'two', src: '/two.wav', title: 'Two', artist: 'Artist', durable: true },
  { id: 'three', src: '/three.wav', title: 'Three', artist: 'Artist', durable: true },
];

test('session normalizes queue and selects tracks by stable id', () => {
  const state = createPlayerSessionState({
    queue: [...queue, queue[0], null, { id: 'missing' }],
    currentId: 'two',
    volume: 3,
    playbackRate: 0,
  });

  assert.equal(state.queue.length, 3);
  assert.equal(state.currentIndex, 1);
  assert.equal(state.currentId, 'two');
  assert.equal(state.volume, 1);
  assert.equal(state.playbackRate, 0.25);
});

test('order and repeat-all modes have explicit queue boundaries', () => {
  const order = createPlayerSessionState({ queue, currentId: 'three', mode: 'order', isPlaying: true });
  const stopped = reducePlayerSession(order, { type: 'ended' });
  assert.equal(stopped.currentId, 'three');
  assert.equal(stopped.status, 'ended');
  assert.equal(stopped.isPlaying, false);

  const repeatAll = createPlayerSessionState({ queue, currentId: 'three', mode: 'repeat-all' });
  const wrapped = reducePlayerSession(repeatAll, { type: 'next' });
  assert.equal(wrapped.currentId, 'one');
  assert.equal(wrapped.isPlaying, true);

  const previous = reducePlayerSession({ ...repeatAll, currentIndex: 0, currentId: 'one' }, { type: 'previous' });
  assert.equal(previous.currentId, 'three');
});

test('repeat-one restarts the same track and shuffle never immediately repeats', () => {
  const repeatOne = createPlayerSessionState({ queue, currentId: 'two', mode: 'repeat-one' });
  const restarted = reducePlayerSession({ ...repeatOne, positionMs: 4200 }, { type: 'ended' });
  assert.equal(restarted.currentId, 'two');
  assert.equal(restarted.positionMs, 0);
  assert.equal(restarted.isPlaying, true);

  const shuffled = reducePlayerSession(
    createPlayerSessionState({ queue, currentId: 'two', mode: 'shuffle' }),
    { type: 'next' },
    { random: () => 0 },
  );
  assert.equal(shuffled.currentId, 'one');
});

test('volume mute rate position and retry remain serializable product state', () => {
  let state = createPlayerSessionState({ queue });
  state = reducePlayerSession(state, { type: 'set-volume', volume: 0.4 });
  state = reducePlayerSession(state, { type: 'set-muted', muted: true });
  state = reducePlayerSession(state, { type: 'set-rate', playbackRate: 1.5 });
  state = reducePlayerSession(state, { type: 'set-position', positionMs: 3200 });
  state = reducePlayerSession(state, { type: 'media-error', error: 'MEDIA_ERR_NETWORK' });

  assert.deepEqual(
    {
      volume: state.volume,
      muted: state.muted,
      playbackRate: state.playbackRate,
      positionMs: state.positionMs,
      status: state.status,
      retryCount: state.retryCount,
    },
    { volume: 0.4, muted: true, playbackRate: 1.5, positionMs: 3200, status: 'error', retryCount: 1 },
  );
  assert.equal(reducePlayerSession(state, { type: 'retry' }).error, null);
});

test('persistence drops temporary and expired sources and restores fresh durable state paused', () => {
  const now = 1_900_000_000_000;
  const state = createPlayerSessionState({
    queue: [
      queue[0],
      { id: 'temporary', src: '/signed.wav?token=secret', title: 'Temporary', durable: false },
      { id: 'expired', src: '/expired.wav', title: 'Expired', expiresAt: now - 1 },
      { id: 'fresh', src: '/fresh.wav', title: 'Fresh', durable: true, expiresAt: now + 10_000 },
    ],
    currentId: 'temporary',
    mode: 'repeat-all',
    positionMs: 4500,
    volume: 0.35,
    muted: true,
    playbackRate: 1.5,
    isPlaying: true,
  });
  const restored = restorePlayerSession(serializePlayerSession(state, { now: () => now }), { now: () => now + 100 });

  assert.deepEqual(restored.queue.map((item) => item.id), ['one', 'fresh']);
  assert.equal(restored.currentId, 'one');
  assert.equal(restored.positionMs, 4500);
  assert.equal(restored.isPlaying, false);
  assert.equal(restored.volume, 0.35);
  assert.equal(restored.muted, true);
  assert.equal(restored.playbackRate, 1.5);
});

test('restored preferences are reconciled against current caller-owned queue sources', () => {
  const restored = createPlayerSessionState({
    queue,
    currentId: 'two',
    positionMs: 3200,
    volume: 0.3,
    muted: true,
    playbackRate: 1.5,
  });
  const currentQueue = [
    { id: 'two', src: '/two-new.wav', title: 'Two new', durable: true },
    { id: 'four', src: '/four.wav', title: 'Four', durable: false },
  ];
  const reconciled = reconcilePlayerSession(restored, currentQueue);

  assert.equal(reconciled.currentId, 'two');
  assert.equal(reconciled.queue[0].src, '/two-new.wav');
  assert.equal(reconciled.queue[1].id, 'four');
  assert.equal(reconciled.positionMs, 3200);
  assert.equal(reconciled.isPlaying, false);
});

test('queue items are not persisted unless durable is explicitly true', () => {
  const now = 1_900_000_000_000;
  const state = createPlayerSessionState({
    queue: [{ id: 'implicit', src: '/possibly-signed.wav?token=secret', title: 'Implicit' }],
  });
  const restored = restorePlayerSession(serializePlayerSession(state, { now: () => now }), { now: () => now });
  assert.equal(restored.queue.length, 0);
});

test('stale, future and malformed persisted sessions fail closed', () => {
  const now = 1_900_000_000_000;
  const serialized = serializePlayerSession(createPlayerSessionState({ queue }), { now: () => now });

  assert.equal(restorePlayerSession(serialized, { now: () => now + 1000, maxAgeMs: 500 }).queue.length, 0);
  assert.equal(restorePlayerSession(serialized, { now: () => now - 1 }).queue.length, 0);
  assert.equal(restorePlayerSession('{broken').queue.length, 0);
});

test('imperative session notifies subscribers and supports deterministic shuffle', () => {
  const session = createPlayerSession({ queue, currentId: 'two', mode: 'shuffle', random: () => 0.9 });
  const seen = [];
  const unsubscribe = session.subscribe((state) => seen.push(state.currentId));

  session.next();
  session.setVolume(0.2);
  unsubscribe();
  session.previous();

  assert.deepEqual(seen, ['two', 'three', 'three']);
  session.destroy();
});

test('persist helper tolerates storage failures', () => {
  const state = createPlayerSessionState({ queue });
  assert.doesNotThrow(() => persistPlayerSession(state, {
    storage: { setItem() { throw new Error('blocked'); } },
  }));
});
