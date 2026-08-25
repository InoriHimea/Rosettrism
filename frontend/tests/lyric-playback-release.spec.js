import fs from 'node:fs';
import { expect, test } from '@playwright/test';

const realDragonKnight = JSON.parse(fs.readFileSync(new URL('../dragon-knight-real.json', import.meta.url), 'utf8'));
const realFirstLine = {
  startMs: 49335,
  endMs: 51414,
  text: '放手一搏令谁都惭愧',
};
const realSecondLine = {
  startMs: 51414,
  text: '迎着风极速在超越',
};

test('real Dragon Knight QQ/QRC data keeps intro, word progress, annotations, and handoff deterministic', async ({ page }) => {
  await openHarness(page, realDragonKnight);
  const card = page.locator('.lyric-playback-card');
  await expect(card.locator('.lyric-playback-head h4')).toContainText('龙战骑士');
  await expect(page.getByTestId('lyric-quality-status')).toHaveText('逐字同步');

  await seek(page, realFirstLine.startMs - 1200);
  await expect(card).toHaveAttribute('data-playback-phase', /metadata|countdown|interlude/);

  await seek(page, realFirstLine.startMs + 350);
  await expect(card).toHaveAttribute('data-playback-phase', 'singing');
  await expectBaseText(card.locator('.lyric-line-active'), realFirstLine.text);
  await expect(card.locator('.lyric-line-active .lyric-annotation-mark').first()).toBeVisible();
  const earlyProgress = await lineProgress(card.locator('.lyric-line-active'));

  await seek(page, realFirstLine.endMs - 80);
  await expectBaseText(card.locator('.lyric-line-active'), realFirstLine.text);
  const lateProgress = await lineProgress(card.locator('.lyric-line-active'));
  expect(lateProgress).toBeGreaterThan(earlyProgress);

  await seek(page, realSecondLine.startMs + 80);
  await expectBaseText(card.locator('.lyric-line-active'), realSecondLine.text);
  await expect(card.locator('.lyric-karaoke-lane')).toHaveCount(2);
  if (process.env.PLAYBACK_EVIDENCE_DIR) {
    await card.screenshot({
      path: `${process.env.PLAYBACK_EVIDENCE_DIR}/dragon-knight-after-desktop-1280x720.png`,
      animations: 'disabled',
    });
  }
});

test('plain line-timed lyrics degrade to stable whole-line playback', async ({ page }) => {
  const payload = {
    document: {
      meta: { title: '普通逐行歌词', artist: '测试歌手', source: 'fixture', input_format: 'lrc' },
      lines: [
        { start_ms: 1000, duration_ms: 2500, text: '第一句普通逐行歌词', words: [] },
        { start_ms: 4000, duration_ms: 2500, text: '第二句没有逐字时间', words: [] },
      ],
    },
  };
  await openHarness(page, payload);
  const card = page.locator('.lyric-playback-card');
  await expect(page.getByTestId('lyric-quality-status')).toHaveText('逐行同步');

  await seek(page, 1800);
  await expect(card).toHaveAttribute('data-playback-phase', 'singing');
  await expectBaseText(card.locator('.lyric-line-active'), '第一句普通逐行歌词');
  const progress = await lineProgress(card.locator('.lyric-line-active'));
  expect(progress).toBeGreaterThan(0);
  expect(progress).toBeLessThan(1);

  await seek(page, 4200);
  await expectBaseText(card.locator('.lyric-line-active'), '第二句没有逐字时间');
});

test('raw lyric text without timing shows an explicit non-playable fallback', async ({ page }) => {
  await openHarness(page, {
    format: 'raw',
    raw: '这是一段没有时间标签的歌词\n只展示文本，不挂载播放状态机',
  }, false);

  await expect(page.getByTestId('playback-fallback')).toBeVisible();
  await expect(page.getByTestId('playback-fallback')).toContainText('暂无可播放时间轴');
  await expect(page.getByTestId('playback-fallback')).toContainText('没有时间标签');
  await expect(page.locator('.lyric-playback-card')).toHaveCount(0);
});

test('missing explicit line timing cannot mount a fake synchronized timeline', async ({ page }) => {
  await openHarness(page, {
    document: {
      meta: { title: '缺失时间戳' },
      lines: [{ text: '不能默认为 0ms', words: [] }],
    },
  }, false);

  await expect(page.getByTestId('playback-fallback')).toBeVisible();
  await expect(page.locator('.lyric-playback-card')).toHaveCount(0);
});

test('real QQ/QRC stage stays within mobile viewport and keeps 44px controls', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openHarness(page, realDragonKnight);
  await seek(page, realFirstLine.startMs + 600);
  if (process.env.PLAYBACK_EVIDENCE_DIR) {
    await page.locator('.lyric-playback-card').screenshot({
      path: `${process.env.PLAYBACK_EVIDENCE_DIR}/dragon-knight-after-mobile-390x844.png`,
      animations: 'disabled',
    });
  }

  const geometry = await page.evaluate(() => {
    const stage = document.querySelector('.lyric-stage');
    const active = document.querySelector('.lyric-line-active');
    const controls = [...document.querySelectorAll('.lyric-controls button')];
    return {
      documentOverflow: Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) - window.innerWidth,
      stageOverflow: stage ? stage.scrollWidth - stage.clientWidth : 999,
      activeOverflow: active ? active.scrollWidth - active.clientWidth : 999,
      controlHeights: controls.map((node) => node.getBoundingClientRect().height),
    };
  });
  expect(geometry.documentOverflow).toBeLessThanOrEqual(2);
  expect(geometry.stageOverflow).toBeLessThanOrEqual(2);
  expect(geometry.activeOverflow).toBeLessThanOrEqual(2);
  expect(Math.min(...geometry.controlHeights)).toBeGreaterThanOrEqual(44);
});

async function openHarness(page, payload, expectPlayable = true) {
  await page.addInitScript((injectedPayload) => {
    window.__LYRIC_PLAYBACK_HARNESS__ = {
      payload: injectedPayload,
      settings: {
        ambientEffects: false,
        lowDistraction: true,
        stage3D: false,
      },
    };
  }, payload);
  await page.goto('/playback-harness.html');
  if (expectPlayable) {
    await expect(page.locator('.lyric-playback-card')).toBeVisible();
  }
}

async function seek(page, ms) {
  await page.evaluate((value) => window.__LYRIC_PLAYBACK_HARNESS_API__.seek(value), ms);
  await expect(page.locator('.lyric-playback-card')).toHaveAttribute('data-playback-time-ms', String(ms));
}

async function expectBaseText(line, expected) {
  const actual = await line.locator('.lyric-progress-base').evaluateAll((nodes) => (
    nodes.map((node) => node.textContent || '').join('')
  ));
  expect(actual).toBe(expected);
}

async function lineProgress(line) {
  return line.locator('.lyric-progress-text').evaluateAll((nodes) => {
    const values = nodes.map((node) => (
      Number.parseFloat(getComputedStyle(node).getPropertyValue('--lyric-progress'))
    ));
    return values.reduce((total, value) => total + value, 0) / Math.max(1, values.length);
  });
}
