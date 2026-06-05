const serverTokenStorageKey = 'rosettrism-server-token';

export function readServerToken() {
  try {
    return sessionStorage.getItem(serverTokenStorageKey) || '';
  } catch {
    return '';
  }
}

export function writeServerToken(serverToken) {
  try {
    const token = String(serverToken || '').trim();
    if (token) {
      sessionStorage.setItem(serverTokenStorageKey, token);
    } else {
      sessionStorage.removeItem(serverTokenStorageKey);
    }
  } catch {
    return;
  }
}

export function createApiClient(serverToken) {
  const request = async (path, options = {}) => {
    const headers = new Headers(options.headers || {});
    const token = String(serverToken || '').trim();
    if (token) {
      headers.set('x-rosettrism-token', token);
    }
    if (options.body && !headers.has('content-type')) {
      headers.set('content-type', 'application/json');
    }

    const response = await fetch(path, { ...options, headers });
    return parseResponse(response);
  };

  return {
    request,
    getJson: (path, options = {}) => request(path, options),
    postJson: (path, payload, options = {}) => request(path, { ...options, method: 'POST', body: JSON.stringify(payload) }),
    delete: (path, options = {}) => request(path, { ...options, method: 'DELETE' }),
  };
}

export async function parseResponse(response) {
  const contentType = response.headers.get('content-type') || '';
  const text = await response.text();
  const isJson = contentType.includes('application/json');
  const data = text && isJson ? parseJson(text) : text;

  if (!response.ok) {
    throw new Error(formatApiError(data, response));
  }

  if (text && !isJson) {
    return data;
  }
  return data || {};
}

function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`Invalid JSON response: ${error.message}`);
  }
}

function formatApiError(data, response) {
  if (data && typeof data === 'object') {
    return data.error || data.message || JSON.stringify(data);
  }
  return data || `${response.status} ${response.statusText}`.trim();
}
