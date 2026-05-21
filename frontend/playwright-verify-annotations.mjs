import { chromium } from '@playwright/test';
import { writeFileSync } from 'fs';

const BASE = 'http://127.0.0.1:5181';

const MOCK_SEARCH = {
  results: [{
    id: 'test-001',
    title: '别怕我伤心',
    artist: '张信哲',
    source: 'qq',
    extra: {
      artist_alias: 'JeffChang',
      singing_annotations: [
        { annotation_type: 'breath',  start_ms: 7600,  duration_ms: 400 },
        { annotation_type: 'stress',  start_ms: 8650,  duration_ms: 400 },
        { annotation_type: 'long_tone', start_ms: 10200, duration_ms: 600 },
      ],
    },
  }],
  warnings: [],
};

const MOCK_FETCH_RESULT = {
  unified: {
    mode: 'word',
    meta: { title: '别怕我伤心', artist: '张信哲' },
    inline_lines: [
      {
        start_ms: 7200,
        duration_ms: 4200,
        text: 'a b c d e f',
        words: [
          { text: 'a', offset_ms: 0,    duration_ms: 650 },
          { text: 'b', offset_ms: 650,  duration_ms: 650 },
          { text: 'c', offset_ms: 1300, duration_ms: 650 },
          { text: 'd', offset_ms: 1950, duration_ms: 650 },
          { text: 'e', offset_ms: 2600, duration_ms: 650 },
          { text: 'f', offset_ms: 3250, duration_ms: 650 },
        ],
      },
      {
        start_ms: 20000,
        duration_ms: 4000,
        text: 'next line',
        words: [
          { text: 'next', offset_ms: 0,    duration_ms: 1000 },
          { text: 'line', offset_ms: 1000, duration_ms: 1000 },
        ],
      },
    ],
  },
  selectedEntry: {
    id: 'test-001',
    title: '别怕我伤心',
    artist: '张信哲',
    extra: {
      artist_alias: 'JeffChang',
      singing_annotations: [
        { annotation_type: 'breath',  start_ms: 7600,  duration_ms: 400 },
        { annotation_type: 'stress',  start_ms: 8650,  duration_ms: 400 },
        { annotation_type: 'long_tone', start_ms: 10200, duration_ms: 600 },
      ],
    },
  },
};

(async () => {
  const browser = await chromium.launch({ headless: false });
  const page = await browser.newPage();

  await page.route('**/api/**', (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/search') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_SEARCH) });
    }
    if (path === '/api/fetch') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_SEARCH.results[0]) });
    }
    if (path === '/api/fetch-result') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_FETCH_RESULT) });
    }
    if (path === '/api/cache') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ entries: [] }) });
    }
    if (path === '/api/health') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, version: 'test' }) });
    }
    if (path === '/api/stats') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ total: 0, fresh: 0, expired: 0 }) });
    }
    return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
  });

  await page.goto(BASE);
  await page.waitForLoadState('networkidle');

  await page.locator('nav button').nth(1).click();
  await page.waitForTimeout(400);

  // Fill search and submit
  const searchInput = page.locator('.primary-search input').first();
  await searchInput.fill('别怕我伤心');
  const submitBtn = page.locator('button[type="submit"]').first();
  await submitBtn.click();
  await page.waitForTimeout(800);

  // Click first result card
  const resultCard = page.locator('.result-card').first();
  await resultCard.click();
  await page.waitForTimeout(600);

  // Click 获取JSON button (index 1 in dialog actions)
  const fetchJsonBtn = page.locator('.dialog-actions button').nth(1);
  await fetchJsonBtn.click();
  await page.waitForTimeout(800);

  // Click play
  const playBtn = page.locator('.result-dialog .lyric-controls .button-primary').first();
  await playBtn.click();
  await page.waitForTimeout(200);

  // Seek to just before first annotated line (7000ms)
  const seekInput = page.locator('.result-dialog input[type="range"]').first();
  const max = await seekInput.getAttribute('max');
  const pct = 7000 / Number(max);
  const box = await seekInput.boundingBox();
  await page.mouse.click(box.x + box.width * pct, box.y + box.height / 2);
  await page.waitForTimeout(600);

  // Screenshot: countdown state (before line starts)
  await page.screenshot({ path: 'playwright-artifacts/verify-countdown.png', fullPage: false });

  // Seek to 7800ms (inside first line, breath annotation active)
  const pct2 = 7800 / Number(max);
  await page.mouse.click(box.x + box.width * pct2, box.y + box.height / 2);
  await page.waitForTimeout(400);
  await page.screenshot({ path: 'playwright-artifacts/verify-annotation-active.png', fullPage: false });

  // Measure annotation mark positions vs word positions
  const words = await page.locator('.lyric-line-active .lyric-word').all();
  const marks = await page.locator('.lyric-line-active .lyric-annotation-mark').all();

  const wordBoxes = await Promise.all(words.map((w) => w.boundingBox()));
  const markBoxes = await Promise.all(marks.map((m) => m.boundingBox()));

  console.log('WORDS:', JSON.stringify(wordBoxes.map((b, i) => ({ i, x: Math.round(b?.x), y: Math.round(b?.y), w: Math.round(b?.width) }))));
  console.log('MARKS:', JSON.stringify(markBoxes.map((b, i) => ({ i, x: Math.round(b?.x), y: Math.round(b?.y), w: Math.round(b?.width) }))));

  // Seek to 8800ms (stress annotation)
  const pct3 = 8800 / Number(max);
  await page.mouse.click(box.x + box.width * pct3, box.y + box.height / 2);
  await page.waitForTimeout(400);
  await page.screenshot({ path: 'playwright-artifacts/verify-annotation-stress.png', fullPage: false });

  // Seek to end of line to check countdown exit (19500ms)
  const pct4 = 19500 / Number(max);
  await page.mouse.click(box.x + box.width * pct4, box.y + box.height / 2);
  await page.waitForTimeout(300);
  await page.screenshot({ path: 'playwright-artifacts/verify-countdown-exit.png', fullPage: false });

  // Seek to 19800ms (exiting state)
  const pct5 = 19800 / Number(max);
  await page.mouse.click(box.x + box.width * pct5, box.y + box.height / 2);
  await page.waitForTimeout(300);
  await page.screenshot({ path: 'playwright-artifacts/verify-countdown-exit2.png', fullPage: false });

  await browser.close();
  console.log('DONE');
})();
