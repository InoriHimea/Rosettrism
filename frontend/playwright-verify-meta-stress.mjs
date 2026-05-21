import { chromium } from '@playwright/test';

const BASE = 'http://127.0.0.1:5181';
const MOCK_SEARCH = { results: [{ id: 'test-001', title: '超长标题测试别怕我伤心雨一直下', artist: '张信哲', source: 'qq', extra: { artist_alias: 'JeffChang' } }], warnings: [] };
const MOCK_FETCH_RESULT = {
  unified: {
    mode: 'word',
    meta: { title: '超长标题测试别怕我伤心雨一直下', artist: '张信哲' },
    inline_lines: [
      { start_ms: 1000, duration_ms: 1000, text: '作词：测试作词', words: [] },
      { start_ms: 2000, duration_ms: 1000, text: '作曲：测试作曲', words: [] },
      {
        start_ms: 7200,
        duration_ms: 4200,
        text: 'a b c d e f',
        words: [
          { text: 'a', offset_ms: 0, duration_ms: 650 },
          { text: 'b', offset_ms: 650, duration_ms: 650 },
          { text: 'c', offset_ms: 1300, duration_ms: 650 },
          { text: 'd', offset_ms: 1950, duration_ms: 650 },
          { text: 'e', offset_ms: 2600, duration_ms: 650 },
          { text: 'f', offset_ms: 3250, duration_ms: 650 },
        ],
      },
      { start_ms: 20000, duration_ms: 1600, text: 'next', words: [{ text: 'next', offset_ms: 0, duration_ms: 1000 }] },
    ],
  },
  selectedEntry: {
    id: 'test-001',
    title: '超长标题测试别怕我伤心雨一直下',
    artist: '张信哲',
    extra: {
      artist_alias: 'JeffChang',
      singing_annotations: [
        { annotation_type: 'stress', start_ms: 8650, duration_ms: 400 },
      ],
    },
  },
};

const browser = await chromium.launch({ headless: false });
const page = await browser.newPage();
await page.route('**/api/**', (route) => {
  const path = new URL(route.request().url()).pathname;
  if (path === '/api/search') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_SEARCH) });
  if (path === '/api/fetch-result') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_FETCH_RESULT) });
  if (path === '/api/cache') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ entries: [] }) });
  if (path === '/api/health') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, version: 'test' }) });
  if (path === '/api/stats') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ total: 0, fresh: 0, expired: 0 }) });
  return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
});

await page.goto(BASE);
await page.locator('nav button').nth(1).click();
await page.locator('.primary-search input').first().fill('超长标题测试');
await page.locator('button[type="submit"]').first().click();
await page.locator('.result-card').first().click();
await page.locator('.dialog-actions button').nth(1).click();
await page.waitForTimeout(600);

const seekInput = page.locator('.result-dialog input[type="range"]').first();
const timeLocator = page.locator('.result-dialog .lyric-time');

function formatMs(ms) {
  const totalSeconds = Math.floor(ms / 1000);
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, '0')}`;
}

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
  await page.waitForTimeout(160);
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
  const sample = await page.locator('.lyric-line-active').first().evaluate((node) => {
    const rect = node.getBoundingClientRect();
    const before = getComputedStyle(node, '::before');
    return {
      text: node.textContent.trim(),
      className: node.className,
      height: Math.round(rect.height),
      scrollWidth: node.scrollWidth,
      clientWidth: node.clientWidth,
      whiteSpace: getComputedStyle(node).whiteSpace,
      beforeWidth: before.width,
      beforeContent: before.content,
    };
  });
  return { ms, ...sample };
}

const metaSamples = [];
for (const ms of [200, 2200, 4200]) {
  metaSamples.push(await activeMetaSample(ms));
}
await seek(6200);
const countdownSample = await page.evaluate(() => ({
  time: document.querySelector('.result-dialog .lyric-time')?.textContent?.trim() || null,
  rows: [...document.querySelectorAll('.lyric-line-countdown')].map((node) => node.textContent.trim()),
  count: document.querySelectorAll('.lyric-line-countdown .lyric-gap-dot:not(.lyric-gap-dot-hidden)').length,
}));

await seek(8800);
const wordBoxes = await page.locator('.lyric-line-active .lyric-word').evaluateAll((nodes) => nodes.map((node, index) => {
  const rect = node.getBoundingClientRect();
  return { index, text: node.textContent.trim(), x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) };
}));
const stressGlyph = await page.locator('.lyric-line-active .annotation-stress .annotation-glyph-text').first().boundingBox();
const stressLabel = await page.locator('.lyric-line-active .annotation-stress .lyric-annotation-label').first().boundingBox();
const stressSample = {
  word: wordBoxes.find((word) => word.text.includes('c')),
  glyph: stressGlyph && { x: Math.round(stressGlyph.x), y: Math.round(stressGlyph.y), w: Math.round(stressGlyph.width), h: Math.round(stressGlyph.height) },
  label: stressLabel && { x: Math.round(stressLabel.x), y: Math.round(stressLabel.y), w: Math.round(stressLabel.width), h: Math.round(stressLabel.height) },
};

console.log('META_SAMPLES:', JSON.stringify(metaSamples));
console.log('COUNTDOWN_AFTER_META:', JSON.stringify(countdownSample));
console.log('STRESS_SAMPLE:', JSON.stringify(stressSample));
await page.screenshot({ path: 'playwright-artifacts/verify-meta-stress.png', fullPage: false });
await browser.close();
