import { chromium } from '@playwright/test';

const BASE = 'http://127.0.0.1:5181';
const MOCK_SEARCH = { results: [{ id: 'test-001', title: '倒计时', artist: '测试', source: 'qq' }], warnings: [] };
const MOCK_FETCH_RESULT = {
  unified: {
    mode: 'word',
    meta: { title: '倒计时', artist: '测试' },
    inline_lines: [
      { start_ms: 7200, duration_ms: 1400, text: 'first', words: [{ text: 'first', offset_ms: 0, duration_ms: 1000 }] },
      { start_ms: 20000, duration_ms: 1600, text: 'next', words: [{ text: 'next', offset_ms: 0, duration_ms: 1000 }] },
    ],
  },
};

const browser = await chromium.launch({ headless: false });
const page = await browser.newPage();
await page.route('**/*', (route) => {
  const path = new URL(route.request().url()).pathname;
  if (!path.startsWith('/api/')) return route.continue();
  if (path === '/api/search') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_SEARCH) });
  if (path === '/api/fetch') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_SEARCH.results[0]) });
  if (path === '/api/fetch-result') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_FETCH_RESULT) });
  if (path === '/api/cache') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ entries: [] }) });
  if (path === '/api/health') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, version: 'test' }) });
  if (path === '/api/stats') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ total: 0, fresh: 0, expired: 0 }) });
  return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
});
await page.goto(BASE);
await page.locator('nav button').nth(1).click();
await page.locator('.primary-search input').first().fill('倒计时');
await page.locator('button[type="submit"]').first().click();
await page.locator('.result-card').first().click();
await page.locator('.dialog-actions button').nth(1).click();
await page.waitForTimeout(600);

const seekInput = page.locator('.result-dialog input[type="range"]').first();
const timeLocator = page.locator('.result-dialog .lyric-time');
const max = Number(await seekInput.getAttribute('max'));

function formatMs(ms) {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, '0');
  return `${minutes}:${seconds}`;
}

async function waitForTimeChange(expectedTime, beforeText) {
  await page.waitForFunction(
    ({ expectedTime, beforeText }) => {
      const text = document.querySelector('.result-dialog .lyric-time')?.textContent?.trim() || '';
      return text !== beforeText && text.startsWith(`${expectedTime} /`);
    },
    { expectedTime, beforeText },
    { timeout: 2400 },
  );
}

async function setRangeValue(ms) {
  const expectedTime = formatMs(ms);
  const beforeText = (await timeLocator.innerText()).trim();
  await seekInput.evaluate((input, value) => {
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
    setter.call(input, String(value));
    const inputEvent = typeof InputEvent === 'function'
      ? new InputEvent('input', { bubbles: true, inputType: 'insertReplacementText', data: String(value) })
      : new Event('input', { bubbles: true });
    input.dispatchEvent(inputEvent);
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }, ms);

  try {
    await waitForTimeChange(expectedTime, beforeText);
    return 'native-setter';
  } catch {
    const box = await seekInput.boundingBox();
    await page.mouse.click(box.x + box.width * Math.min(1, Math.max(0, ms / max)), box.y + box.height / 2);
    await waitForTimeChange(expectedTime, beforeText);
    return 'mouse-click';
  }
}

async function seek(ms) {
  const expectedTime = formatMs(ms);
  const currentTime = (await timeLocator.innerText()).trim();
  if (currentTime.startsWith(`${expectedTime} /`)) {
    const resetMs = ms >= 1200 ? ms - 1200 : ms + 1200;
    await setRangeValue(resetMs);
  }
  const method = await setRangeValue(ms);
  const time = await timeLocator.innerText();
  const count = await page.locator('.lyric-line-countdown .lyric-gap-dot:not(.lyric-gap-dot-hidden)').count();
  const exiting = await page.locator('.lyric-line-countdown.lyric-dots-exiting').count();
  const rows = await page.locator('.lyric-line-countdown').evaluateAll((nodes) => nodes.map((node) => node.textContent.trim()));
  const dots = await page.locator('.lyric-line-countdown .lyric-gap-dot').evaluateAll((nodes) => nodes.map((node) => node.className));
  return { ms, method, time, count, exiting, rows, dots };
}

const samples = [];
for (const ms of [16600, 18100, 18800, 19720, 19920, 20020, 20180, 20320]) {
  samples.push(await seek(ms));
}
console.log('COUNTDOWN_SAMPLES:', JSON.stringify(samples));
await page.screenshot({ path: 'playwright-artifacts/verify-countdown-samples.png', fullPage: false });
await browser.close();
