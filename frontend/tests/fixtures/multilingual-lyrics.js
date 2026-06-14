export const multilingualLyricFixtures = [
  {
    id: 'mandarin-dragon-knight',
    language: 'mandarin',
    query: '龙战骑士 周杰伦',
    title: '龙战骑士',
    artist: '周杰伦',
    source: 'qq',
    inputFormat: 'qrc',
    searchExtra: {
      artist_alias: 'Jay Chou',
      has_singing_annotations: true,
      singing_annotations: [
        { annotation_type: 'breath', start_ms: 1180, duration_ms: 260, text: '我' },
        { annotation_type: 'stress', start_ms: 1760, duration_ms: 260, text: '坚' },
        { annotation_type: 'long_tone', start_ms: 2360, duration_ms: 520, text: '决' },
      ],
    },
    document: {
      meta: { title: '龙战骑士', artist: '周杰伦', source: 'qq', input_format: 'qrc' },
      lines: [
        {
          start_ms: 0,
          duration_ms: 900,
          text: '龙战骑士 - 周杰伦',
          words: [],
        },
        {
          start_ms: 1200,
          duration_ms: 3200,
          text: '中文逐字测试线',
          words: [
            { text: '中', offset_ms: 0, duration_ms: 420 },
            { text: '文', offset_ms: 420, duration_ms: 420 },
            { text: '逐', offset_ms: 840, duration_ms: 520 },
            { text: '字', offset_ms: 1420, duration_ms: 440 },
            { text: '测试', offset_ms: 1860, duration_ms: 620 },
            { text: '线', offset_ms: 2480, duration_ms: 560 },
          ],
        },
        {
          start_ms: 6400,
          duration_ms: 2600,
          text: '助唱标注保持对齐',
          words: [
            { text: '助唱', offset_ms: 0, duration_ms: 520 },
            { text: '标注', offset_ms: 520, duration_ms: 560 },
            { text: '保持', offset_ms: 1080, duration_ms: 620 },
            { text: '对齐', offset_ms: 1700, duration_ms: 680 },
          ],
        },
      ],
    },
  },
  {
    id: 'cantonese-boundless-ocean',
    language: 'cantonese',
    query: '海阔天空 Beyond',
    title: '海阔天空',
    artist: 'Beyond',
    source: 'qq',
    inputFormat: 'qrc',
    searchExtra: { artist_alias: 'Beyond' },
    document: {
      meta: { title: '海阔天空', artist: 'Beyond', source: 'qq', input_format: 'qrc' },
      lines: [
        { start_ms: 0, duration_ms: 1000, text: '海阔天空 - Beyond', words: [] },
        { start_ms: 1200, duration_ms: 900, text: '作词：黄家驹', words: [] },
        {
          start_ms: 2800,
          duration_ms: 3200,
          text: '粤语排版测试线',
          reading: 'jyut jyu paai baan caak si sin',
          words: [
            { text: '粤语', offset_ms: 0, duration_ms: 700 },
            { text: '排版', offset_ms: 700, duration_ms: 700 },
            { text: '测试', offset_ms: 1400, duration_ms: 700 },
            { text: '线', offset_ms: 2100, duration_ms: 700 },
          ],
        },
        {
          start_ms: 8200,
          duration_ms: 3200,
          text: '海阔天空',
          romanized: 'hoi fut tin hung',
          words: [
            { text: '海', offset_ms: 0, duration_ms: 700 },
            { text: '阔', offset_ms: 700, duration_ms: 700 },
            { text: '天', offset_ms: 1400, duration_ms: 700 },
            { text: '空', offset_ms: 2100, duration_ms: 700 },
          ],
        },
      ],
    },
  },
  {
    id: 'japanese-blue-bird',
    language: 'japanese',
    query: 'ブルーバード いきものがかり',
    title: 'ブルーバード',
    artist: 'いきものがかり',
    source: 'utaten',
    inputFormat: 'json',
    searchExtra: { artist_alias: 'Ikimonogakari' },
    document: {
      meta: { title: 'ブルーバード', artist: 'いきものがかり', source: 'utaten', input_format: 'json' },
      lines: [
        { start_ms: 0, duration_ms: 1000, text: 'ブルーバード - いきものがかり', words: [] },
        {
          start_ms: 1600,
          duration_ms: 3600,
          text: '青い空へ進む',
          romanized: 'aoi sora e susumu',
          ruby: [
            { start_char: 0, end_char: 1, text: '青', reading: 'あお' },
            { start_char: 2, end_char: 3, text: '空', reading: 'そら' },
          ],
          words: [],
        },
        {
          start_ms: 7000,
          duration_ms: 3200,
          text: 'かな表示を確認',
          romanized: 'kana hyouji o kakunin',
          words: [
            { text: 'かな', offset_ms: 0, duration_ms: 800 },
            { text: '表示を', offset_ms: 800, duration_ms: 1000 },
            { text: '確認', offset_ms: 1800, duration_ms: 900 },
          ],
        },
      ],
    },
  },
  {
    id: 'english-last-dance',
    language: 'english',
    query: 'Just One Last Dance Sarah Connor',
    title: 'Just One Last Dance',
    artist: 'Sarah Connor',
    source: 'lrclib',
    inputFormat: 'lrc',
    searchExtra: {},
    document: {
      meta: { title: 'Just One Last Dance', artist: 'Sarah Connor', source: 'lrclib', input_format: 'lrc' },
      lines: [
        { start_ms: 0, duration_ms: 1000, text: 'Just One Last Dance - Sarah Connor', words: [] },
        {
          start_ms: 1300,
          duration_ms: 3300,
          text: 'English timing sample',
          words: [
            { text: 'English', offset_ms: 0, duration_ms: 720 },
            { text: 'timing', offset_ms: 720, duration_ms: 680 },
            { text: 'sample', offset_ms: 1400, duration_ms: 720 },
          ],
        },
        {
          start_ms: 6500,
          duration_ms: 3200,
          text: 'Long Latin words stay readable',
          words: [
            { text: 'Long', offset_ms: 0, duration_ms: 480 },
            { text: 'Latin', offset_ms: 480, duration_ms: 520 },
            { text: 'words', offset_ms: 1000, duration_ms: 520 },
            { text: 'stay', offset_ms: 1520, duration_ms: 480 },
            { text: 'readable', offset_ms: 2000, duration_ms: 780 },
          ],
        },
      ],
    },
  },
];

export function searchResultForFixture(fixture) {
  return {
    source: fixture.source,
    id: fixture.id,
    title: fixture.title,
    artist: fixture.artist,
    duration_ms: 180000,
    extra: fixture.searchExtra,
  };
}

export function fetchResultForFixture(fixture) {
  return {
    document: fixture.document,
    selectedEntry: searchResultForFixture(fixture),
  };
}
