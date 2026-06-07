import test from 'node:test';
import assert from 'node:assert/strict';
import { parseResponse } from '../client.js';
import { ApiClientError } from '../errors.js';

test('parseResponse throws structured API errors from JSON bodies', async () => {
  const response = new Response(JSON.stringify({
    code: 'no_lyrics_found',
    message: 'no lyric candidates found',
    details: { source: 'provider' },
    retryable: false,
  }), {
    status: 404,
    headers: { 'content-type': 'application/json; charset=utf-8' },
  });

  await assert.rejects(parseResponse(response), (error) => {
    assert.equal(error instanceof ApiClientError, true);
    assert.equal(error.message, 'no lyric candidates found');
    assert.equal(error.code, 'no_lyrics_found');
    assert.deepEqual(error.details, { source: 'provider' });
    assert.equal(error.retryable, false);
    assert.equal(error.status, 404);
    return true;
  });
});

test('parseResponse keeps legacy JSON error messages with fallback code', async () => {
  const response = new Response(JSON.stringify({ error: 'legacy failure' }), {
    status: 500,
    headers: { 'content-type': 'application/json' },
  });

  await assert.rejects(parseResponse(response), (error) => {
    assert.equal(error.message, 'legacy failure');
    assert.equal(error.code, 'internal_error');
    assert.equal(error.status, 500);
    return true;
  });
});
