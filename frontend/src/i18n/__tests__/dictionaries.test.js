import assert from 'node:assert/strict';
import { test } from 'node:test';
import { dictionaries } from '../dictionaries.js';

const { zh, en } = dictionaries;

function placeholders(value) {
  return [...value.matchAll(/\{([^}]+)\}/g)].map((m) => m[1]).sort();
}

test('zh and en expose exactly the same key set', () => {
  const zhKeys = Object.keys(zh).sort();
  const enKeys = Object.keys(en).sort();
  const missingInEn = zhKeys.filter((key) => !(key in en));
  const missingInZh = enKeys.filter((key) => !(key in zh));
  assert.deepEqual(missingInEn, [], `keys present in zh but missing in en: ${missingInEn.join(', ')}`);
  assert.deepEqual(missingInZh, [], `keys present in en but missing in zh: ${missingInZh.join(', ')}`);
  assert.deepEqual(zhKeys, enKeys);
});

test('every dictionary value is a non-empty string', () => {
  for (const [locale, dict] of Object.entries({ zh, en })) {
    for (const [key, value] of Object.entries(dict)) {
      assert.equal(typeof value, 'string', `${locale}.${key} must be a string`);
      assert.ok(value.trim().length > 0, `${locale}.${key} must not be empty or whitespace`);
    }
  }
});

test('icu-style placeholders stay consistent across locales', () => {
  for (const key of Object.keys(zh)) {
    assert.deepEqual(
      placeholders(zh[key]),
      placeholders(en[key]),
      `placeholder mismatch for key \`${key}\``
    );
  }
});
