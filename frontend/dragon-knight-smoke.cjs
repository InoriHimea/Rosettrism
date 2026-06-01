const { chromium } = require('@playwright/test');

(async () => {
  const browser = await chromium.launch({ headless: false });
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  try {
    await page.goto('http://127.0.0.1:5173/dragon-knight-smoke.html', { waitUntil: 'networkidle' });
    await page.waitForSelector('.lyric-playback-card');

    const normalized = await page.evaluate(() => ({
      title: window.__dragonKnightLyric.displayTitle,
      source: window.__dragonKnightLyric.source,
      inputFormat: window.__dragonKnightLyric.inputFormat,
      renderModeClass: document.querySelector('.lyric-stage')?.className,
      lineCount: window.__dragonKnightLyric.lines.length,
      annotationCount: window.__dragonKnightLyric.annotations.length,
      firstTimedWordLine: window.__dragonKnightLyric.lines.find((line) => !line.isMeta && line.words.length)?.text,
      annotatedLine: window.__dragonKnightLyric.lines.find((line) => line.text === '迎着风极速在超越'),
    }));
    if (!normalized.title.includes('龙战骑士')) {
      throw new Error(`displayTitle mismatch: ${normalized.title}`);
    }
    if (normalized.source !== 'Tencent' || normalized.inputFormat !== 'QRC') {
      throw new Error(`real QQ/QRC metadata lost: ${JSON.stringify(normalized)}`);
    }
    if (!normalized.renderModeClass.includes('lyric-stage-karaoke')) {
      throw new Error(`default mode should be karaoke: ${normalized.renderModeClass}`);
    }
    if (normalized.lineCount < 50 || normalized.annotationCount < 40) {
      throw new Error(`real Dragon Knight payload not loaded: ${JSON.stringify(normalized)}`);
    }
    if (normalized.firstTimedWordLine !== '词：方文山') {
      throw new Error(`top-level unified words not normalized: ${normalized.firstTimedWordLine}`);
    }
    if (!normalized.annotatedLine || normalized.annotatedLine.words.map((word) => word.text).join('') !== '迎着风极速在超越') {
      throw new Error(`annotated word line lost timing: ${JSON.stringify(normalized.annotatedLine)}`);
    }

    await page.evaluate(() => window.__setDragonKnightTime(51680));
    await page.waitForTimeout(260);

    const state = await page.evaluate(() => {
      const stage = document.querySelector('.lyric-stage');
      const list = document.querySelector('.lyric-karaoke-lines');
      const activeLine = document.querySelector('.lyric-line-active');
      const words = [...document.querySelectorAll('.lyric-line-active .lyric-word')].map((node) => ({
        text: node.querySelector('.lyric-progress-base')?.textContent || '',
        progress: Number(getComputedStyle(node).getPropertyValue('--lyric-progress')),
        fillEnd: getComputedStyle(node).getPropertyValue('--lyric-fill-end').trim(),
        rect: node.getBoundingClientRect().toJSON(),
      }));
      const ying = words.find((word) => word.text === '迎');
      const zhe = words.find((word) => word.text === '着');
      const feng = words.find((word) => word.text === '风');
      const stress = document.querySelector('.lyric-line-active .annotation-stress .annotation-glyph-text');
      const stressLabel = document.querySelector('.lyric-line-active .annotation-stress .lyric-annotation-label');
      return {
        stageClass: stage?.className,
        scrollTop: list?.scrollTop,
        scrollHeight: list?.scrollHeight,
        clientHeight: list?.clientHeight,
        activeOffsetTop: activeLine?.offsetTop,
        stageRect: stage?.getBoundingClientRect().toJSON(),
        activeText: activeLine?.innerText,
        activeRect: activeLine?.getBoundingClientRect().toJSON(),
        words,
        ying,
        zhe,
        feng,
        stressText: stress?.textContent,
        stressRect: stress?.getBoundingClientRect().toJSON(),
        stressLabelText: stressLabel?.textContent,
        stressLabelRect: stressLabel?.getBoundingClientRect().toJSON(),
      };
    });

    if (state.words.map((word) => word.text).join('') !== '迎着风极速在超越') {
      throw new Error(`active line mismatch: ${JSON.stringify(state.words)}`);
    }
    if (!(state.scrollTop > 250)) {
      throw new Error(`karaoke list did not scroll to active line: ${JSON.stringify({ scrollTop: state.scrollTop, scrollHeight: state.scrollHeight, clientHeight: state.clientHeight, activeOffsetTop: state.activeOffsetTop })}`);
    }
    const stageCenterY = state.stageRect.y + state.stageRect.height / 2;
    const activeCenterY = state.activeRect.y + state.activeRect.height / 2;
    if (Math.abs(stageCenterY - activeCenterY) > state.stageRect.height * 0.24) {
      throw new Error(`active karaoke line not centered: ${JSON.stringify({ stage: state.stageRect, active: state.activeRect })}`);
    }
    if (!state.ying || !state.zhe || !state.feng) {
      throw new Error(`missing active words: ${JSON.stringify(state.words)}`);
    }
    if (state.ying.progress !== 1) {
      throw new Error(`迎 should be complete, got ${state.ying.progress}`);
    }
    if (!(state.zhe.progress > 0 && state.zhe.progress < 1)) {
      throw new Error(`着 should be current, got ${state.zhe.progress}`);
    }
    if (state.zhe.fillEnd !== `${state.zhe.progress * 100}%`) {
      throw new Error(`word fill should be exact, got ${state.zhe.fillEnd} for ${state.zhe.progress}`);
    }
    if (state.feng.progress !== 0) {
      throw new Error(`风 should be future, got ${state.feng.progress}`);
    }
    if (state.stressText !== '·' || state.stressLabelText !== '重音') {
      throw new Error(`stress label/glyph mismatch: ${state.stressText} / ${state.stressLabelText}`);
    }

    const yingCenter = state.ying.rect.x + state.ying.rect.width / 2;
    const stressCenter = state.stressRect.x + state.stressRect.width / 2;
    if (Math.abs(stressCenter - yingCenter) > Math.max(10, state.ying.rect.width * 0.35)) {
      throw new Error(`stress mark not centered on 迎: ${JSON.stringify({ ying: state.ying.rect, stress: state.stressRect })}`);
    }
    if (state.stressRect.width < state.ying.rect.width * 0.35) {
      throw new Error(`stress mark too small: ${JSON.stringify({ ying: state.ying.rect, stress: state.stressRect })}`);
    }
    if (state.stressRect.y <= state.ying.rect.y + state.ying.rect.height * 0.45) {
      throw new Error(`stress mark should sit below lower half of 迎: ${JSON.stringify({ ying: state.ying.rect, stress: state.stressRect })}`);
    }
    if (state.stressLabelRect.y >= state.ying.rect.y) {
      throw new Error(`stress label should sit above 迎: ${JSON.stringify({ ying: state.ying.rect, label: state.stressLabelRect })}`);
    }

    console.log('dragon knight visual smoke ok', JSON.stringify({
      title: normalized.title,
      source: normalized.source,
      inputFormat: normalized.inputFormat,
      scrollTop: state.scrollTop,
      activeText: state.activeText,
      progress: state.words.map((word) => ({ text: word.text, progress: word.progress, fillEnd: word.fillEnd })),
      ying: state.ying.rect,
      stress: state.stressRect,
    }, null, 2));
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
