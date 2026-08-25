import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { defineConfig } from 'vite';

const optionalEntries = [
  ['playbackHarness', 'playback-harness.html'],
  ['mediaPlaybackHarness', 'media-playback-harness.html'],
].reduce((entries, [name, file]) => {
  const path = resolve(import.meta.dirname, file);
  if (existsSync(path)) {
    entries[name] = path;
  }
  return entries;
}, {});

export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        app: resolve(import.meta.dirname, 'index.html'),
        ...optionalEntries,
      },
    },
  },
  resolve: {
    dedupe: ['react', 'react-dom'],
  },
});
