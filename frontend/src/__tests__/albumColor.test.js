import assert from 'node:assert/strict';
import { test } from 'node:test';
import { pickDominantColors, rgbToHsl } from '../albumColor.js';

function rgba(hexes) {
  const out = [];
  for (const hex of hexes) {
    out.push(parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16), 255);
  }
  return new Uint8ClampedArray(out);
}

test('pickDominantColors returns saturated tones for a dominant hue', () => {
  const colors = pickDominantColors(rgba([
    '#1e88e5', '#1e88e5', '#1e88e5', '#1e88e5',
    '#1e88e5', '#1e88e5', '#1e88e5', '#1e88e5',
    '#ef5350',
  ]));
  assert.ok(colors, 'colors extracted');
  assert.match(colors.start, /^#[0-9a-f]{6}$/);
  assert.match(colors.end, /^#[0-9a-f]{6}$/);
  const [h, s] = rgbToHsl(...[colors.start.slice(1, 3), colors.start.slice(3, 5), colors.start.slice(5, 7)].map((x) => parseInt(x, 16)));
  assert.ok(s >= 0.42 && s <= 0.82, `saturation clamped, got ${s}`);
  assert.ok(h >= 190 && h <= 230, `hue stays near blue anchor, got ${h}`);
});

test('pickDominantColors picks the most frequent bucket, not the brightest', () => {
  const colors = pickDominantColors(rgba([
    '#000000', '#000000', '#000000', '#000000', '#000000',
    '#ffdd00',
  ]));
  assert.ok(colors, 'colors extracted');
  const [h, s, l] = rgbToHsl(...[colors.start.slice(1, 3), colors.start.slice(3, 5), colors.start.slice(5, 7)].map((x) => parseInt(x, 16)));
  assert.ok(l >= 0.42, `black lifted to visible tone, got ${l}`);
});

test('pickDominantColors ignores transparent pixels', () => {
  const pixels = new Uint8ClampedArray(8 * 4);
  for (let i = 0; i < 8; i += 1) {
    pixels[i * 4] = 200;
    pixels[i * 4 + 1] = 30;
    pixels[i * 4 + 2] = 30;
    pixels[i * 4 + 3] = 0;
  }
  assert.equal(pickDominantColors(pixels), null);
});

test('secondary color diverges from primary when hues are close', () => {
  const colors = pickDominantColors(rgba([
    '#22c55e', '#22c55e', '#22c55e', '#22c55e',
    '#16a34a', '#16a34a',
  ]));
  assert.ok(colors, 'colors extracted');
  const parse = (hex) => rgbToHsl(...[hex.slice(1, 3), hex.slice(3, 5), hex.slice(5, 7)].map((x) => parseInt(x, 16)));
  const d = Math.abs(parse(colors.start)[0] - parse(colors.end)[0]);
  const delta = d > 180 ? 360 - d : d;
  assert.ok(delta >= 24, `end hue rotated away from start, got ${delta}`);
});
