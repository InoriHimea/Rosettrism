import { mkdir, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { multilingualLyricFixtures } from '../tests/fixtures/multilingual-lyrics.js';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPT_DIR, '..', '..');
const OUTPUT_DIR = resolve(REPO_ROOT, 'frontend', 'verification', 'live-multilingual');
const binary = process.platform === 'win32' ? 'rosettrism.exe' : 'rosettrism';
const defaultBinary = resolve(REPO_ROOT, 'target', 'debug', binary);
const rosettrism = process.env.ROSETTRISM_BIN || defaultBinary;

await mkdir(OUTPUT_DIR, { recursive: true });

const summary = [];
for (const fixture of multilingualLyricFixtures) {
  const output = resolve(OUTPUT_DIR, `${fixture.id}.json`);
  const args = [
    'fetch',
    fixture.query,
    '--source',
    fixture.source,
    '--format',
    'json',
    '--ttl',
    '1d',
    '-o',
    output,
  ];
  const startedAt = new Date().toISOString();
  const result = await run(rosettrism, args);
  summary.push({
    id: fixture.id,
    language: fixture.language,
    query: fixture.query,
    source: fixture.source,
    output,
    startedAt,
    finishedAt: new Date().toISOString(),
    status: result.status,
    ok: result.status === 0,
    stderr: result.stderr.slice(-2000),
  });
}

const summaryPath = resolve(OUTPUT_DIR, 'summary.json');
await writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');

const failed = summary.filter((item) => !item.ok);
if (failed.length) {
  console.error(`Live multilingual capture failed for ${failed.length} fixture(s). See ${summaryPath}`);
  process.exit(1);
}
console.log(`Live multilingual capture completed. See ${summaryPath}`);

function run(command, args) {
  return new Promise((resolvePromise) => {
    const child = spawn(command, args, {
      cwd: REPO_ROOT,
      stdio: ['ignore', 'pipe', 'pipe'],
      env: {
        ...process.env,
        NO_PROXY: [process.env.NO_PROXY, '127.0.0.1', 'localhost'].filter(Boolean).join(','),
        no_proxy: [process.env.no_proxy, '127.0.0.1', 'localhost'].filter(Boolean).join(','),
      },
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });
    child.on('error', (error) => {
      resolvePromise({ status: 1, stdout, stderr: `${stderr}\n${error.message}` });
    });
    child.on('close', (status) => {
      resolvePromise({ status, stdout, stderr });
    });
  });
}
