import { expect, test } from '@playwright/test';
import { multilingualLyricFixtures } from './fixtures/multilingual-lyrics.js';

const baseFixture = multilingualLyricFixtures.find((item) => item.language === 'mandarin');
const viewports = [
  { width: 390, height: 844 },
  { width: 768, height: 1024 },
  { width: 1280, height: 720 },
  { width: 1440, height: 900 },
];

test('fixed clock harness exposes deterministic lyric states at benchmark checkpoints', async ({ page }) => {
  const payload = payloadForFixture(baseFixture);
  await openHarness(page, payload);
  const card = page.locator('.lyric-playback-card');
  const bodyLines = baseFixture.document.lines.filter((line) => line.words?.length);
  const checkpoints = [
    { ms: 0, phase: /metadata|countdown|interlude/ },
    { ms: bodyLines[0].start_ms - 80, phase: /countdown|interlude/ },
    { ms: bodyLines[0].start_ms + 300, phase: 'singing', activeText: bodyLines[0].text },
    { ms: bodyLines[0].start_ms + bodyLines[0].duration_ms - 20, phase: 'singing', activeText: bodyLines[0].text },
    { ms: bodyLines[1].start_ms + 20, phase: 'singing', activeText: bodyLines[1].text },
    { ms: bodyLines.at(-1).start_ms + bodyLines.at(-1).duration_ms + 300, phase: 'ended' },
  ];

  const durationMs = await page.evaluate(() => window.__LYRIC_PLAYBACK_HARNESS_API__.metrics().durationMs);
  for (const checkpoint of checkpoints) {
    await harnessCall(page, 'seek', checkpoint.ms);
    await expect(card).toHaveAttribute('data-playback-time-ms', String(Math.min(checkpoint.ms, durationMs)));
    await expect(card).toHaveAttribute('data-playback-phase', checkpoint.phase);
    if (checkpoint.activeText) {
      const activeLine = card.locator('.lyric-line-active');
      const baseText = await activeLine.locator('.lyric-progress-base').evaluateAll((nodes) => (
        nodes.map((node) => node.textContent || '').join('')
      ));
      expect(baseText).toBe(checkpoint.activeText);
      const progress = await activeLine.locator('.lyric-progress-text').first().evaluate((node) => (
        Number.parseFloat(getComputedStyle(node).getPropertyValue('--lyric-progress'))
      ));
      expect(progress).toBeGreaterThanOrEqual(0);
      expect(progress).toBeLessThanOrEqual(1);
    }
  }
});

for (const viewport of viewports) {
  test(`fixed harness remains stable at ${viewport.width}x${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await openHarness(page, payloadForFixture(baseFixture));
    const firstBodyLine = baseFixture.document.lines.find((line) => line.words?.length);
    await harnessCall(page, 'seek', firstBodyLine.start_ms + 600);

    const geometry = await page.evaluate(() => {
      const card = document.querySelector('.lyric-playback-card');
      const stage = document.querySelector('.lyric-stage');
      const active = document.querySelector('.lyric-line-active');
      const controls = [...document.querySelectorAll('.lyric-controls button')];
      return {
        documentOverflow: Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) - window.innerWidth,
        cardOverflow: card ? card.scrollWidth - card.clientWidth : 999,
        stageOverflow: stage ? stage.scrollWidth - stage.clientWidth : 999,
        activeOverflow: active ? active.scrollWidth - active.clientWidth : 999,
        controlHeights: controls.map((node) => node.getBoundingClientRect().height),
      };
    });

    expect(geometry.documentOverflow).toBeLessThanOrEqual(2);
    expect(geometry.cardOverflow).toBeLessThanOrEqual(2);
    expect(geometry.stageOverflow).toBeLessThanOrEqual(2);
    expect(geometry.activeOverflow).toBeLessThanOrEqual(2);
    expect(Math.min(...geometry.controlHeights)).toBeGreaterThanOrEqual(44);

    await expect(page.locator('.lyric-playback-card')).toHaveScreenshot(
      `playback-${viewport.width}x${viewport.height}-at-1800ms.png`,
      { animations: 'disabled', caret: 'hide' },
    );
  });
}

test('200-line fixture keeps fixed-clock updates bounded and avoids lyric long tasks', async ({ page }) => {
  const fixture = longFixture(200);
  await openHarness(page, payloadForFixture(fixture));
  await installLongTaskObserver(page);
  await harnessCall(page, 'clearMetrics');

  const checkpoints = Array.from({ length: 60 }, (_, index) => index * 1000);
  const updateCosts = [];
  const startedAt = Date.now();
  for (const ms of checkpoints) {
    const updateStartedAt = performance.now();
    await harnessCall(page, 'seek', ms);
    updateCosts.push(performance.now() - updateStartedAt);
  }
  const elapsedMs = Date.now() - startedAt;
  const report = await page.evaluate(() => ({
    metrics: window.__LYRIC_PLAYBACK_HARNESS_API__.metrics(),
    longTasks: window.__lyricLongTasks || [],
  }));

  expect(report.metrics.currentMs).toBe(59000);
  expect(report.metrics.commits.length).toBeLessThanOrEqual(70);
  expect(Math.max(0, ...report.metrics.commits.map((entry) => entry.actualDuration))).toBeLessThan(100);
  expect(report.longTasks.filter((entry) => entry.duration > 100)).toEqual([]);
  expect(Math.max(...updateCosts)).toBeLessThan(100);
  expect(elapsedMs).toBeLessThan(10000);
});

test('200-line frame-state checkpoint cost does not grow with timeline position', async ({ page }) => {
  const fixture = longFixture(200);
  await openHarness(page, payloadForFixture(fixture));
  const lines = fixture.document.lines.filter((line) => line.words?.length);
  const sampleIndexes = [5, 50, 100, 150, 195];
  const costs = [];

  for (const index of sampleIndexes) {
    await harnessCall(page, 'clearMetrics');
    const line = lines[index];
    const start = performance.now();
    await harnessCall(page, 'seek', line.start_ms + 200);
    costs.push(performance.now() - start);
  }

  const firstHalf = Math.max(...costs.slice(0, 2));
  const lastHalf = Math.max(...costs.slice(-2));
  expect(lastHalf).toBeLessThan(Math.max(120, firstHalf * 4));
});

async function openHarness(page, payload, settings = {}) {
  await page.addInitScript(({ injectedPayload, injectedSettings }) => {
    window.__LYRIC_PLAYBACK_HARNESS__ = {
      payload: injectedPayload,
      settings: {
        ambientEffects: false,
        lowDistraction: true,
        stage3D: false,
        ...injectedSettings,
      },
    };
  }, { injectedPayload: payload, injectedSettings: settings });
  await page.goto('/playback-harness.html');
  await expect(page.locator('.lyric-playback-card')).toBeVisible();
}

async function harnessCall(page, method, value) {
  await page.evaluate(({ apiMethod, apiValue }) => {
    window.__LYRIC_PLAYBACK_HARNESS_API__[apiMethod](apiValue);
  }, { apiMethod: method, apiValue: value });
}

async function installLongTaskObserver(page) {
  await page.evaluate(() => {
    window.__lyricLongTasks = [];
    if (!PerformanceObserver.supportedEntryTypes?.includes('longtask')) {
      return;
    }
    const observer = new PerformanceObserver((list) => {
      window.__lyricLongTasks.push(...list.getEntries().map((entry) => ({
        duration: entry.duration,
        startTime: entry.startTime,
      })));
    });
    observer.observe({ type: 'longtask', buffered: true });
    window.__lyricLongTaskObserver = observer;
  });
}

function payloadForFixture(fixture) {
  return {
    document: fixture.document,
    selectedEntry: {
      source: fixture.source,
      id: fixture.id,
      title: fixture.title,
      artist: fixture.artist,
      duration_ms: fixture.document.lines.at(-1).start_ms + fixture.document.lines.at(-1).duration_ms + 1000,
      extra: fixture.searchExtra,
    },
  };
}

function longFixture(lineCount) {
  const lineDuration = 900;
  const gap = 300;
  const lines = Array.from({ length: lineCount }, (_, index) => {
    const text = `第${index + 1}行固定时钟性能歌词`;
    return {
      start_ms: index * (lineDuration + gap),
      duration_ms: lineDuration,
      text,
      words: [
        { text: `第${index + 1}行`, offset_ms: 0, duration_ms: 300 },
        { text: '固定时钟', offset_ms: 300, duration_ms: 300 },
        { text: '性能歌词', offset_ms: 600, duration_ms: 300 },
      ],
    };
  });
  return {
    id: 'benchmark-200-lines',
    language: 'mandarin',
    query: 'benchmark',
    title: '200 行性能歌词',
    artist: 'Rosettrism',
    source: 'fixture',
    inputFormat: 'json',
    searchExtra: {},
    document: {
      meta: { title: '200 行性能歌词', artist: 'Rosettrism', source: 'fixture', input_format: 'json' },
      lines,
    },
  };
}
