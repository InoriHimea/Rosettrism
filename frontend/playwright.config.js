import { fileURLToPath } from 'node:url';
import { defineConfig } from '@playwright/test';

const frontendDir = fileURLToPath(new URL('.', import.meta.url));
const testPort = Number(process.env.PLAYWRIGHT_PORT || 55174);
const testUrl = `http://127.0.0.1:${testPort}`;
const browserChannel = process.env.PLAYWRIGHT_CHANNEL || undefined;
const buildDir = process.env.PLAYWRIGHT_BUILD_DIR || 'playwright-dist';
const outputDir = process.env.PLAYWRIGHT_OUTPUT_DIR || 'test-results';

export default defineConfig({
  testDir: './tests',
  outputDir,
  timeout: 30_000,
  use: {
    baseURL: testUrl,
    channel: browserChannel,
  },
  webServer: {
    command: `npm run build -- --outDir ${buildDir} --emptyOutDir=false && npm exec vite -- preview --host 127.0.0.1 --port ${testPort} --strictPort --outDir ${buildDir}`,
    cwd: frontendDir,
    url: testUrl,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
