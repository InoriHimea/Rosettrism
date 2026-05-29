const { chromium } = require('@playwright/test');

(async () => {
  const browser = await chromium.launch({ headless: false });
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  try {
    await page.goto('http://127.0.0.1:5173/dragon-knight-smoke.html', { waitUntil: 'networkidle' });
    await page.waitForSelector('.lyric-playback-card');

    const normalized = await page.evaluate(() => ({
      title: window.__dragonKnightLyric.displayTitle,
      lines: window.__dragonKnightLyric.lines.map((line) => ({
        text: line.text,
        isMeta: line.isMeta,
        words: line.words.map((word) => word.text),
      })),
    }));
    if (normalized.title !== '龙战骑士 - 周杰伦 (Jay Chou)') {
      throw new Error(`displayTitle mismatch: ${normalized.title}`);
    }
    if (normalized.lines[1].isMeta || normalized.lines[1].words.join('') !== '久晴天') {
      throw new Error(`body word line lost timing: ${JSON.stringify(normalized.lines[1])}`);
    }

    await page.evaluate(() => window.__setDragonKnightTime(17680));
    await page.waitForTimeout(120);

    const state = await page.evaluate(() => {
      const words = [...document.querySelectorAll('.lyric-line-active .lyric-word')].map((node) => ({
        text: node.querySelector('.lyric-progress-base')?.textContent || '',
        progress: Number(getComputedStyle(node).getPropertyValue('--lyric-progress')),
        fillEnd: getComputedStyle(node).getPropertyValue('--lyric-fill-end').trim(),
        rect: node.getBoundingClientRect().toJSON(),
      }));
      const qing = words.find((word) => word.text === '晴');
      const jiu = words.find((word) => word.text === '久');
      const tian = words.find((word) => word.text === '天');
      const stress = document.querySelector('.lyric-line-active .annotation-stress .annotation-glyph-text');
      const stressLabel = document.querySelector('.lyric-line-active .annotation-stress .lyric-annotation-label');
      const longTone = document.querySelector('.lyric-line-active .annotation-long-tone .annotation-glyph-text');
      return {
        words,
        qing,
        jiu,
        tian,
        stressText: stress?.textContent,
        stressRect: stress?.getBoundingClientRect().toJSON(),
        stressLabelText: stressLabel?.textContent,
        stressLabelRect: stressLabel?.getBoundingClientRect().toJSON(),
        longToneRect: longTone?.getBoundingClientRect().toJSON(),
      };
    });

    if (!state.qing || !state.jiu || !state.tian) {
      throw new Error(`missing active words: ${JSON.stringify(state.words)}`);
    }
    if (state.jiu.progress !== 1) {
      throw new Error(`久 should be complete, got ${state.jiu.progress}`);
    }
    if (!(state.qing.progress > 0 && state.qing.progress < 1)) {
      throw new Error(`晴 should be current, got ${state.qing.progress}`);
    }
    if (state.qing.fillEnd !== `${state.qing.progress * 100}%`) {
      throw new Error(`word fill should be exact, got ${state.qing.fillEnd} for ${state.qing.progress}`);
    }
    if (state.tian.progress !== 0) {
      throw new Error(`天 should be future, got ${state.tian.progress}`);
    }
    if (state.stressText !== '·' || state.stressLabelText !== '重音') {
      throw new Error(`stress label/glyph mismatch: ${state.stressText} / ${state.stressLabelText}`);
    }

    const qingCenter = state.qing.rect.x + state.qing.rect.width / 2;
    const stressCenter = state.stressRect.x + state.stressRect.width / 2;
    const jiuCenter = state.jiu.rect.x + state.jiu.rect.width / 2;
    if (Math.abs(stressCenter - qingCenter) > Math.max(10, state.qing.rect.width * 0.3)) {
      throw new Error(`stress mark not centered on 晴: ${JSON.stringify({ qing: state.qing.rect, stress: state.stressRect })}`);
    }
    if (stressCenter <= jiuCenter + 8) {
      throw new Error(`stress mark drifted toward sentence start: ${JSON.stringify({ jiu: state.jiu.rect, stress: state.stressRect })}`);
    }
    if (state.stressRect.width < state.qing.rect.width * 0.35) {
      throw new Error(`stress mark too small: ${JSON.stringify({ qing: state.qing.rect, stress: state.stressRect })}`);
    }
    if (state.stressRect.y <= state.qing.rect.y + state.qing.rect.height * 0.45) {
      throw new Error(`stress mark should sit below lower half of 晴: ${JSON.stringify({ qing: state.qing.rect, stress: state.stressRect })}`);
    }
    if (state.stressLabelRect.y >= state.qing.rect.y) {
      throw new Error(`stress label should sit above 晴: ${JSON.stringify({ qing: state.qing.rect, label: state.stressLabelRect })}`);
    }

    console.log('dragon knight visual smoke ok', JSON.stringify({
      title: normalized.title,
      progress: state.words.map((word) => ({ text: word.text, progress: word.progress, fillEnd: word.fillEnd })),
      qing: state.qing.rect,
      stress: state.stressRect,
    }, null, 2));
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
