import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { normalizeLyricPayload } from './src/lyricPlayback.js';

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
    {
      start_ms: 1000,
      duration_ms: 1000,
      text: '作词：黄家驹',
      words: [
        { text: '作词', offset_ms: 0, duration_ms: 260 },
        { text: '黄家驹', offset_ms: 360, duration_ms: 420 },
      ],
    },
    {
      start_ms: 2000,
      duration_ms: 1000,
      text: '作曲：黄家驹',
      words: [
        { text: '作曲', offset_ms: 0, duration_ms: 260 },
        { text: '黄家驹', offset_ms: 360, duration_ms: 420 },
      ],
    },
    {
      start_ms: 3000,
      duration_ms: 1000,
      text: '编曲：钟兴民',
      words: [
        { text: '编曲', offset_ms: 0, duration_ms: 260 },
        { text: '钟兴民', offset_ms: 360, duration_ms: 420 },
      ],
    },
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
    {
      start_ms: 12600,
      duration_ms: 2200,
      text: 'a b xy mn',
      words: [
        { text: 'a', offset_ms: 0, duration_ms: 650 },
        { text: 'b', offset_ms: 650, duration_ms: 650 },
        { text: 'xy', offset_ms: 1300, duration_ms: 650 },
        { text: 'm', offset_ms: 1650, duration_ms: 260 },
        { text: 'n', offset_ms: 1910, duration_ms: 260 },
      ],
    },
    { start_ms: 20000, duration_ms: 1600, text: 'next', words: [{ text: 'next', offset_ms: 0, duration_ms: 1000 }] },
    {
      start_ms: 23000,
      duration_ms: 4400,
      text: '我坚决 冲破这一场浩劫',
      words: [
        { text: '我', offset_ms: 0, duration_ms: 440 },
        { text: '坚', offset_ms: 440, duration_ms: 225 },
        { text: '决', offset_ms: 665, duration_ms: 735 },
        { text: ' ', offset_ms: 1400, duration_ms: 0 },
        { text: '冲', offset_ms: 2008, duration_ms: 481 },
        { text: '破', offset_ms: 2489, duration_ms: 251 },
        { text: '这', offset_ms: 2740, duration_ms: 484 },
        { text: '一', offset_ms: 3224, duration_ms: 512 },
        { text: '场', offset_ms: 3736, duration_ms: 497 },
        { text: '浩', offset_ms: 4233, duration_ms: 167 },
      ],
    },
  ],
};
const singingAnnotations = [
  { annotation_type: 'breath', start_ms: 7350, duration_ms: 300, text: '换气' },
  { annotation_type: 'stress', start_ms: 8050, duration_ms: 400, text: '阔' },
  { annotation_type: 'long_tone', start_ms: 8700, duration_ms: 600, text: '天' },
  { annotation_type: 'breath', start_ms: 13050, duration_ms: 260, text: '换气' },
  { annotation_type: 'breath', start_ms: 13900, duration_ms: 260, text: 'x' },
  { annotation_type: 'stress', start_ms: 13900, duration_ms: 260, text: 'x' },
  { annotation_type: 'breath', start_ms: 14250, duration_ms: 180, text: 'm' },
  { annotation_type: 'stress', start_ms: 14250, duration_ms: 180, text: 'm' },
  { annotation_type: 'stress', start_ms: 14510, duration_ms: 180, text: 'n' },
  { annotation_type: 'breath', start_ms: 23000, duration_ms: 300, text: '我' },
  { annotation_type: 'stress', start_ms: 23665, duration_ms: 400, text: '决' },
  { annotation_type: 'breath', start_ms: 25008, duration_ms: 300, text: '冲' },
  { annotation_type: 'stress', start_ms: 26736, duration_ms: 300, text: '场' },
  { annotation_type: 'stress', start_ms: 27233, duration_ms: 300, text: '浩' },
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

const dragonKnightReal = normalizeLyricPayload(JSON.parse(readFileSync(resolve(SCRIPT_DIR, 'dragon-knight-real.json'), 'utf8')));

let fetchResultCount = 0;

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

function overlappingBoxes(boxes) {
  for (let left = 0; left < boxes.length; left += 1) {
    for (let right = left + 1; right < boxes.length; right += 1) {
      const a = boxes[left];
      const b = boxes[right];
      const separated = a.x + a.w <= b.x + 2
        || b.x + b.w <= a.x + 2
        || a.y + a.h <= b.y + 2
        || b.y + b.h <= a.y + 2;
      if (!separated) {
        return true;
      }
    }
  }
  return false;
}

function startVite() {
  const isWindows = process.platform === 'win32';
  const command = isWindows ? 'cmd.exe' : 'npm';
  const args = isWindows
    ? ['/d', '/s', '/c', 'npm run dev -- --port 5181 --strictPort']
    : ['run', 'dev', '--', '--port', '5181', '--strictPort'];
  const child = spawn(command, args, { cwd: SCRIPT_DIR, stdio: ['ignore', 'pipe', 'pipe'] });
  let output = '';
  child.stdout.on('data', (chunk) => { output += chunk.toString(); });
  child.stderr.on('data', (chunk) => { output += chunk.toString(); });
  return { child, getOutput: () => output };
}

async function waitForServer() {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(BASE);
      if (response.ok) {
        return;
      }
    } catch {
      // Keep polling until Vite is ready.
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  }
  throw new Error('Meta stress dev server did not start in time');
}

async function stopServer(server) {
  if (!server?.child || server.child.exitCode !== null) {
    return;
  }
  if (process.platform === 'win32') {
    await new Promise((resolvePromise) => {
      const killer = spawn('taskkill.exe', ['/pid', String(server.child.pid), '/t', '/f'], { stdio: 'ignore' });
      killer.on('exit', resolvePromise);
      killer.on('error', resolvePromise);
    });
    return;
  }
  server.child.kill();
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  if (server.child.exitCode === null) {
    server.child.kill('SIGKILL');
  }
}

const dragonKnightPrevLine = dragonKnightReal.lines.find((line) => line.text === '咆哮声不自觉');
const dragonKnightHengLine = dragonKnightReal.lines.find((line) => line.text === '横越过了几条街');
const dragonKnightPrevBreath = dragonKnightPrevLine?.words.flatMap((word) => word.annotations || []).find((annotation) => annotation.type === 'breath' && annotation.text === '横');
const dragonKnightHengBreath = dragonKnightHengLine?.words.find((word) => word.text === '横')?.annotations.find((annotation) => annotation.type === 'breath' && annotation.text === '横');
requirePass(dragonKnightHengBreath && !dragonKnightPrevBreath, 'DRAGON_KNIGHT_HENG_BREATH_ANCHORED_TO_WRONG_LINE', {
  prev: dragonKnightPrevLine?.words.map((word) => ({ text: word.text, annotations: word.annotations })),
  heng: dragonKnightHengLine?.words.map((word) => ({ text: word.text, annotations: word.annotations })),
});
const dragonKnightChargeLine = dragonKnightReal.lines.find((line) => line.text === '我坚决 冲破这一场浩劫');
const dragonKnightWoBreath = dragonKnightChargeLine?.words.find((word) => word.text === '我')?.annotations.find((annotation) => annotation.type === 'breath');
const dragonKnightChongBreath = dragonKnightChargeLine?.words.find((word) => word.text === '冲')?.annotations.find((annotation) => annotation.type === 'breath');
const dragonKnightJueStress = dragonKnightChargeLine?.words.find((word) => word.text === '决')?.annotations.find((annotation) => annotation.type === 'stress');
requirePass(dragonKnightWoBreath && !dragonKnightWoBreath.suppressLabel && dragonKnightChongBreath && dragonKnightChongBreath.suppressLabel && dragonKnightJueStress, 'DRAGON_KNIGHT_SINGLE_STRESS_BREATH_LABEL_PRIORITY_WRONG', {
  charge: dragonKnightChargeLine?.words.map((word) => ({ text: word.text, annotations: word.annotations })),
});

const server = startVite();
let browser;
try {
await waitForServer();
browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 920 } });
await page.route('**/*', (route) => {
  const path = new URL(route.request().url()).pathname;
  if (!path.startsWith('/api/')) return route.continue();
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
    countdownRows: [...document.querySelectorAll('.lyric-karaoke-meta-countdown')].map((node) => ({ text: node.textContent.trim(), className: node.className })),
    karaokeItems: [...document.querySelectorAll('.lyric-karaoke-lines .lyric-karaoke-line')].map((node) => ({ text: node.textContent.trim(), className: node.className })),
    directKaraokeItems: [...document.querySelectorAll('.lyric-karaoke-lines > .lyric-karaoke-line')].map((node) => ({ text: node.textContent.trim(), className: node.className })),
    metaPanels: [...document.querySelectorAll('.lyric-karaoke-meta-panel')].map((node) => ({ text: node.textContent.trim(), className: node.className })),
    countdownCount: document.querySelectorAll('.lyric-karaoke-meta-countdown .lyric-gap-dot-active').length,
    poppingCount: document.querySelectorAll('.lyric-karaoke-meta-countdown .lyric-gap-dot-popping').length,
    goneCount: document.querySelectorAll('.lyric-karaoke-meta-countdown .lyric-gap-dot-gone').length,
    exitingCount: document.querySelectorAll('.lyric-karaoke-meta-countdown.lyric-dots-exiting').length,
    dotBackgrounds: [...document.querySelectorAll('.lyric-karaoke-meta-countdown .lyric-gap-dot-active, .lyric-karaoke-meta-countdown .lyric-gap-dot-popping')].map((dot) => getComputedStyle(dot).backgroundImage),
    dotAnimations: [...document.querySelectorAll('.lyric-karaoke-meta-countdown .lyric-gap-dot-popping')].map((dot) => getComputedStyle(dot).animationName),
    ringAnimations: [...document.querySelectorAll('.lyric-karaoke-meta-countdown .lyric-gap-dot-popping')].map((dot) => getComputedStyle(dot, '::before').animationName),
    fragmentAnimations: [...document.querySelectorAll('.lyric-karaoke-meta-countdown .lyric-gap-dot-popping')].map((dot) => getComputedStyle(dot, '::after').animationName),
  }));
}

async function activeMetaSample(ms) {
  await seek(ms);
  const activeCount = await page.locator('.lyric-karaoke-meta-panel').count();
  if (!activeCount) {
    const debug = await page.evaluate(() => ({
      time: document.querySelector('.result-dialog .lyric-time')?.textContent?.trim() || null,
      lineCount: document.querySelectorAll('.lyric-line').length,
      lines: [...document.querySelectorAll('.lyric-line')].slice(0, 8).map((node) => ({ text: node.textContent.trim(), className: node.className })),
      metaPanels: [...document.querySelectorAll('.lyric-karaoke-meta-panel')].map((node) => ({ text: node.textContent.trim(), className: node.className })),
      dialog: Boolean(document.querySelector('.result-dialog')),
    }));
    throw new Error(`NO_ACTIVE ${JSON.stringify(debug)}`);
  }
  return page.locator('.lyric-karaoke-meta-panel').first().evaluate((node, ms) => {
    const rect = node.getBoundingClientRect();
    const textNode = node.querySelector('.lyric-karaoke-meta-line');
    return {
      ms,
      text: textNode?.textContent?.trim() || node.textContent.trim(),
      className: textNode?.className || '',
      panelClassName: node.className,
      height: Math.round(rect.height),
      scrollWidth: node.scrollWidth,
      clientWidth: node.clientWidth,
      whiteSpace: textNode ? getComputedStyle(textNode).whiteSpace : getComputedStyle(node).whiteSpace,
      title: node.querySelector('.lyric-karaoke-meta-title')?.textContent?.trim() || '',
      detail: node.querySelector('.lyric-karaoke-meta-line')?.textContent?.trim() || '',
      countdownVisible: Boolean(node.querySelector('.lyric-karaoke-meta-countdown')),
      titleCentered: (() => {
        const title = node.querySelector('.lyric-karaoke-meta-title');
        if (!title) return false;
        const titleRect = title.getBoundingClientRect();
        const panelRect = node.getBoundingClientRect();
        return Math.abs((titleRect.left + titleRect.width / 2) - (panelRect.left + panelRect.width / 2)) <= 3;
      })(),
      hasMetaIndex: Boolean(node.querySelector('.lyric-karaoke-meta-index')),
      directKaraokeItems: document.querySelectorAll('.lyric-karaoke-lines > .lyric-karaoke-line').length,
      detailRows: [...document.querySelectorAll('.lyric-karaoke-meta-line')].map((item) => item.textContent.trim()),
    };
  }, ms);
}

const metaSamples = [];
for (const ms of [200, 2600, 5000]) {
  metaSamples.push(await activeMetaSample(ms));
}
for (const sample of metaSamples) {
  requirePass(sample.panelClassName.includes('lyric-karaoke-meta-panel'), 'META_PANEL_CLASS_MISSING', sample);
  requirePass(!sample.className.includes('lyric-karaoke-line'), 'META_SHOULD_NOT_JOIN_KARAOKE_LANES', sample);
  requirePass(sample.directKaraokeItems === 0, 'META_SHOULD_NOT_RENDER_IN_DUAL_LANES', sample);
  requirePass(sample.whiteSpace === 'nowrap', 'META_LINE_WRAPPED', sample);
  requirePass(sample.height <= 76, 'META_LINE_TOO_TALL', sample);
  requirePass(sample.title.includes(MOCK_SEARCH.results[0].title), 'META_TITLE_SHOULD_STAY_VISIBLE', sample);
  requirePass(sample.titleCentered, 'META_TITLE_SHOULD_BE_CENTERED', sample);
  requirePass(!sample.hasMetaIndex, 'META_INDEX_SHOULD_BE_REMOVED', sample);
}
requirePass(metaSamples.some((sample) => sample.detail.includes('作词') && sample.detail.includes('作曲') && sample.detail.includes('编曲')), 'META_COMBINED_LINE_MISSING', { metaSamples });
requirePass(metaSamples.every((sample) => sample.detailRows.length <= 1), 'META_DETAIL_SHOULD_RENDER_ONCE', { metaSamples });
requirePass(metaSamples.every((sample) => !sample.detail.includes('海 阔 天 空')), 'BODY_SHOULD_NOT_RENDER_DURING_METADATA', { metaSamples });
const titleOccurrences = metaSamples.filter((sample) => sample.title.includes(MOCK_SEARCH.results[0].title)).length;
requirePass(titleOccurrences === metaSamples.length, 'TITLE_SHOULD_REMAIN_VISIBLE_DURING_METADATA', { titleOccurrences, metaSamples });

const introCountdownSample = await activeTextSample(200);
requirePass(introCountdownSample.countdownCount === 0, 'INTRO_COUNTDOWN_SHOULD_NOT_APPEAR_DURING_METADATA', introCountdownSample);
requirePass(introCountdownSample.metaPanels.length === 1 && introCountdownSample.directKaraokeItems.length === 0, 'METADATA_SHOULD_STAY_ABOVE_KARAOKE_LANES', introCountdownSample);
const countdownSample = await activeTextSample(6500);
requirePass(countdownSample.countdownCount === 1, 'COUNTDOWN_AFTER_META_WRONG', countdownSample);
requirePass(countdownSample.countdownRows[0]?.className.includes('lyric-karaoke-meta-countdown'), 'INTRO_COUNTDOWN_SHOULD_USE_META_ROW', countdownSample);
requirePass(countdownSample.dotBackgrounds.some((background) => background.includes('47, 158, 132') || background.includes('242, 211, 111')), 'COUNTDOWN_BUBBLE_COLOR_NOT_REFRESHED', countdownSample);
const shortGapSample = await activeTextSample(10200);
requirePass(shortGapSample.countdownCount === 0, 'SHORT_GAP_SHOULD_NOT_SHOW_COUNTDOWN', shortGapSample);
const interludeSample = await activeTextSample(17000);
requirePass(interludeSample.countdownCount === 3 && interludeSample.poppingCount === 0, 'INTERLUDE_COUNTDOWN_MISSING', interludeSample);
requirePass(interludeSample.karaokeItems.length <= 2 && interludeSample.countdownRows[0]?.className.includes('lyric-karaoke-meta-countdown'), 'COUNTDOWN_SHOULD_USE_META_ROW', interludeSample);
const twoDotSample = await activeTextSample(18000);
requirePass(twoDotSample.countdownCount === 2 && twoDotSample.poppingCount === 1 && twoDotSample.goneCount === 0, 'COUNTDOWN_THIRD_DOT_SHOULD_POP_ALONE', twoDotSample);
const oneDotSample = await activeTextSample(19000);
requirePass(oneDotSample.countdownCount === 1 && oneDotSample.poppingCount === 1 && oneDotSample.goneCount === 1, 'COUNTDOWN_SECOND_DOT_SHOULD_POP_ALONE', oneDotSample);
const bubbleSample = await activeTextSample(20040);
requirePass(bubbleSample.countdownCount === 0 && bubbleSample.poppingCount === 1 && bubbleSample.goneCount === 2 && bubbleSample.exitingCount === 1, 'COUNTDOWN_FINAL_DOT_SHOULD_POP_ALONE', bubbleSample);
requirePass(bubbleSample.dotAnimations.length === 1 && bubbleSample.dotAnimations[0].includes('lyric-dot-pop-core'), 'COUNTDOWN_BUBBLE_CORE_POP_MISSING', bubbleSample);
requirePass(bubbleSample.ringAnimations.length === 1 && bubbleSample.ringAnimations[0].includes('lyric-dot-pop-ring'), 'COUNTDOWN_BUBBLE_RING_POP_MISSING', bubbleSample);
requirePass(bubbleSample.fragmentAnimations.length === 1 && bubbleSample.fragmentAnimations[0].includes('lyric-dot-pop-fragments'), 'COUNTDOWN_BUBBLE_FRAGMENT_POP_MISSING', bubbleSample);

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
const labelBoxes = await page.locator('.lyric-line-active .lyric-annotation-label').evaluateAll((nodes) => nodes.map((node) => {
  const rect = node.getBoundingClientRect();
  return { text: node.textContent.trim(), x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) };
}));
const stressWord = wordBoxes.find((word) => word.text.includes('阔'));
const longToneWord = wordBoxes.find((word) => word.text.includes('天'));
const breathWord = wordBoxes.find((word) => word.text.includes('海'));
const stressSample = { word: stressWord, glyph: stressGlyph, label: stressLabel, labelBoxes };
const longToneSample = { word: longToneWord, glyph: longToneGlyph, label: longToneLabel, labelBoxes };
const breathSample = { word: breathWord, glyph: breathGlyph, label: breathLabel, labelBoxes };
const labelTopSpread = Math.max(...labelBoxes.map((box) => box.y)) - Math.min(...labelBoxes.map((box) => box.y));
const labelCenterSpread = Math.max(...labelBoxes.map((box) => box.y + box.h / 2)) - Math.min(...labelBoxes.map((box) => box.y + box.h / 2));
requirePass(labelBoxes.length >= 3, 'INLINE_ANNOTATION_LABELS_MISSING', { labelBoxes });
requirePass(!overlappingBoxes(labelBoxes), 'INLINE_ANNOTATION_LABELS_OVERLAPPED', { labelBoxes });
requirePass(labelTopSpread <= 1 && labelCenterSpread <= 1, 'INLINE_ANNOTATION_LABELS_NOT_ALIGNED', { labelBoxes, labelTopSpread, labelCenterSpread });
requirePass(stressWord && stressGlyph && stressLabel, 'STRESS_BOUNDS_MISSING', stressSample);
requirePass(longToneWord && longToneGlyph && longToneLabel, 'LONG_TONE_BOUNDS_MISSING', longToneSample);
requirePass(breathWord && breathGlyph && breathLabel, 'BREATH_BOUNDS_MISSING', breathSample);
requirePass(stressGlyph.y + stressGlyph.h / 2 >= stressWord.y + stressWord.h * 0.70, 'STRESS_DOT_NOT_BELOW_TEXT', stressSample);
requirePass(stressGlyph.y - stressWord.y <= stressWord.h * 1.18, 'STRESS_DOT_TOO_FAR_BELOW_TEXT', stressSample);
requirePass(longToneGlyph.y + longToneGlyph.h / 2 >= longToneWord.y + longToneWord.h * 0.60, 'LONG_TONE_UNDERSCORE_NOT_BELOW_TEXT', longToneSample);
requirePass(longToneGlyph.y - longToneWord.y <= longToneWord.h * 0.80, 'LONG_TONE_UNDERSCORE_TOO_FAR_BELOW_TEXT', longToneSample);
requirePass(stressLabel.y + stressLabel.h <= stressWord.y - 4, 'STRESS_LABEL_NOT_ABOVE_TEXT', stressSample);
requirePass(longToneLabel.y + longToneLabel.h <= longToneWord.y + 4, 'LONG_TONE_LABEL_NOT_ABOVE_TEXT', longToneSample);
requirePass(breathLabel.y + breathLabel.h <= breathWord.y + 6, 'BREATH_LABEL_NOT_ABOVE_TEXT', breathSample);
requirePass(Math.abs((stressGlyph.x + stressGlyph.w / 2) - (stressWord.x + stressWord.w / 2)) <= 8, 'STRESS_DOT_NOT_CENTERED', stressSample);
requirePass(Math.abs((longToneGlyph.x + longToneGlyph.w / 2) - (longToneWord.x + longToneWord.w / 2)) <= 10, 'LONG_TONE_UNDERSCORE_NOT_CENTERED', longToneSample);

await seek(13950);
const multiMarkerSample = await page.evaluate(() => {
  const box = (node) => {
    if (!node) return null;
    const rect = node.getBoundingClientRect();
    return { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) };
  };
  const words = [...document.querySelectorAll('.lyric-line-active .lyric-word')].map((node) => {
    const breathGlyph = node.querySelector('.annotation-breath .annotation-glyph-text');
    const stressGlyph = node.querySelector('.annotation-stress .annotation-glyph-text');
    return {
      text: node.querySelector('.lyric-progress-base')?.textContent?.trim() || '',
      box: box(node),
      breath: node.querySelectorAll('.annotation-breath').length,
      stress: node.querySelectorAll('.annotation-stress').length,
      labels: [...node.querySelectorAll('.lyric-annotation-label')].map((label) => label.textContent.trim()),
      breathGlyph: box(breathGlyph),
      stressGlyph: box(stressGlyph),
    };
  });
  const labels = [...document.querySelectorAll('.lyric-line-active .lyric-annotation-label')].map((label) => ({
    text: label.textContent.trim(),
    ...box(label),
  }));
  return { words, labels };
});
const markerAWord = multiMarkerSample.words.find((word) => word.text === 'a');
const markerBWord = multiMarkerSample.words.find((word) => word.text === 'b');
const markerXyWord = multiMarkerSample.words.find((word) => word.text === 'xy');
const markerMWord = multiMarkerSample.words.find((word) => word.text === 'm');
const markerNWord = multiMarkerSample.words.find((word) => word.text === 'n');
requirePass(markerAWord && markerBWord && markerXyWord && markerMWord && markerNWord, 'MULTI_MARKER_WORDS_MISSING', multiMarkerSample);
requirePass(markerAWord?.breath === 0 && markerBWord?.breath === 1, 'BREATH_FALLBACK_ANCHORED_TO_WRONG_WORD', multiMarkerSample);
requirePass(markerXyWord?.breath === 1 && markerXyWord?.stress === 1, 'SAME_CHARACTER_MARKERS_NOT_RENDERED_TOGETHER', multiMarkerSample);
requirePass(markerXyWord.labels.length === 1, 'SAME_CHARACTER_LABELS_NOT_DEDUPED', multiMarkerSample);
requirePass(markerXyWord.labels[0] === '换气', 'SINGLE_STRESS_NEAR_BREATH_SHOULD_KEEP_BREATH_LABEL', multiMarkerSample);
requirePass(markerMWord?.breath === 1 && markerMWord?.stress === 1 && markerNWord?.stress === 1, 'REPEATED_STRESS_MARKERS_MISSING', multiMarkerSample);
requirePass(markerMWord.labels.length === 1 && markerMWord.labels[0] === '重音', 'REPEATED_STRESS_CLUSTER_SHOULD_SUPPRESS_BREATH_LABEL', multiMarkerSample);
requirePass(!overlappingBoxes(multiMarkerSample.labels), 'MULTI_MARKER_LABELS_OVERLAPPED', multiMarkerSample);
requirePass(markerBWord.breathGlyph && markerXyWord.breathGlyph && markerXyWord.stressGlyph, 'MULTI_MARKER_GLYPH_BOUNDS_MISSING', multiMarkerSample);
requirePass(markerBWord.breathGlyph.x + markerBWord.breathGlyph.w / 2 <= markerBWord.box.x + markerBWord.box.w * 0.35, 'BREATH_FALLBACK_NOT_LEADING_TARGET_WORD', multiMarkerSample);
requirePass(markerXyWord.breathGlyph.x + markerXyWord.breathGlyph.w / 2 <= markerXyWord.box.x + markerXyWord.box.w * 0.30, 'SAME_CHARACTER_BREATH_NOT_LEADING_TARGET_CHAR', multiMarkerSample);
requirePass(markerXyWord.stressGlyph.x + markerXyWord.stressGlyph.w / 2 >= markerXyWord.box.x && markerXyWord.stressGlyph.x + markerXyWord.stressGlyph.w / 2 <= markerXyWord.box.x + markerXyWord.box.w * 0.48, 'SAME_CHARACTER_STRESS_NOT_ON_TARGET_CHAR', multiMarkerSample);
requirePass(markerXyWord.stressGlyph.y + markerXyWord.stressGlyph.h / 2 >= markerXyWord.box.y + markerXyWord.box.h * 0.60, 'SAME_CHARACTER_STRESS_DOT_NOT_BELOW_TEXT', multiMarkerSample);

await seek(25200);
const dragonKnightClusterSample = await page.evaluate(() => {
  const box = (node) => {
    if (!node) return null;
    const rect = node.getBoundingClientRect();
    return { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) };
  };
  const words = [...document.querySelectorAll('.lyric-line-active .lyric-word')].map((node) => ({
    text: node.querySelector('.lyric-progress-base')?.textContent?.trim() || '',
    box: box(node),
    labels: [...node.querySelectorAll('.lyric-annotation-label')].map((label) => label.textContent.trim()),
    breathGlyph: box(node.querySelector('.annotation-breath .annotation-glyph-text')),
    stressGlyph: box(node.querySelector('.annotation-stress .annotation-glyph-text')),
    hasBreathSpacing: node.classList.contains('lyric-word-has-breath'),
  }));
  const labels = [...document.querySelectorAll('.lyric-line-active .lyric-annotation-label')].map((label) => ({
    text: label.textContent.trim(),
    ...box(label),
  }));
  return { words, labels };
});
const jueWord = dragonKnightClusterSample.words.find((word) => word.text === '决');
const chongWord = dragonKnightClusterSample.words.find((word) => word.text === '冲');
requirePass(jueWord?.stressGlyph && chongWord?.breathGlyph, 'DRAGON_KNIGHT_CLUSTER_MARKERS_MISSING', dragonKnightClusterSample);
requirePass(chongWord.hasBreathSpacing, 'BREATH_WORD_SHOULD_RESERVE_SPACING', dragonKnightClusterSample);
requirePass(jueWord.labels.includes('重音') && !chongWord.labels.includes('换气'), 'STRESS_CLUSTER_SHOULD_HIDE_BREATH_LABEL', dragonKnightClusterSample);
requirePass(chongWord.box.x - (jueWord.box.x + jueWord.box.w) >= 3, 'BREATH_CLUSTER_WORDS_TOO_TIGHT', dragonKnightClusterSample);

await seek(27400);
const endingKaraokeSample = await page.evaluate(() => ({
  rows: [...document.querySelectorAll('.lyric-karaoke-lines .lyric-karaoke-line')].map((node) => ({
    text: node.textContent.trim(),
    className: node.className,
  })),
  placeholders: document.querySelectorAll('.lyric-karaoke-placeholder').length,
}));
requirePass(endingKaraokeSample.rows.length >= 2, 'ENDING_KARAOKE_SHOULD_KEEP_PREVIOUS_LINE', endingKaraokeSample);
requirePass(endingKaraokeSample.rows.some((row) => row.text.includes('决') && row.text.includes('冲')), 'ENDING_KARAOKE_LAST_LINE_MISSING', endingKaraokeSample);
requirePass(endingKaraokeSample.rows.some((row) => row.text.includes('next')), 'ENDING_KARAOKE_PREVIOUS_LINE_MISSING', endingKaraokeSample);

console.log('META_SAMPLES:', JSON.stringify(metaSamples));
console.log('INTRO_COUNTDOWN:', JSON.stringify(introCountdownSample));
console.log('COUNTDOWN_AFTER_META:', JSON.stringify(countdownSample));
console.log('SHORT_GAP_SAMPLE:', JSON.stringify(shortGapSample));
console.log('INTERLUDE_COUNTDOWN:', JSON.stringify(interludeSample));
console.log('TWO_DOT_COUNTDOWN:', JSON.stringify(twoDotSample));
console.log('ONE_DOT_COUNTDOWN:', JSON.stringify(oneDotSample));
console.log('BUBBLE_SAMPLE:', JSON.stringify(bubbleSample));
console.log('ANNOTATION_COUNTS_AFTER_REPEAT_FETCH:', JSON.stringify(annotationCounts));
console.log('STRESS_SAMPLE:', JSON.stringify(stressSample));
console.log('LONG_TONE_SAMPLE:', JSON.stringify(longToneSample));
console.log('BREATH_SAMPLE:', JSON.stringify(breathSample));
console.log('MULTI_MARKER_SAMPLE:', JSON.stringify(multiMarkerSample));
console.log('DRAGON_KNIGHT_CLUSTER_SAMPLE:', JSON.stringify(dragonKnightClusterSample));
console.log('ENDING_KARAOKE_SAMPLE:', JSON.stringify(endingKaraokeSample));
await page.screenshot({ path: resolve(SCRIPT_DIR, 'playwright-artifacts/verify-meta-stress.png'), fullPage: false });
} finally {
  if (browser) {
    await browser.close();
  }
  await stopServer(server);
}
