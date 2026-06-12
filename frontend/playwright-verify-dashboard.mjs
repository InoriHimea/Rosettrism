import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const BASE = 'http://127.0.0.1:5182';
const ARTIFACT_DIR = resolve(SCRIPT_DIR, 'playwright-artifacts');

const populatedCache = [
  { id: 1, source: 'qq', operation: 'fetch', fresh: true, body_len: 9200, title: '龙战骑士', artist: '周杰伦' },
  { id: 2, source: 'netease', operation: 'search', fresh: true, body_len: 4200, query: '晴天' },
  { id: 3, source: 'qq', operation: 'search', fresh: true, body_len: 3600, query: '搁浅' },
  { id: 4, source: 'kugou', operation: 'fetch', fresh: false, body_len: 2800, title: '一路向北' },
  { id: 5, source: 'lrclib', operation: 'fetch', fresh: true, body_len: 1800, title: '夜曲' },
  { id: 6, source: 'qq', operation: 'fetch', fresh: true, body_len: 6200, title: '以父之名' },
  { id: 7, source: 'migu', operation: 'search', fresh: false, body_len: 1200, query: '反方向的钟' },
  { id: 8, source: 'utaten', operation: 'fetch', fresh: true, body_len: 3000, title: 'Lemon' },
];

function startVite() {
  const isWindows = process.platform === 'win32';
  const command = isWindows ? 'cmd.exe' : 'npm';
  const args = isWindows
    ? ['/d', '/s', '/c', 'npm run dev -- --port 5182 --strictPort']
    : ['run', 'dev', '--', '--port', '5182', '--strictPort'];
  const child = spawn(
    command,
    args,
    { cwd: SCRIPT_DIR, stdio: ['ignore', 'pipe', 'pipe'] },
  );
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
  throw new Error('Dashboard dev server did not start in time');
}

function requirePass(condition, message, sample = {}) {
  if (!condition) {
    throw new Error(`${message} ${JSON.stringify(sample)}`);
  }
}

async function installRoutes(page, scenarioRef) {
  await page.route('**/api/**', (route) => {
    const path = new URL(route.request().url()).pathname;
    const cache = scenarioRef.current === 'empty' ? [] : populatedCache;
    const fresh = cache.filter((entry) => entry.fresh).length;
    const expired = Math.max(cache.length - fresh, 0);

    if (path === '/api/health') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, version: 'dashboard-test' }) });
    }
    if (path === '/api/stats') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ cache: { upstream_entries: cache.length, fresh_upstream_entries: fresh, expired_upstream_entries: expired } }),
      });
    }
    if (path === '/api/cache') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ entries: cache }) });
    }
    return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
  });
}

async function dashboardSnapshot(page) {
  return page.evaluate(() => {
    const box = (selector) => {
      const node = document.querySelector(selector);
      if (!node) return null;
      const rect = node.getBoundingClientRect();
      return { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) };
    };
    const visible = (selector) => [...document.querySelectorAll(selector)]
      .filter((node) => {
        const rect = node.getBoundingClientRect();
        return rect.width > 0 && rect.height > 0 && rect.bottom > 0 && rect.top < window.innerHeight;
      })
      .map((node) => ({
        text: node.textContent.trim().replace(/\s+/g, ' ').slice(0, 140),
        className: node.className,
        box: (() => {
          const rect = node.getBoundingClientRect();
          return { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) };
        })(),
      }));
    const overflowing = [...document.querySelectorAll('body *')]
      .filter((node) => node.scrollWidth > node.clientWidth + 1 && getComputedStyle(node).overflowX !== 'hidden')
      .slice(0, 8)
      .map((node) => ({ tag: node.tagName, className: node.className, text: node.textContent.trim().slice(0, 80), scrollWidth: node.scrollWidth, clientWidth: node.clientWidth }));
    return {
      viewport: { width: window.innerWidth, height: window.innerHeight },
      brief: box('.dashboard-brief'),
      health: box('.health-panel'),
      totalEmptyStates: document.querySelectorAll('.rich-empty-state').length,
      panels: visible('.dashboard-grid > article'),
      miniStats: visible('.mini-stat'),
      emptyStates: visible('.rich-empty-state'),
      sparkColumns: visible('.spark-column'),
      overflowing,
    };
  });
}

mkdirSync(ARTIFACT_DIR, { recursive: true });

const server = startVite();
let browser;
try {
  await waitForServer();
  browser = await chromium.launch({ headless: true });
  const scenarioRef = { current: 'populated' };
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  await installRoutes(page, scenarioRef);

  await page.goto(BASE);
  await page.waitForSelector('.dashboard-brief');
  const desktop = await dashboardSnapshot(page);
  requirePass(desktop.panels.length >= 5, 'DESKTOP_DASHBOARD_PANELS_MISSING', desktop);
  requirePass(desktop.brief && desktop.health && desktop.brief.y < desktop.health.y, 'DESKTOP_DASHBOARD_BRIEF_NOT_ABOVE_HEALTH', desktop);
  requirePass(desktop.miniStats.length >= 3, 'DESKTOP_DASHBOARD_MINI_STATS_MISSING', desktop);
  requirePass(desktop.sparkColumns.length >= 6, 'DESKTOP_RECENT_CACHE_COLUMNS_MISSING', desktop);
  requirePass(desktop.overflowing.length === 0, 'DESKTOP_DASHBOARD_TEXT_OVERFLOW', desktop);
  await page.screenshot({ path: resolve(ARTIFACT_DIR, 'verify-dashboard-desktop.png'), fullPage: false });

  scenarioRef.current = 'empty';
  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await page.waitForSelector('.dashboard-brief');
  const mobile = await dashboardSnapshot(page);
  requirePass(mobile.brief && mobile.health, 'MOBILE_DASHBOARD_PRIMARY_MODULES_MISSING', mobile);
  requirePass(mobile.brief.y < 340, 'MOBILE_DASHBOARD_NAV_TOO_TALL', mobile);
  requirePass(mobile.health.y < 620, 'MOBILE_DASHBOARD_HEALTH_NOT_IN_FIRST_SCREEN', mobile);
  requirePass(mobile.totalEmptyStates >= 3, 'MOBILE_DASHBOARD_EMPTY_STATES_MISSING', mobile);
  requirePass(mobile.panels.length >= 3, 'MOBILE_DASHBOARD_AUXILIARY_MODULE_NOT_VISIBLE', mobile);
  requirePass(mobile.overflowing.length === 0, 'MOBILE_DASHBOARD_TEXT_OVERFLOW', mobile);
  await page.screenshot({ path: resolve(ARTIFACT_DIR, 'verify-dashboard-mobile.png'), fullPage: false });

  console.log('DASHBOARD_DESKTOP:', JSON.stringify(desktop));
  console.log('DASHBOARD_MOBILE:', JSON.stringify(mobile));
} finally {
  if (browser) {
    await browser.close();
  }
  server.child.kill();
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  if (server.child.exitCode === null) {
    server.child.kill('SIGKILL');
  }
}
