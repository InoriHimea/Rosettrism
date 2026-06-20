import test from 'node:test';
import assert from 'node:assert/strict';
import {
  annotationLabelState,
  buildIntroMetaLines,
  formatArtistLabel,
  karaokeLaneItems,
  karaokePlaceholderLanes,
  lyricCountdown,
  lyricLineProgress,
  shouldReserveBreathGap,
  translationModeLabel,
  visibleWordAnnotations,
  wordProgress,
} from '../lyricPlaybackViewModel.js';

test('artist and translation labels keep readable multilingual fallbacks', () => {
  assert.equal(formatArtistLabel('いきものがかり', 'Ikimonogakari'), 'いきものがかり（Ikimonogakari）');
  assert.equal(translationModeLabel('translation', {}), '译文');
  assert.equal(translationModeLabel('bilingual', {}), '双语');
  assert.equal(translationModeLabel('off', {}), '原文');
});

test('intro metadata deduplicates title-like lines and fits before first lyric', () => {
  const lines = [
    { id: 'meta-title-1', text: '海阔天空 - Beyond', startMs: 0, endMs: 1200 },
    { id: 'credit-1', text: '作词：黄家驹', startMs: 1200, endMs: 2400 },
    { id: 'duplicate-title', text: '海阔天空（Beyond）', startMs: 2400, endMs: 3600 },
  ];

  const intro = buildIntroMetaLines(lines, 7200);

  assert.deepEqual(intro.map((line) => line.text), ['海阔天空 - Beyond', '作词：黄家驹']);
  assert.equal(intro[0].startMs, 0);
  assert.equal(intro[1].endMs <= 6000, true);
  assert.equal(intro.every((line) => line.isMeta && line.words.length === 0), true);
});

test('karaoke lanes keep two alternating rows and countdown on target lane', () => {
  const bodyLines = [
    { id: 'a', startMs: 1000, endMs: 2400 },
    { id: 'b', startMs: 3000, endMs: 4400 },
    { id: 'c', startMs: 10000, endMs: 12000 },
  ];

  const lanes = karaokeLaneItems(bodyLines, bodyLines[1], 1, false, {});
  assert.deepEqual(lanes.map((item) => item.lanePositionClass), ['lyric-karaoke-lane-top', 'lyric-karaoke-lane-bottom']);
  assert.deepEqual(karaokePlaceholderLanes(lanes), []);

  const countdown = karaokeLaneItems(bodyLines, null, 1, true, { targetLineId: 'c' });
  assert.equal(countdown[0].kind, 'countdown');
  assert.equal(countdown[0].lanePositionClass, 'lyric-karaoke-lane-top');
  assert.equal(countdown[0].targetLine.id, 'c');
});

test('karaoke title metadata is rendered once and body lines keep one row', () => {
  const lines = [
    { id: 'meta-title-1', text: 'Demo Title - Artist', startMs: 0, endMs: 1000, words: [] },
    { id: 'credit-1', text: 'Lyricist: Sample', startMs: 1000, endMs: 2000, words: [] },
    { id: 'credit-2', text: 'Composer: Sample', startMs: 2000, endMs: 3000, words: [] },
    { id: 'body-1', text: 'first line', startMs: 3000, endMs: 5000, words: [{ text: 'first' }, { text: 'line' }] },
  ];

  const intro = buildIntroMetaLines(lines.slice(0, 3), 6000);
  assert.equal(intro.length, 3);
  assert.equal(intro[0].text, 'Demo Title - Artist');
  assert.equal(intro[1].text, 'Lyricist: Sample');
  assert.equal(intro[2].text, 'Composer: Sample');
  assert.equal(intro.every((line) => line.isMeta), true);
  assert.equal(intro.every((line) => line.words.length === 0), true);
});

test('countdown distinguishes metadata intro, short gaps, interludes, and exiting bubble', () => {
  const lines = [
    { id: 'first', startMs: 7200, endMs: 9000 },
    { id: 'short', startMs: 10400, endMs: 11500 },
    { id: 'after-gap', startMs: 18000, endMs: 20000 },
  ];

  assert.equal(lyricCountdown(lines, 5000, { introMetaEndMs: 4800 }).visible, true);
  assert.equal(lyricCountdown(lines, 9900).visible, false);
  assert.deepEqual(
    pickCountdownFields(lyricCountdown(lines, 15000)),
    { count: 3, kind: 'interlude', targetLineId: 'after-gap', visible: true },
  );
  assert.equal(lyricCountdown(lines, 18020).exiting, true);
});

test('annotation label selection suppresses duplicates by anchor and priority', () => {
  const state = annotationLabelState([
    { id: 'breath', type: 'breath', anchorPercent: 10 },
    { id: 'stress', type: 'stress', anchorPercent: 11 },
    { id: 'hidden', type: 'long_tone', anchorPercent: 80, suppressLabel: true },
    { id: 'long', type: 'long_tone', anchorPercent: 82 },
  ]);

  assert.equal(state.ids.has('breath'), true);
  assert.equal(state.ids.has('stress'), false);
  assert.equal(state.ids.has('hidden'), false);
  assert.equal(state.ids.has('long'), true);
  assert.equal(state.rows.get('breath'), 0);
  assert.equal(state.rows.get('long'), 1);
});

test('word annotation visibility, breath spacing, and progress are deterministic', () => {
  const shared = { id: 'same-marker', type: 'stress', anchorPercent: 50 };
  const words = [
    { text: 'Just', startMs: 1000, endMs: 1500, annotations: [shared] },
    { text: 'dance', startMs: 1500, endMs: 2500, annotations: [shared, { id: 'breath', type: 'breath', anchorPercent: 5 }] },
  ];

  assert.deepEqual(visibleWordAnnotations(words, words[1], 1).map((item) => item.id), ['breath']);
  assert.equal(shouldReserveBreathGap(words, 1, words[1].annotations), true);
  assert.equal(wordProgress(words[1], 2000), 0.5);
  assert.equal(lyricLineProgress({ startMs: 1000, endMs: 3000 }, 2500), 0.75);
});

function pickCountdownFields(countdown) {
  const { count, kind, targetLineId, visible } = countdown;
  return { count, kind, targetLineId, visible };
}
