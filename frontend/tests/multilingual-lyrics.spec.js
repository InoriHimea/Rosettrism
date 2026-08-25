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
    await expect(dialog.locator('.lyric-karaoke-meta-title')).toHaveCount(0);
    await expect(dialog.locator('.lyric-playback-head h4')).toContainText(fixture.title);
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

for (const viewport of [
  { width: 390, height: 844 },
  { width: 768, height: 1024 },
  { width: 1280, height: 720 },
  { width: 1440, height: 900 },
]) {
  test(`multilingual lyric playback stays within ${viewport.width}x${viewport.height} viewport`, async ({ page }) => {
    await page.setViewportSize(viewport);
    const fixture = longLineFixture(multilingualLyricFixtures.find((item) => item.language === 'english'));
    await mockApi(page, fixture);
    await page.goto('/');

    await page.locator('nav button').nth(1).click();
    await page.locator('.primary-search input').fill(fixture.query);
    await page.locator('button[type="submit"]').click();
    await page.getByRole('button', { name: new RegExp(escapeRegExp(fixture.title)) }).click();
    await page.locator('.dialog-actions .button-primary').first().click();
    await expect(page.getByTestId('karaoke-stage')).toBeVisible();

    const longLine = fixture.document.lines.at(-1);
    await seek(page.locator('.result-dialog'), longLine.start_ms + 300);
    const activeLine = page.locator('.lyric-karaoke-line.lyric-line-active');
    await expect(activeLine).toBeVisible();
    await expect(activeLine).toHaveAttribute('data-fit', /^(?:compact|tight|wrap)$/);

    const geometry = await page.evaluate(() => {
      const stage = document.querySelector('.lyric-karaoke-lines');
      const active = document.querySelector('.lyric-karaoke-line.lyric-line-active');
      const documentWidth = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
      return {
        documentOverflow: documentWidth - window.innerWidth,
        stageOverflowX: stage ? stage.scrollWidth - stage.clientWidth : 999,
        activeOverflowX: active ? active.scrollWidth - active.clientWidth : 999,
      };
    });
    expect(geometry.documentOverflow).toBeLessThanOrEqual(2);
    expect(geometry.stageOverflowX).toBeLessThanOrEqual(2);
    expect(geometry.activeOverflowX).toBeLessThanOrEqual(2);

    const controls = page.locator('.result-dialog .lyric-controls button');
    const count = await controls.count();
    for (let index = 0; index < count; index += 1) {
      const box = await controls.nth(index).boundingBox();
      expect(box?.height || 0).toBeGreaterThanOrEqual(44);
    }
  });
}

test('default lyric stage keeps ambient effects off and emphasizes the primary controls', async ({ page }) => {
  const fixture = multilingualLyricFixtures.find((item) => item.language === 'mandarin');
  await mockApi(page, fixture);
  await page.goto('/');

  await page.locator('nav button').nth(1).click();
  await page.locator('.primary-search input').fill(fixture.query);
  await page.locator('button[type="submit"]').click();
  await page.getByRole('button', { name: new RegExp(escapeRegExp(fixture.title)) }).click();
  await page.locator('.dialog-actions .button-primary').first().click();

  const dialog = page.locator('.result-dialog');
  const stage = page.getByTestId('karaoke-stage');
  await expect(stage).toBeVisible();
  await expect(stage.locator('.lyric-stage-3d')).toHaveCount(0);
  await expect(stage.locator('.lyric-ambient')).toHaveCount(0);
  await expect(dialog.locator('.lyric-controls > .lyric-timeline-control')).toHaveCount(1);
  await expect(dialog.locator('.lyric-controls > .lyric-playback-actions')).toHaveCount(1);
  await expect(dialog.locator('.lyric-play-toggle')).toBeVisible();
  await expect(dialog.locator('.lyric-play-toggle')).toHaveCSS('min-height', '44px');
  const firstBodyLine = fixture.document.lines.find((line) => line.words?.length);
  await seek(dialog, firstBodyLine.start_ms + 300);
  await expect(dialog.locator('.lyric-line-active')).toHaveCSS('filter', 'none');

  const visualNoise = await stage.evaluate((node) => {
    const stageStyle = getComputedStyle(node);
    const active = node.querySelector('.lyric-line-active');
    const activeBefore = active ? getComputedStyle(active, '::before') : null;
    return {
      backgroundImages: stageStyle.backgroundImage.split(',').length,
      activeDecoration: activeBefore?.content || '',
    };
  });
  expect(visualNoise.backgroundImages).toBeLessThanOrEqual(1);
  expect(visualNoise.activeDecoration).toBe('none');
});

test('reduced motion keeps countdown semantics without bubble animations', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const fixture = introCountdownFixture(multilingualLyricFixtures.find((item) => item.language === 'mandarin'));
  await mockApi(page, fixture);
  await page.goto('/');

  await page.locator('nav button').nth(1).click();
  await page.locator('.primary-search input').fill(fixture.query);
  await page.locator('button[type="submit"]').click();
  await page.getByRole('button', { name: new RegExp(escapeRegExp(fixture.title)) }).click();
  await page.locator('.dialog-actions .button-primary').first().click();
  const dialog = page.locator('.result-dialog');
  const firstBodyLine = fixture.document.lines.find((line) => line.words?.length);
  await seek(dialog, Math.max(0, firstBodyLine.start_ms - 1600));

  const dots = dialog.locator('.lyric-karaoke-meta-countdown .lyric-gap-dot');
  await expect(dots).toHaveCount(3);
  for (let index = 0; index < 3; index += 1) {
    await expect(dots.nth(index)).toHaveCSS('animation-name', 'none');
  }
});

test('karaoke lanes keep physical slots stable during line handoff', async ({ page }) => {
  const fixture = multilingualLyricFixtures.find((item) => item.language === 'mandarin');
  await mockApi(page, fixture);
  await page.goto('/');

  await page.locator('nav button').nth(1).click();
  await page.locator('.primary-search input').fill(fixture.query);
  await page.locator('button[type="submit"]').click();
  await page.getByRole('button', { name: new RegExp(escapeRegExp(fixture.title)) }).click();
  await page.locator('.dialog-actions .button-primary').first().click();
  const dialog = page.locator('.result-dialog');
  await expect(page.getByTestId('karaoke-stage')).toBeVisible();

  const bodyLines = fixture.document.lines.filter((line) => line.words?.length);
  await seek(dialog, bodyLines[0].start_ms + 500);
  const before = await laneSnapshot(dialog, true);
  expect(before.map((lane) => lane.slot)).toEqual(['top', 'bottom']);
  expect(before.map((lane) => lane.role)).toEqual(['active', 'upcoming']);

  await seek(dialog, bodyLines[1].start_ms + 80);
  const after = await laneSnapshot(dialog, false);
  expect(after.map((lane) => lane.role)).toEqual(['leaving', 'active']);
  expect(after.map((lane) => lane.sameNode)).toEqual([true, true]);
  expect(Math.abs(after[0].y - before[0].y)).toBeLessThanOrEqual(1);
  expect(Math.abs(after[1].y - before[1].y)).toBeLessThanOrEqual(1);
});

async function laneSnapshot(dialog, rememberNodes) {
  return dialog.locator('.lyric-karaoke-lane').evaluateAll((nodes, shouldRemember) => {
    if (shouldRemember) {
      window.__karaokeLaneNodes = nodes;
    }
    return nodes.map((node, index) => {
      return {
        slot: node.dataset.laneSlot,
        role: node.dataset.laneRole,
        y: node.offsetTop,
        sameNode: !window.__karaokeLaneNodes || window.__karaokeLaneNodes[index] === node,
      };
    });
  }, rememberNodes);
}

function introCountdownFixture(fixture) {
  return {
    ...fixture,
    document: {
      ...fixture.document,
      lines: fixture.document.lines.map((line, index) => (
        index === 0 ? line : { ...line, start_ms: line.start_ms + 4000 }
      )),
    },
  };
}

function longLineFixture(fixture) {
  const text = 'A deliberately extended multilingual karaoke sentence keeps every timed word readable without causing horizontal stage overflow';
  const words = text.split(' ').map((word, index) => ({
    text: `${index ? ' ' : ''}${word}`,
    offset_ms: index * 180,
    duration_ms: 240,
  }));
  return {
    ...fixture,
    document: {
      ...fixture.document,
      lines: [
        ...fixture.document.lines,
        {
          start_ms: 12000,
          duration_ms: words.length * 180 + 360,
          text,
          words,
        },
      ],
    },
  };
}

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
