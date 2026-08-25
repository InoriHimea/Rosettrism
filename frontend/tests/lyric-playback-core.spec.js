import { expect, test } from '@playwright/test';
import {
  fetchResultForFixture,
  multilingualLyricFixtures,
  searchResultForFixture,
} from './fixtures/multilingual-lyrics.js';

const fixture = multilingualLyricFixtures.find((item) => item.language === 'mandarin');

test.beforeEach(async ({ page }) => {
  await mockApi(page);
  await page.goto('/');
  await page.locator('nav button').nth(1).click();
  await page.locator('.primary-search input').fill(fixture.query);
  await page.locator('button[type="submit"]').click();
  await page.getByRole('button', { name: new RegExp(escapeRegExp(fixture.title)) }).click();
  await page.locator('.dialog-actions .button-primary').first().click();
  await expect(page.locator('.result-dialog .lyric-playback-card')).toBeVisible();
});

test('playback phase and seek state are derived from one deterministic frame state', async ({ page }) => {
  const card = page.locator('.result-dialog .lyric-playback-card');
  const firstWordLine = fixture.document.lines.find((line) => line.words?.length);
  const lastLine = fixture.document.lines.at(-1);

  await expect(card).toHaveAttribute('data-playback-phase', /metadata|countdown|interlude/);
  await seek(card, firstWordLine.start_ms + 300);
  await expect(card).toHaveAttribute('data-playback-phase', 'singing');
  await expect(card.locator('.lyric-line-active')).toBeVisible();
  await expect(card).toHaveAttribute('data-playback-time-ms', String(firstWordLine.start_ms + 300));

  const requestedEndMs = lastLine.start_ms + lastLine.duration_ms + 500;
  const durationMs = Number(await card.locator('input[type="range"]').getAttribute('max'));
  await seek(card, requestedEndMs, durationMs);
  await expect(card).toHaveAttribute('data-playback-phase', 'ended');
});

test('play, pause, resume, and restart follow the clock without stale progress', async ({ page }) => {
  const card = page.locator('.result-dialog .lyric-playback-card');
  const playButton = card.locator('.lyric-controls .button-primary');
  const restartButton = card.locator('.lyric-controls .button-secondary').first();
  const firstWordLine = fixture.document.lines.find((line) => line.words?.length);

  await seek(card, firstWordLine.start_ms + 300);
  await playButton.click();
  const started = Number(await card.getAttribute('data-playback-time-ms'));
  await page.waitForTimeout(240);
  const advanced = Number(await card.getAttribute('data-playback-time-ms'));
  expect(advanced).toBeGreaterThan(started + 50);

  await playButton.click();
  const paused = Number(await card.getAttribute('data-playback-time-ms'));
  await page.waitForTimeout(220);
  const stillPaused = Number(await card.getAttribute('data-playback-time-ms'));
  expect(Math.abs(stillPaused - paused)).toBeLessThanOrEqual(20);

  await playButton.click();
  await page.waitForTimeout(180);
  expect(Number(await card.getAttribute('data-playback-time-ms'))).toBeGreaterThan(stillPaused + 80);

  await restartButton.click();
  await expect(card).toHaveAttribute('data-playback-time-ms', /^(?:0|[1-9]\d?|1\d\d)$/);
  await expect(card).toHaveAttribute('data-playback-phase', /metadata|countdown|interlude/);
  await playButton.click();
});

async function seek(card, ms, expectedMs = ms) {
  const input = card.locator('input[type="range"]').first();
  await input.evaluate((node, value) => {
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
    setter.call(node, String(value));
    node.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertReplacementText', data: String(value) }));
    node.dispatchEvent(new Event('change', { bubbles: true }));
  }, ms);
  await expect(card).toHaveAttribute('data-playback-time-ms', String(expectedMs));
}

async function mockApi(page) {
  const routes = {
    health: { ok: true, version: 'playback-core-fixture', cache: true },
    stats: {
      cache: {}, provider_health: [], ai_scores: [], fetch_runs: [], fetch_run_status_counts: [],
    },
    cache: { entries: [], unified_entries: [] },
    sources: { sources: [] },
    search: { results: [searchResultForFixture(fixture)], warnings: [] },
    'fetch-result': fetchResultForFixture(fixture),
  };
  for (const [name, body] of Object.entries(routes)) {
    await page.route(`**/api/${name}`, (route) => route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify(body),
    }));
  }
}

function escapeRegExp(value) {
  return String(value).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
