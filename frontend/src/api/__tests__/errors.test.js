import test from 'node:test';
import assert from 'node:assert/strict';
import { ApiClientError, apiErrorAdvice, formatApiErrorForDisplay, hasProviderWarning } from '../errors.js';

const t = {
  language: 'Language',
  apiError_auth_missing_or_invalid: 'Open Settings and update the server token.',
  apiError_no_lyrics_found: 'Try another source.',
  apiError_provider_warning: 'Retry or force refresh.',
};

test('UI fallback advice is selected by API error code', () => {
  assert.equal(
    apiErrorAdvice(new ApiClientError('bad token', { code: 'auth_missing_or_invalid' }), t),
    'Open Settings and update the server token.',
  );
  assert.equal(
    formatApiErrorForDisplay(new ApiClientError('nothing found', { code: 'no_lyrics_found' }), t),
    'nothing found\nTry another source.',
  );
});

test('provider warnings are recognized for retry and force refresh guidance', () => {
  assert.equal(hasProviderWarning(['provider_warning: provider task failed']), true);
  assert.equal(hasProviderWarning(['ai_skipped: missing key']), false);
});
