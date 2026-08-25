import fs from 'node:fs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeLyricPayload } from '../lyricPlayback.js';
import { buildIntroMetaLines, buildPlaybackFrameState, lyricLineProgress } from '../lyricPlaybackViewModel.js';

const realPayload = JSON.parse(fs.readFileSync(new URL('../../dragon-knight-real.json', import.meta.url), 'utf8'));

test('real Dragon Knight unified QQ/QRC payload preserves word timing and singing annotations', () => {
  const lyric = normalizeLyricPayload(realPayload);
  const bodyLines = lyric.lines.filter((line) => !line.isMeta);
  const metaLines = lyric.lines.filter((line) => line.isMeta);
  const firstBody = bodyLines[0];
  const introMetaLines = buildIntroMetaLines(metaLines, firstBody.startMs);

  assert.equal(lyric.playable, true);
  assert.equal(lyric.quality.timingLevel, 'word_timed');
  assert.equal(lyric.capabilities.wordTiming, true);
  assert.equal(lyric.capabilities.annotations, true);
  assert.equal(lyric.lines.length, 52);
  assert.equal(bodyLines.length, 48);
  assert.equal(lyric.annotations.length, 86);
  assert.equal(firstBody.text, '放手一搏令谁都惭愧');
  assert.equal(firstBody.words.length, 9);
  assert.equal(firstBody.annotations[0].type, 'breath');

  const frame = buildPlaybackFrameState({
    bodyLines,
    introMetaLines,
    currentMs: firstBody.startMs + 350,
    durationMs: lyric.durationMs,
    introMetaEndMs: introMetaLines.at(-1)?.endMs || 0,
  });
  assert.equal(frame.phase, 'singing');
  assert.equal(frame.activeBodyLine.id, firstBody.id);
  assert.ok(lyricLineProgress(firstBody, firstBody.startMs + 350) > 0);
});

test('line-timed lyrics without word timings remain playable with whole-line progress', () => {
  const lyric = normalizeLyricPayload({
    document: {
      meta: { title: '普通逐行歌词', artist: '测试歌手', input_format: 'lrc' },
      lines: [
        { start_ms: 1000, duration_ms: 2500, text: '第一句普通逐行歌词', words: [] },
        { start_ms: 4000, duration_ms: 2500, text: '第二句没有逐字时间', words: [] },
      ],
    },
  });
  assert.equal(lyric.playable, true);
  assert.equal(lyric.quality.timingLevel, 'line_timed');
  assert.equal(lyric.capabilities.lineTiming, true);
  assert.equal(lyric.capabilities.wordTiming, false);
  assert.equal(lyric.lines.some((line) => line.words.length > 0), false);
  assert.ok(lyricLineProgress(lyric.lines[0], 1800) > 0);
  assert.ok(lyricLineProgress(lyric.lines[0], 1800) < 1);
});

test('raw lyric text without timing is explicitly non-playable', () => {
  const lyric = normalizeLyricPayload({ format: 'raw', raw: '没有时间标签的歌词' });
  assert.equal(lyric.playable, false);
  assert.equal(lyric.quality.timingLevel, 'unsynced');
  assert.deepEqual(lyric.quality.degradationReasons, ['UNSYNCED_RAW_TEXT']);
  assert.equal(lyric.raw, '没有时间标签的歌词');
  assert.deepEqual(lyric.lines, []);
});
