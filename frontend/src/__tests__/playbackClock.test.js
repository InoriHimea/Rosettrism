import test from 'node:test';
import assert from 'node:assert/strict';
import { createFakeClock, createMediaClock, createPreviewClock } from '../playbackClock.js';

test('preview clock derives time from the injected monotonic source without drift', () => {
  let nowMs = 1000;
  let scheduled = null;
  const clock = createPreviewClock({
    durationMs: 600000,
    initialMs: 2500,
    now: () => nowMs,
    requestFrame: (callback) => {
      scheduled = callback;
      return 1;
    },
    cancelFrame: () => {
      scheduled = null;
    },
  });

  clock.play();
  nowMs += 10 * 60 * 1000;
  assert.equal(clock.nowMs(), 600000);
  scheduled?.(nowMs);
  assert.equal(clock.isPlaying(), false);
  assert.equal(clock.nowMs(), 600000);
});

test('preview clock pause, seek, resume, and restart always re-anchor to clock truth', () => {
  let nowMs = 0;
  let scheduled = null;
  const clock = createPreviewClock({
    durationMs: 10000,
    now: () => nowMs,
    requestFrame: (callback) => {
      scheduled = callback;
      return 7;
    },
    cancelFrame: () => {
      scheduled = null;
    },
  });

  clock.play();
  nowMs = 1200;
  assert.equal(clock.nowMs(), 1200);
  clock.pause();
  nowMs = 5000;
  assert.equal(clock.nowMs(), 1200);

  clock.seek(8200);
  clock.play();
  nowMs = 5500;
  assert.equal(clock.nowMs(), 8700);
  clock.seek(10000);
  assert.equal(clock.isPlaying(), false);

  clock.play();
  assert.equal(clock.nowMs(), 0);
  assert.equal(clock.isPlaying(), true);
  scheduled?.(nowMs);
});

test('fake clock is deterministic for frame-state and seek tests', () => {
  const states = [];
  const clock = createFakeClock({ durationMs: 5000, initialMs: 1000 });
  const unsubscribe = clock.subscribe((state) => states.push(state));

  clock.play();
  clock.advance(1250);
  clock.pause();
  clock.advance(500);
  clock.seek(4800);
  clock.play();
  clock.advance(500);

  assert.deepEqual(states.at(-1), { currentMs: 5000, durationMs: 5000, isPlaying: false });
  assert.equal(clock.nowMs(), 5000);
  unsubscribe();
});

test('media clock reads media.currentTime instead of accumulating frame deltas', async () => {
  const listeners = new Map();
  const media = {
    currentTime: 1.25,
    duration: 20,
    paused: true,
    ended: false,
    playbackRate: 1,
    volume: 1,
    muted: false,
    readyState: 4,
    networkState: 1,
    seeking: false,
    error: null,
    addEventListener(name, listener) {
      listeners.set(name, listener);
    },
    removeEventListener(name) {
      listeners.delete(name);
    },
    async play() {
      this.paused = false;
      listeners.get('play')?.();
    },
    pause() {
      this.paused = true;
      listeners.get('pause')?.();
    },
  };
  let frameCallback = null;
  const clock = createMediaClock(media, {
    requestFrame: (callback) => {
      frameCallback = callback;
      return 3;
    },
    cancelFrame: () => {
      frameCallback = null;
    },
  });

  assert.equal(clock.nowMs(), 1250);
  assert.equal(clock.snapshot().playbackRate, 1);
  await clock.play();
  media.currentTime = 7.75;
  frameCallback?.();
  assert.equal(clock.nowMs(), 7750);

  clock.seek(19500);
  assert.equal(media.currentTime, 19.5);
  media.currentTime = 4;
  listeners.get('seeked')?.({ type: 'seeked' });
  assert.equal(clock.nowMs(), 4000);

  listeners.get('waiting')?.({ type: 'waiting' });
  assert.equal(clock.snapshot().isBuffering, true);
  listeners.get('playing')?.({ type: 'playing' });
  assert.equal(clock.snapshot().isBuffering, false);

  clock.setPlaybackRate(1.5);
  clock.setVolume(0.35);
  clock.setMuted(true);
  assert.equal(media.playbackRate, 1.5);
  assert.equal(media.volume, 0.35);
  assert.equal(media.muted, true);
  assert.deepEqual(
    {
      playbackRate: clock.snapshot().playbackRate,
      volume: clock.snapshot().volume,
      muted: clock.snapshot().muted,
    },
    { playbackRate: 1.5, volume: 0.35, muted: true },
  );
  clock.destroy();
  assert.equal(listeners.size, 0);
});
