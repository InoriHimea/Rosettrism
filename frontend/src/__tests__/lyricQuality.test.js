import test from 'node:test';
import assert from 'node:assert/strict';
import { assessLyricQuality } from '../lyricQuality.js';
import { normalizeLyricPayload } from '../lyricPlayback.js';

function line(overrides = {}) {
  return {
    startMs: 1000,
    timingExplicit: true,
    durationMs: 2000,
    endMs: 3000,
    text: '测试歌词',
    isMeta: false,
    words: [],
    ruby: [],
    ...overrides,
  };
}

test('quality contract distinguishes word, line, and unsynced timing', () => {
  const wordTimed = assessLyricQuality({
    lines: [line({
      words: [{ startMs: 1000, durationMs: 600, endMs: 1600, text: '测' }],
    })],
  });
  assert.equal(wordTimed.timingLevel, 'word_timed');
  assert.equal(wordTimed.playable, true);
  assert.equal(wordTimed.capabilities.wordTiming, true);

  const lineTimed = assessLyricQuality({ lines: [line()] });
  assert.equal(lineTimed.timingLevel, 'line_timed');
  assert.equal(lineTimed.playable, true);
  assert.equal(lineTimed.capabilities.wordTiming, false);
  assert.ok(lineTimed.diagnostics.some(
    (item) => item.code === 'WORD_TIMING_UNAVAILABLE',
  ));

  const unsynced = assessLyricQuality({ raw: '没有时间戳的歌词' });
  assert.equal(unsynced.timingLevel, 'unsynced');
  assert.equal(unsynced.playable, false);
  assert.deepEqual(unsynced.degradationReasons, ['UNSYNCED_RAW_TEXT']);
});

test('missing explicit line timing cannot be normalized into a fake 0ms timeline', () => {
  const lyric = normalizeLyricPayload({
    document: {
      meta: { title: '无时间歌词' },
      lines: [{ text: '这句没有 start_ms', words: [] }],
    },
  });

  assert.equal(lyric.playable, false);
  assert.equal(lyric.quality.timingLevel, 'invalid');
  assert.equal(lyric.quality.capabilities.synced, false);
  assert.ok(lyric.quality.diagnostics.some(
    (item) => item.code === 'LINE_TIMING_MISSING' && item.lineIndex === 0,
  ));
});

test('quality diagnostics expose line, word, and annotation timing defects', () => {
  const quality = assessLyricQuality({
    lines: [
      line({
        startMs: 2000,
        endMs: 4000,
        words: [
          { startMs: 2200, durationMs: 700, endMs: 2900, text: '先' },
          { startMs: 2100, durationMs: 0, endMs: 2100, text: '后' },
          { startMs: 3900, durationMs: 300, endMs: 4200, text: '界' },
        ],
      }),
      line({ startMs: 1500, endMs: 2500 }),
    ],
    annotations: [
      { startMs: 8000, durationMs: 500, endMs: 8500 },
    ],
  });
  const codes = new Set(quality.diagnostics.map((item) => item.code));

  assert.equal(quality.timingLevel, 'invalid');
  assert.equal(quality.playable, false);
  assert.ok(codes.has('LINE_TIMING_OUT_OF_ORDER'));
  assert.ok(codes.has('WORD_TIMING_OUT_OF_ORDER'));
  assert.ok(codes.has('WORD_DURATION_INVALID'));
  assert.ok(codes.has('WORD_TIMING_OUTSIDE_LINE'));
  assert.ok(codes.has('ANNOTATION_TIMING_OUTSIDE_LYRIC'));
});

test('translation, reading, ruby, and annotation capabilities are explicit', () => {
  const quality = assessLyricQuality({
    lines: [line({
      translation: 'Translation',
      reading: 'reading',
      ruby: [{ startChar: 0, endChar: 1, reading: 'test' }],
    })],
    annotations: [{ startMs: 1200, endMs: 1400 }],
  });

  assert.deepEqual(
    {
      translation: quality.capabilities.translation,
      reading: quality.capabilities.reading,
      ruby: quality.capabilities.ruby,
      annotations: quality.capabilities.annotations,
    },
    {
      translation: true,
      reading: true,
      ruby: true,
      annotations: true,
    },
  );
});
