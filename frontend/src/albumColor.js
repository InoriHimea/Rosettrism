// 封面取色：量化主色 + 邻近色，驱动歌词填充渐变。
// 纯函数核心 pickDominantColors 可单测；extractAlbumColors 负责图片解码采样。

const SAMPLE_SIZE = 32;

export function pickDominantColors(pixels) {
  // pixels: Uint8ClampedArray RGBA
  const buckets = new Map();
  const count = Math.floor(pixels.length / 4);
  for (let i = 0; i < count; i += 1) {
    const r = pixels[i * 4];
    const g = pixels[i * 4 + 1];
    const b = pixels[i * 4 + 2];
    const a = pixels[i * 4 + 3];
    if (a < 128) {
      continue;
    }
    const key = `${r >> 5}-${g >> 5}-${b >> 5}`;
    const bucket = buckets.get(key) || { r: 0, g: 0, b: 0, n: 0 };
    bucket.r += r;
    bucket.g += g;
    bucket.b += b;
    bucket.n += 1;
    buckets.set(key, bucket);
  }
  if (!buckets.size) {
    return null;
  }
  const ranked = [...buckets.values()]
    .map((bucket) => ({ r: bucket.r / bucket.n, g: bucket.g / bucket.n, b: bucket.b / bucket.n, n: bucket.n }))
    .sort((a, b) => b.n - a.n);
  const primary = normalizeTone(ranked[0]);
  let secondarySource = ranked.find((candidate, index) => (
    index > 0 && colorDistance(candidate, ranked[0]) > 64
  )) || ranked[1] || ranked[0];
  const secondary = normalizeTone(secondarySource, ranked[0]);
  return { start: primary, end: secondary };
}

function normalizeTone({ r, g, b }, anchor = null) {
  const [h0, s0, l0] = rgbToHsl(r, g, b);
  let [h, s, l] = [h0, s0, l0];
  if (s0 < 0.18) {
    // 低饱和(灰/黑/白)没有可用色相:以亮度定调,给一个克制的冷绿色相
    h = 168;
    s = 0.5;
  } else {
    s = Math.min(0.82, Math.max(0.42, s0));
  }
  l = Math.min(0.72, Math.max(0.46, l0));
  if (anchor) {
    // 副色与主色拉开色相但保持同亮度域,避免渐变两端明暗失衡
    const [ah, as] = rgbToHsl(anchor.r, anchor.g, anchor.b);
    if (as < 0.18) {
      h = (h + 60) % 360;
    } else if (hueDelta(h, ah) < 24) {
      h = (ah + 40) % 360;
    }
    l = Math.min(0.74, Math.max(0.4, l + 0.08));
  }
  return hslToCss(h, s, l);
}

function hueDelta(a, b) {
  const d = Math.abs(a - b) % 360;
  return d > 180 ? 360 - d : d;
}
function colorDistance(a, b) {
  return Math.sqrt((a.r - b.r) ** 2 + (a.g - b.g) ** 2 + (a.b - b.b) ** 2);
}

export function rgbToHsl(r, g, b) {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  if (max === min) {
    return [0, 0, l];
  }
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h;
  if (max === rn) {
    h = ((gn - bn) / d + (gn < bn ? 6 : 0)) / 6;
  } else if (max === gn) {
    h = ((bn - rn) / d + 2) / 6;
  } else {
    h = ((rn - gn) / d + 4) / 6;
  }
  return [h * 360, s, l];
}

function hslToCss(h, s, l) {
  const f = (n) => {
    const k = (n + h / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    const v = l - a * Math.max(-1, Math.min(k - 3, Math.min(9 - k, 1)));
    return Math.round(v * 255);
  };
  return `#${[f(0), f(8), f(4)].map((c) => c.toString(16).padStart(2, '0')).join('')}`;
}

export async function extractAlbumColors(imageUrl) {
  if (!imageUrl) {
    return null;
  }
  const image = new Image();
  image.crossOrigin = 'anonymous';
  image.decoding = 'async';
  const loaded = new Promise((resolve, reject) => {
    image.onload = resolve;
    image.onerror = reject;
  });
  image.src = imageUrl;
  await loaded;
  const canvas = document.createElement('canvas');
  canvas.width = SAMPLE_SIZE;
  canvas.height = SAMPLE_SIZE;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  ctx.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
  const { data } = ctx.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
  return pickDominantColors(data);
}
