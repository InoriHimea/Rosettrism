const fallbackMessages = {
  auth_missing_or_invalid: {
    en: 'Server token is missing or invalid. Open Settings and update the token.',
    zh: 'Server Token 缺失或无效。请前往设置更新 Token。',
  },
  cache_disabled: {
    en: 'Cache is disabled on the server. Start the server with a database path to use cache features.',
    zh: '服务端未启用缓存。请使用数据库路径启动服务以使用缓存功能。',
  },
  no_lyrics_found: {
    en: 'No lyrics were found. Try a different source, keyword, title/artist pair, or platform ID.',
    zh: '未找到歌词。请尝试更换来源、关键词、歌名/歌手或平台 ID。',
  },
  provider_warning: {
    en: 'The provider reported a temporary warning. Retry, enable Force refresh, or choose another source.',
    zh: '来源暂时返回告警。建议重试、启用强刷，或更换来源。',
  },
  ai_skipped: {
    en: 'AI selection was skipped. Check AI settings, then retry or continue without AI selection.',
    zh: 'AI 优选已跳过。请检查 AI 设置后重试，或不启用 AI 优选继续。',
  },
  validation_error: {
    en: 'The request is invalid. Check the form values and try again.',
    zh: '请求参数无效。请检查表单后重试。',
  },
  internal_error: {
    en: 'The server hit an internal error. Retry or inspect server logs.',
    zh: '服务端发生内部错误。请重试或检查服务端日志。',
  },
};

export class ApiClientError extends Error {
  constructor(message, { code = 'internal_error', details = null, retryable = false, status = 0 } = {}) {
    super(message);
    this.name = 'ApiClientError';
    this.code = code;
    this.details = details;
    this.retryable = Boolean(retryable);
    this.status = status;
  }
}

export function normalizeApiError(error) {
  if (!error) {
    return null;
  }
  if (typeof error === 'string') {
    return new ApiClientError(error);
  }
  if (error instanceof ApiClientError) {
    return error;
  }
  return new ApiClientError(error.message || String(error), {
    code: error.code,
    details: error.details,
    retryable: error.retryable,
    status: error.status,
  });
}

export function apiErrorAdvice(error, t = {}) {
  const normalized = normalizeApiError(error);
  if (!normalized) {
    return '';
  }
  const key = `apiError_${normalized.code}`;
  if (t[key]) {
    return t[key];
  }
  const language = t.language === 'Language' ? 'en' : 'zh';
  return fallbackMessages[normalized.code]?.[language] || fallbackMessages.internal_error[language];
}

export function formatApiErrorForDisplay(error, t = {}) {
  const normalized = normalizeApiError(error);
  if (!normalized) {
    return '';
  }
  return [normalized.message, apiErrorAdvice(normalized, t)].filter(Boolean).join('\n');
}

export function hasProviderWarning(warnings = []) {
  return warnings.some((warning) => String(warning || '').toLowerCase().includes('provider_warning'));
}
