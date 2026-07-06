#!/usr/bin/env node
'use strict';
/**
 * Self-test for the secret scanner: a dirty fixture (fake key) must FAIL the
 * check, a clean fixture must PASS. Run via `npm run test:secret-scan`.
 */

const path = require('path');
const { spawnSync } = require('child_process');
const { scanDir, scanText } = require('./secretScanner');

let failures = 0;
function check(cond, msg) {
  if (cond) {
    console.log(`  ✓ ${msg}`);
  } else {
    console.error(`  ✗ ${msg}`);
    failures += 1;
  }
}

const dirtyDir = path.join(__dirname, '__fixtures__', 'dirty');
const cleanDir = path.join(__dirname, '__fixtures__', 'clean');

// scanner-level
const dirtyFindings = scanDir(dirtyDir);
check(dirtyFindings.length >= 1, `dirty fixture flagged (${dirtyFindings.length} finding[s])`);
check(dirtyFindings.some((f) => f.pattern === 'Anthropic API key'), 'Anthropic key detected');
check(scanDir(cleanDir).length === 0, 'clean fixture has no findings');

// scanText-level (key assembled from parts so THIS file stays clean)
const fakeKey = 'sk-' + 'ant-' + 'A'.repeat(40);
check(scanText(fakeKey).length === 1, 'scanText flags a fake Anthropic key');
check(scanText('const answer = 42; // nothing secret').length === 0, 'scanText passes clean text');

// end-to-end: the CLI must exit nonzero on dirty, zero on clean
const cli = path.join(__dirname, 'check-no-secrets.js');
const dirtyRun = spawnSync('node', [cli, dirtyDir], { encoding: 'utf8' });
check(dirtyRun.status === 1, 'check:secrets CLI FAILS (exit 1) on dirty fixture');
const cleanRun = spawnSync('node', [cli, cleanDir], { encoding: 'utf8' });
check(cleanRun.status === 0, 'check:secrets CLI PASSES (exit 0) on clean fixture');

if (failures > 0) {
  console.error(`\nsecret-scanner self-test FAILED (${failures} assertion[s])`);
  process.exit(1);
}
console.log('\n✓ secret-scanner self-test passed');
