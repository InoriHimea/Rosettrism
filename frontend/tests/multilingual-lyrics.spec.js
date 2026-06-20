import { expect, test } from '@playwright/test';
import {
  fetchResultForFixture,
  multilingualLyricFixtures,
  searchResultForFixture,
} from './fixtures/multilingual-lyrics.js';

async function mockApi(page, fixture) {
  await page.route('**/api/health', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ ok: true, version: 'multilingual-fixture', cache: true }),
  }));
  await page.route('**/api/stats', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({
      cache: {
        upstream_entries: 4,
        unified_entries: 4,
        ai_score_entries: 0,
        fetch_run_entries: 4,
        fresh_upstream_entries: 4,
        expired_upstream_entries: 0,
      },
      provider_health: [],
      ai_scores: [],
      fetch_runs: [],
      fetch_run_status_counts: [],
    }),
  }));
  await page.route('**/api/cache', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ entries: [], unified_entries: [] }),
  }));
  await page.route('**/api/sources', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ sources: [] }),
  }));
  await page.route('**/api/search', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ results: [searchResultForFixture(fixture)], warnings: [] }),
  }));
  await page.route('**/api/fetch-result', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify(fetchResultForFixture(fixture)),
  }));
}

for (const fixture of multilingualLyricFixtures) {
  test(`${fixture.language} fixture opens playable lyric view for ${fixture.title}`, async ({ page }) => {
    await mockApi(page, fixture);
    await page.goto('/');

    await page.locator('nav button').nth(1).click();
    await page.locator('.primary-search input').fill(fixture.query);
    await page.locator('button[type="submit"]').click();
    await page.getByRole('button', { name: new RegExp(escapeRegExp(fixture.title)) }).click();
    await page.locator('.dialog-actions .button-primary').first().click();

    const dialog = page.locator('.result-dialog');
    await expect(dialog).toBeVisible();
    await expect(page.getByTestId('karaoke-stage')).toBeVisible();
    await expect(dialog.locator('.lyric-current-strip')).toHaveCount(0);
    await expect(dialog.locator('.lyric-karaoke-meta-index')).toHaveCount(0);
    await expect(dialog.locator('.lyric-karaoke-meta-title')).toBeVisible();
    await expect(dialog.locator('.lyric-karaoke-meta-title').first()).toContainText(fixture.title);
    await expect(dialog.locator('.lyric-karaoke-meta-title').first()).toHaveCSS('text-align', 'center');
    const firstWordLine = fixture.document.lines.find((line) => line.words?.length);
    await seek(dialog, firstWordLine.start_ms + 300);
    await expect(dialog.locator('.lyric-word').first()).toBeVisible();
    await expect(dialog.locator('.lyric-karaoke-line .lyric-words').first()).toHaveCSS('white-space', 'nowrap');

    if (fixture.searchExtra?.singing_annotations?.length) {
      await seek(dialog, 1800);
      await expect(dialog.locator('.lyric-annotation-mark').first()).toBeVisible();
      const overlap = await labelsOverlap(dialog);
      expect(overlap).toBe(false);
    }

    if (fixture.language === 'japanese') {
      await seek(dialog, 2400);
      await expect(dialog.locator('ruby').first()).toBeVisible();
      await seek(dialog, 7400);
      await expect(dialog.locator('.lyric-line-reading').filter({ hasText: 'kana hyouji' })).toBeVisible();
    }

    if (fixture.language === 'cantonese') {
      await seek(dialog, 8400);
      await expect(dialog.locator('.lyric-line-reading').filter({ hasText: 'hoi fut tin hung' })).toBeVisible();
    }
  });
}

test('multilingual lyric playback stays within mobile viewport', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const fixture = multilingualLyricFixtures.find((item) => item.language === 'english');
  await mockApi(page, fixture);
  await page.goto('/');

  await page.locator('nav button').nth(1).click();
  await page.locator('.primary-search input').fill(fixture.query);
  await page.locator('button[type="submit"]').click();
  await page.getByRole('button', { name: new RegExp(escapeRegExp(fixture.title)) }).click();
  await page.locator('.dialog-actions .button-primary').first().click();
  await expect(page.getByTestId('karaoke-stage')).toBeVisible();

  const overflow = await page.evaluate(() => {
    const documentWidth = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
    return documentWidth - window.innerWidth;
  });
  expect(overflow).toBeLessThanOrEqual(2);

  const controls = page.locator('.result-dialog .lyric-controls button');
  const count = await controls.count();
  for (let index = 0; index < count; index += 1) {
    const box = await controls.nth(index).boundingBox();
    expect(box?.height || 0).toBeGreaterThanOrEqual(32);
  }

  const firstWordLine = fixture.document.lines.find((line) => line.words?.length);
  await seek(page.locator('.result-dialog'), firstWordLine.start_ms + 300);
  await expect(page.locator('.lyric-karaoke-line .lyric-words').first()).toHaveCSS('white-space', 'nowrap');
});

async function seek(dialog, ms) {
  const seekInput = dialog.locator('input[type="range"]').first();
  await seekInput.evaluate((input, value) => {
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
    setter.call(input, String(value));
    input.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertReplacementText', data: String(value) }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }, ms);
  await dialog.page().waitForTimeout(120);
}

async function labelsOverlap(dialog) {
  return dialog.locator('.lyric-line-active .lyric-annotation-label').evaluateAll((nodes) => {
    const boxes = nodes.map((node) => {
      const rect = node.getBoundingClientRect();
      return { x: rect.x, y: rect.y, w: rect.width, h: rect.height };
    });
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
  });
}

function escapeRegExp(value) {
  return String(value).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
