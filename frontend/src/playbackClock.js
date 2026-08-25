function clampTime(value, durationMs) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return 0;
  }
  return Math.max(0, Math.min(numeric, durationMs));
}

function browserNow() {
  return globalThis.performance?.now?.() ?? Date.now();
}

function browserRequestFrame(callback) {
  if (typeof globalThis.requestAnimationFrame === 'function') {
    return globalThis.requestAnimationFrame(callback);
  }
  return globalThis.setTimeout(() => callback(browserNow()), 16);
}

function browserCancelFrame(handle) {
  if (typeof globalThis.cancelAnimationFrame === 'function') {
    globalThis.cancelAnimationFrame(handle);
    return;
  }
  globalThis.clearTimeout(handle);
}

export function createPreviewClock({
  durationMs,
  initialMs = 0,
  now = browserNow,
  requestFrame = browserRequestFrame,
  cancelFrame = browserCancelFrame,
} = {}) {
  const duration = Math.max(0, Number(durationMs) || 0);
  const listeners = new Set();
  let anchorMs = clampTime(initialMs, duration);
  let anchorNow = now();
  let playing = false;
  let frameHandle = null;
  let destroyed = false;

  function currentTime() {
    if (!playing) {
      return anchorMs;
    }
    return clampTime(anchorMs + Math.max(0, now() - anchorNow), duration);
  }

  function snapshot() {
    const timeMs = currentTime();
    return {
      currentMs: timeMs,
      durationMs: duration,
      isPlaying: playing && timeMs < duration,
    };
  }

  function notify() {
    const value = snapshot();
    for (const listener of listeners) {
      listener(value);
    }
  }

  function stopFrame() {
    if (frameHandle !== null) {
      cancelFrame(frameHandle);
      frameHandle = null;
    }
  }

  function scheduleFrame() {
    if (destroyed || !playing || frameHandle !== null) {
      return;
    }
    frameHandle = requestFrame(tick);
  }

  function tick() {
    frameHandle = null;
    const timeMs = currentTime();
    if (timeMs >= duration) {
      anchorMs = duration;
      anchorNow = now();
      playing = false;
    }
    notify();
    scheduleFrame();
  }

  return {
    nowMs: currentTime,
    durationMs: () => duration,
    isPlaying: () => playing,
    play() {
      if (destroyed || playing || duration <= 0) {
        return;
      }
      if (anchorMs >= duration) {
        anchorMs = 0;
      }
      anchorNow = now();
      playing = true;
      notify();
      scheduleFrame();
    },
    pause() {
      if (destroyed || !playing) {
        return;
      }
      anchorMs = currentTime();
      anchorNow = now();
      playing = false;
      stopFrame();
      notify();
    },
    seek(nextMs) {
      if (destroyed) {
        return;
      }
      anchorMs = clampTime(nextMs, duration);
      anchorNow = now();
      if (anchorMs >= duration) {
        playing = false;
        stopFrame();
      }
      notify();
      scheduleFrame();
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(snapshot());
      return () => listeners.delete(listener);
    },
    destroy() {
      destroyed = true;
      playing = false;
      stopFrame();
      listeners.clear();
    },
  };
}

export function createMediaClock(media, {
  requestFrame = browserRequestFrame,
  cancelFrame = browserCancelFrame,
} = {}) {
  if (!media) {
    throw new TypeError('createMediaClock requires a media element');
  }

  const listeners = new Set();
  const eventNames = [
    'canplay',
    'canplaythrough',
    'durationchange',
    'emptied',
    'ended',
    'error',
    'loadedmetadata',
    'loadstart',
    'pause',
    'play',
    'playing',
    'progress',
    'ratechange',
    'seeked',
    'seeking',
    'stalled',
    'suspend',
    'timeupdate',
    'volumechange',
    'waiting',
  ];
  let frameHandle = null;
  let destroyed = false;
  let waiting = false;

  function durationMs() {
    return Number.isFinite(media.duration) ? Math.max(0, media.duration * 1000) : 0;
  }

  function nowMs() {
    return clampTime((Number(media.currentTime) || 0) * 1000, durationMs());
  }

  function snapshot() {
    const readyState = Number(media.readyState) || 0;
    const networkState = Number(media.networkState) || 0;
    return {
      currentMs: nowMs(),
      durationMs: durationMs(),
      isPlaying: !media.paused && !media.ended,
      playbackRate: Number(media.playbackRate) || 1,
      volume: clampTime(Number(media.volume) || 0, 1),
      muted: Boolean(media.muted),
      readyState,
      networkState,
      isLoading: networkState === 2 && readyState < 3,
      isBuffering: waiting || (!media.paused && !media.ended && readyState < 3),
      isSeeking: Boolean(media.seeking),
      isEnded: Boolean(media.ended),
      error: mediaErrorMessage(media.error),
    };
  }

  function notify() {
    const value = snapshot();
    for (const listener of listeners) {
      listener(value);
    }
  }

  function stopFrame() {
    if (frameHandle !== null) {
      cancelFrame(frameHandle);
      frameHandle = null;
    }
  }

  function scheduleFrame() {
    if (destroyed || media.paused || media.ended || frameHandle !== null) {
      return;
    }
    frameHandle = requestFrame(() => {
      frameHandle = null;
      notify();
      scheduleFrame();
    });
  }

  function handleMediaEvent(event) {
    if (event?.type === 'waiting' || event?.type === 'stalled') {
      waiting = true;
    } else if (
      event?.type === 'playing'
      || event?.type === 'canplay'
      || event?.type === 'canplaythrough'
      || event?.type === 'seeked'
      || event?.type === 'emptied'
    ) {
      waiting = false;
    }
    notify();
    if (media.paused || media.ended) {
      stopFrame();
    } else {
      scheduleFrame();
    }
  }

  for (const eventName of eventNames) {
    media.addEventListener(eventName, handleMediaEvent);
  }

  return {
    nowMs,
    durationMs,
    isPlaying: () => !media.paused && !media.ended,
    snapshot,
    play: () => media.play(),
    pause: () => media.pause(),
    setPlaybackRate(rate) {
      const numeric = Number(rate);
      if (Number.isFinite(numeric) && numeric > 0) {
        media.playbackRate = numeric;
      }
      notify();
    },
    setVolume(volume) {
      media.volume = clampTime(volume, 1);
      notify();
    },
    setMuted(muted) {
      media.muted = Boolean(muted);
      notify();
    },
    seek(nextMs) {
      media.currentTime = clampTime(nextMs, durationMs()) / 1000;
      notify();
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(snapshot());
      scheduleFrame();
      return () => listeners.delete(listener);
    },
    destroy() {
      destroyed = true;
      stopFrame();
      for (const eventName of eventNames) {
        media.removeEventListener(eventName, handleMediaEvent);
      }
      listeners.clear();
    },
  };
}

function mediaErrorMessage(error) {
  if (!error) {
    return null;
  }
  if (typeof error.message === 'string' && error.message.trim()) {
    return error.message.trim();
  }
  switch (Number(error.code)) {
    case 1:
      return 'MEDIA_ERR_ABORTED';
    case 2:
      return 'MEDIA_ERR_NETWORK';
    case 3:
      return 'MEDIA_ERR_DECODE';
    case 4:
      return 'MEDIA_ERR_SRC_NOT_SUPPORTED';
    default:
      return 'MEDIA_ERR_UNKNOWN';
  }
}

export function createFakeClock({ durationMs, initialMs = 0 } = {}) {
  const duration = Math.max(0, Number(durationMs) || 0);
  const listeners = new Set();
  let currentMs = clampTime(initialMs, duration);
  let playing = false;

  function snapshot() {
    return { currentMs, durationMs: duration, isPlaying: playing };
  }

  function notify() {
    const value = snapshot();
    for (const listener of listeners) {
      listener(value);
    }
  }

  return {
    nowMs: () => currentMs,
    durationMs: () => duration,
    isPlaying: () => playing,
    play() {
      if (currentMs >= duration) {
        currentMs = 0;
      }
      playing = duration > 0;
      notify();
    },
    pause() {
      playing = false;
      notify();
    },
    seek(nextMs) {
      currentMs = clampTime(nextMs, duration);
      if (currentMs >= duration) {
        playing = false;
      }
      notify();
    },
    advance(deltaMs) {
      if (!playing) {
        return;
      }
      currentMs = clampTime(currentMs + Math.max(0, Number(deltaMs) || 0), duration);
      if (currentMs >= duration) {
        playing = false;
      }
      notify();
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(snapshot());
      return () => listeners.delete(listener);
    },
    destroy() {
      listeners.clear();
      playing = false;
    },
  };
}
