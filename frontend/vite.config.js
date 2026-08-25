import { resolve } from 'node:path';
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        app: resolve(import.meta.dirname, 'index.html'),
        playbackHarness: resolve(import.meta.dirname, 'playback-harness.html'),
        mediaPlaybackHarness: resolve(import.meta.dirname, 'media-playback-harness.html'),
      },
    },
  },
  resolve: {
    dedupe: ['react', 'react-dom'],
  },
});
