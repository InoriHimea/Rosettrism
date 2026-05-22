import { chromium } from '@playwright/test';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const BASE = 'http://127.0.0.1:5181';
const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const MOCK_SEARCH = {
  results: [{
    id: 'test-001',
    title: '超长标题测试别怕我伤心雨一直下海阔天空',
    artist: 'Beyond',
    source: 'qq',
    extra: { artist_alias: 'Beyond', has_singing_annotations: true },
  }],
  warnings: [],
};
const unified = {
  mode: 'word',
  meta: { title: '超长标题测试别怕我伤心雨一直下海阔天空', artist: 'Beyond' },
  inline_lines: [
    { start_ms: 0, duration_ms: 800, text: '超长标题测试别怕我伤心雨一直下海阔天空 - Beyond', words: [] },
    { start_ms: 1000, duration_ms: 1000, text: '作词：黄家驹', words: [] },
    { start_ms: 2000, duration_ms: 1000, text: '作曲：黄家驹', words: [] },
    {
      start_ms: 7200,
      duration_ms: 4200,
      text: '海 阔 天 空',
      extra: { cantonese_romanization: 'hoi fut tin hung' },
      words: [
        { text: '海', offset_ms: 0, duration_ms: 650 },
        { text: '阔', offset_ms: 650, duration_ms: 650 },
        { text: '天', offset_ms: 1300, duration_ms: 650 },
        { text: '空', offset_ms: 1950, duration_ms: 650 },
      ],
    },
    { start_ms: 10500, duration_ms: 1200, text: '短句', words: [{ text: '短句', offset_ms: 0, duration_ms: 900 }] },
    { start_ms: 20000, duration_ms: 1600, text: 'next', words: [{ text: 'next', offset_ms: 0, duration_ms: 1000 }] },
  ],
};
const singingAnnotations = [
  { annotation_type: 'breath', start_ms: 7350, duration_ms: 300, text: '换气' },
  { annotation_type: 'stress', start_ms: 8050, duration_ms: 400, text: '阔' },
  { annotation_type: 'long_tone', start_ms: 8700, duration_ms: 600, text: '天' },
];
const MOCK_FETCH_RESULT_WITH_ANNOTATIONS = {
  unified,
  selectedEntry: {
    id: 'test-001',
    title: '超长标题测试别怕我伤心雨一直下海阔天空',
    artist: 'Beyond',
    extra: {
      artist_alias: 'Beyond',
      singing_annotations: singingAnnotations,
    },
  },
};
const MOCK_FETCH_RESULT_WITHOUT_ANNOTATIONS = {
  unified,
  selectedEntry: {
    id: 'test-001',
    title: '超长标题测试别怕我伤心雨一直下海阔天空',
    artist: 'Beyond',
    extra: { artist_alias: 'Beyond' },
  },
};

let fetchResultCount = 0;
const browser = await chromium.launch({ headless: false });
const page = await browser.newPage({ viewport: { width: 1280, height: 920 } });
await page.route('**/api/**', (route) => {
  const path = new URL(route.request().url()).pathname;
  if (path === '/api/search') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_SEARCH) });
  if (path === '/api/fetch-result') {
    fetchResultCount += 1;
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(fetchResultCount === 1 ? MOCK_FETCH_RESULT_WITH_ANNOTATIONS : MOCK_FETCH_RESULT_WITHOUT_ANNOTATIONS),
    });
  }
  if (path === '/api/cache') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ entries: [] }) });
  if (path === '/api/health') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, version: 'test' }) });
  if (path === '/api/stats') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ total: 0, fresh: 0, expired: 0 }) });
  return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
});

function requirePass(condition, message, sample = {}) {
  if (!condition) {
    throw new Error(`${message} ${JSON.stringify(sample)}`);
  }
}

function formatMs(ms) {
  const totalSeconds = Math.floor(ms / 1000);
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, '0')}`;
}

function compactBox(box) {
  return box && { x: Math.round(box.x), y: Math.round(box.y), w: Math.round(box.width), h: Math.round(box.height) };
}

await page.goto(BASE);
await page.locator('nav button').nth(1).click();
await page.locator('.primary-search input').first().fill('海阔天空');
await page.locator('button[type="submit"]').first().click();
await page.locator('.result-card').first().click();
await page.locator('.dialog-actions button').nth(1).click();
await page.locator('.lyric-playback-card').waitFor({ timeout: 5000 });
await page.locator('.dialog-actions button').nth(1).click();
await page.waitForTimeout(500);
requirePass(fetchResultCount === 2, 'FETCH_RESULT_NOT_REPEATED', { fetchResultCount });

const seekInput = page.locator('.result-dialog input[type="range"]').first();

async function seek(ms) {
  const expectedTime = formatMs(ms);
  await seekInput.evaluate((input, value) => {
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
    setter.call(input, String(value));
    input.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertReplacementText', data: String(value) }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }, ms);
  await page.waitForFunction((expectedTime) => {
    const text = document.querySelector('.result-dialog .lyric-time')?.textContent?.trim() || '';
    return text.startsWith(`${expectedTime} /`);
  }, expectedTime, { timeout: 2400 });
  await page.waitForTimeout(180);
}

async function activeTextSample(ms) {
  await seek(ms);
  return page.evaluate(() => ({
    active: [...document.querySelectorAll('.lyric-line-active')].map((node) => ({ text: node.textContent.trim(), className: node.className })),
    countdownRows: [...document.querySelectorAll('.lyric-line-countdown')].map((node) => ({ text: node.textContent.trim(), className: node.className })),
    countdownCount: document.querySelectorAll('.lyric-line-countdown .lyric-gap-dot:not(.lyric-gap-dot-hidden)').length,
    exitingCount: document.querySelectorAll('.lyric-line-countdown.lyric-dots-exiting').length,
  }));
}

async function activeMetaSample(ms) {
  await seek(ms);
  const activeCount = await page.locator('.lyric-line-active').count();
  if (!activeCount) {
    const debug = await page.evaluate(() => ({
      time: document.querySelector('.result-dialog .lyric-time')?.textContent?.trim() || null,
      lineCount: document.querySelectorAll('.lyric-line').length,
      lines: [...document.querySelectorAll('.lyric-line')].slice(0, 8).map((node) => ({ text: node.textContent.trim(), className: node.className })),
      dialog: Boolean(document.querySelector('.result-dialog')),
    }));
    throw new Error(`NO_ACTIVE ${JSON.stringify(debug)}`);
  }
  return page.locator('.lyric-line-active').first().evaluate((node, ms) => {
    const rect = node.getBoundingClientRect();
    const before = getComputedStyle(node, '::before');
    return {
      ms,
      text: node.textContent.trim(),
      className: node.className,
      height: Math.round(rect.height),
      scrollWidth: node.scrollWidth,
      clientWidth: node.clientWidth,
      whiteSpace: getComputedStyle(node).whiteSpace,
      beforeWidth: before.width,
      beforeContent: before.content,
    };
  }, ms);
}

const metaSamples = [];
for (const ms of [200, 2200, 4200]) {
  metaSamples.push(await activeMetaSample(ms));
}
for (const sample of metaSamples) {
  requirePass(sample.whiteSpace === 'nowrap', 'META_LINE_WRAPPED', sample);
  requirePass(sample.beforeWidth !== 'auto' && sample.beforeWidth !== '0px', 'META_HIGHLIGHT_BAR_MISSING', sample);
  requirePass(sample.height <= 72, 'META_LINE_TOO_TALL', sample);
}
const titleOccurrences = await page.locator('.lyric-line-meta').evaluateAll((nodes) => nodes
  .map((node) => node.textContent.trim())
  .filter((text) => text.includes('超长标题测试别怕我伤心雨一直下海阔天空')).length);
requirePass(titleOccurrences === 1, 'TITLE_RENDERED_MORE_THAN_ONCE', { titleOccurrences });

const introCountdownSample = await activeTextSample(200);
requirePass(introCountdownSample.countdownCount === 3, 'INTRO_COUNTDOWN_NOT_VISIBLE_WITH_TITLE', introCountdownSample);
const countdownSample = await activeTextSample(6200);
requirePass(countdownSample.countdownCount === 1, 'COUNTDOWN_AFTER_META_WRONG', countdownSample);
const shortGapSample = await activeTextSample(10200);
requirePass(shortGapSample.countdownCount === 0, 'SHORT_GAP_SHOULD_NOT_SHOW_COUNTDOWN', shortGapSample);
const interludeSample = await activeTextSample(17000);
requirePass(interludeSample.countdownCount === 3, 'INTERLUDE_COUNTDOWN_MISSING', interludeSample);
const bubbleSample = await activeTextSample(20040);
requirePass(bubbleSample.countdownCount === 1 && bubbleSample.exitingCount === 1, 'COUNTDOWN_BUBBLE_OUT_MISSING', bubbleSample);

await seek(8200);
const annotationCounts = await page.evaluate(() => ({
  stress: document.querySelectorAll('.lyric-line-active .annotation-stress').length,
  longTone: document.querySelectorAll('.lyric-line-active .annotation-long-tone').length,
  breath: document.querySelectorAll('.lyric-line-active .annotation-breath').length,
  reading: [...document.querySelectorAll('.lyric-line-active small')].map((node) => node.textContent.trim()),
  rawJsonHasAnnotations: document.querySelector('.result-dialog .result-preview')?.textContent?.includes('singing_annotations') || false,
}));
requirePass(annotationCounts.stress === 1, 'STRESS_ANNOTATION_LOST_AFTER_REPEAT_FETCH', annotationCounts);
requirePass(annotationCounts.longTone === 1, 'LONG_TONE_ANNOTATION_LOST_AFTER_REPEAT_FETCH', annotationCounts);
requirePass(annotationCounts.breath === 1, 'BREATH_ANNOTATION_LOST_AFTER_REPEAT_FETCH', annotationCounts);
requirePass(annotationCounts.reading.some((text) => text.includes('hoi fut tin hung')), 'TRANSLITERATION_MISSING', annotationCounts);
requirePass(annotationCounts.rawJsonHasAnnotations, 'RAW_JSON_DID_NOT_PRESERVE_ANNOTATIONS', annotationCounts);

const wordBoxes = await page.locator('.lyric-line-active .lyric-word').evaluateAll((nodes) => nodes.map((node, index) => {
  const rect = node.getBoundingClientRect();
  const text = node.querySelector('.lyric-progress-base')?.textContent?.trim() || node.textContent.trim();
  return { index, text, x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) };
}));
const stressGlyph = compactBox(await page.locator('.lyric-line-active .annotation-stress .annotation-glyph-text').first().boundingBox());
const stressLabel = compactBox(await page.locator('.lyric-line-active .annotation-stress .lyric-annotation-label').first().boundingBox());
const longToneGlyph = compactBox(await page.locator('.lyric-line-active .annotation-long-tone .annotation-glyph-text').first().boundingBox());
const longToneLabel = compactBox(await page.locator('.lyric-line-active .annotation-long-tone .lyric-annotation-label').first().boundingBox());
const breathGlyph = compactBox(await page.locator('.lyric-line-active .annotation-breath .annotation-glyph-text').first().boundingBox());
const breathLabel = compactBox(await page.locator('.lyric-line-active .annotation-breath .lyric-annotation-label').first().boundingBox());
const stressWord = wordBoxes.find((word) => word.text.includes('阔'));
const longToneWord = wordBoxes.find((word) => word.text.includes('天'));
const breathWord = wordBoxes.find((word) => word.text.includes('海'));
const stressSample = { word: stressWord, glyph: stressGlyph, label: stressLabel };
const longToneSample = { word: longToneWord, glyph: longToneGlyph, label: longToneLabel };
const breathSample = { word: breathWord, glyph: breathGlyph, label: breathLabel };
requirePass(stressWord && stressGlyph && stressLabel, 'STRESS_BOUNDS_MISSING', stressSample);
requirePass(longToneWord && longToneGlyph && longToneLabel, 'LONG_TONE_BOUNDS_MISSING', longToneSample);
requirePass(breathWord && breathGlyph && breathLabel, 'BREATH_BOUNDS_MISSING', breathSample);
requirePass(stressGlyph.y + stressGlyph.h / 2 >= stressWord.y + stressWord.h * 0.60, 'STRESS_DOT_NOT_BELOW_TEXT', stressSample);
requirePass(stressGlyph.y - stressWord.y <= stressWord.h * 0.80, 'STRESS_DOT_TOO_FAR_BELOW_TEXT', stressSample);
requirePass(longToneGlyph.y + longToneGlyph.h / 2 >= longToneWord.y + longToneWord.h * 0.60, 'LONG_TONE_UNDERSCORE_NOT_BELOW_TEXT', longToneSample);
requirePass(longToneGlyph.y - longToneWord.y <= longToneWord.h * 0.80, 'LONG_TONE_UNDERSCORE_TOO_FAR_BELOW_TEXT', longToneSample);
requirePass(stressLabel.y + stressLabel.h <= stressWord.y + 4, 'STRESS_LABEL_NOT_ABOVE_TEXT', stressSample);
requirePass(longToneLabel.y + longToneLabel.h <= longToneWord.y + 4, 'LONG_TONE_LABEL_NOT_ABOVE_TEXT', longToneSample);
requirePass(breathLabel.y + breathLabel.h <= breathWord.y + 6, 'BREATH_LABEL_NOT_ABOVE_TEXT', breathSample);
requirePass(Math.abs((stressGlyph.x + stressGlyph.w / 2) - (stressWord.x + stressWord.w / 2)) <= 8, 'STRESS_DOT_NOT_CENTERED', stressSample);
requirePass(Math.abs((longToneGlyph.x + longToneGlyph.w / 2) - (longToneWord.x + longToneWord.w / 2)) <= 10, 'LONG_TONE_UNDERSCORE_NOT_CENTERED', longToneSample);

console.log('META_SAMPLES:', JSON.stringify(metaSamples));
console.log('INTRO_COUNTDOWN:', JSON.stringify(introCountdownSample));
console.log('COUNTDOWN_AFTER_META:', JSON.stringify(countdownSample));
console.log('SHORT_GAP_SAMPLE:', JSON.stringify(shortGapSample));
console.log('INTERLUDE_COUNTDOWN:', JSON.stringify(interludeSample));
console.log('BUBBLE_SAMPLE:', JSON.stringify(bubbleSample));
console.log('ANNOTATION_COUNTS_AFTER_REPEAT_FETCH:', JSON.stringify(annotationCounts));
console.log('STRESS_SAMPLE:', JSON.stringify(stressSample));
console.log('LONG_TONE_SAMPLE:', JSON.stringify(longToneSample));
console.log('BREATH_SAMPLE:', JSON.stringify(breathSample));
await page.screenshot({ path: resolve(SCRIPT_DIR, 'playwright-artifacts/verify-meta-stress.png'), fullPage: false });
await browser.close();
