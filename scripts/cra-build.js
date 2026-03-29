#!/usr/bin/env node
/**
 * Runs react-scripts build with CI unset (Vercel sets CI=true and CRA turns warnings into errors).
 * Uses stdio: 'inherit' so webpack can stream freely. Piped spawnSync + maxBuffer kills large CRA builds
 * (ERR_CHILD_PROCESS_STD_MAX_BUFFER) right after "Creating an optimized production build...".
 */
const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..');
const pkg = path.join(root, 'package.json');
const rsPkg = path.join(root, 'node_modules', 'react-scripts', 'package.json');
const srcDir = path.join(root, 'src');

function die(msg, extra) {
  console.error('');
  console.error('[cra-build] FATAL:', msg);
  if (extra) console.error(extra);
  console.error('');
  console.error(
    'Vercel: open this deployment, scroll to the section named "Build Logs" or the "Building" step.',
  );
  console.error('The top "Logs" tab is for RUNTIME only (after deploy) — it stays empty when the build fails.');
  console.error(
    'If this repo is a monorepo, set Project Settings → General → Root Directory to: gaply-react-frontend',
  );
  console.error('');
  process.exit(1);
}

console.error('[cra-build] node', process.version);
console.error('[cra-build] cwd', process.cwd());
console.error('[cra-build] app root', root);
if (!fs.existsSync(pkg)) die('package.json missing — wrong directory or Root Directory on Vercel.');
if (!fs.existsSync(srcDir)) die('src/ missing — wrong directory or Root Directory on Vercel.');
if (!fs.existsSync(rsPkg)) {
  die(
    'node_modules/react-scripts missing — install step failed or Root Directory is wrong.',
    'Run npm install locally and commit package-lock.json; on Vercel check Install logs above this step.',
  );
}

const env = { ...process.env };
delete env.CI;
if (!env.GENERATE_SOURCEMAP) env.GENERATE_SOURCEMAP = 'false';
if (!env.DISABLE_ESLINT_PLUGIN) env.DISABLE_ESLINT_PLUGIN = 'true';
// Avoid duplicating / mangling NODE_OPTIONS from the platform
if (!env.NODE_OPTIONS || !String(env.NODE_OPTIONS).includes('max-old-space-size')) {
  env.NODE_OPTIONS = [env.NODE_OPTIONS, '--max-old-space-size=6144'].filter(Boolean).join(' ').trim();
}

let cli;
try {
  cli = require.resolve('react-scripts/bin/react-scripts.js');
} catch (e) {
  die('Could not resolve react-scripts/bin/react-scripts.js', e.message);
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
if (r.signal) {
  console.error('cra-build: react-scripts killed by signal:', r.signal);
  process.exit(1);
}
process.exit(r.status === null ? 1 : r.status);
