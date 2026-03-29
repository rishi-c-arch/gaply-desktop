#!/usr/bin/env node
/**
 * Runs react-scripts build with CI unset so Vercel's CI=true does not turn
 * webpack/eslint warnings into hard failures (see react-scripts/scripts/build.js).
 */
const { spawnSync } = require('child_process');
const path = require('path');

const root = path.join(__dirname, '..');
// Helpful first line in Vercel “Build Logs” if the job still fails
console.error('[cra-build]', process.version, root);
const env = { ...process.env };
delete env.CI;
if (!env.GENERATE_SOURCEMAP) {
  env.GENERATE_SOURCEMAP = 'false';
}

let cli;
try {
  cli = require.resolve('react-scripts/bin/react-scripts.js');
} catch (e) {
  console.error('cra-build: react-scripts not found. Run npm ci.');
  process.exit(1);
}

const r = spawnSync(process.execPath, [cli, 'build'], {
  stdio: 'inherit',
  env,
  cwd: root,
});

if (r.error) {
  console.error('cra-build: failed to spawn react-scripts:', r.error.message);
  process.exit(1);
}
process.exit(r.status === null ? 1 : r.status);
