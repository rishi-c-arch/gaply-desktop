#!/usr/bin/env node
'use strict';
/**
 * Self-test for the React security lint: the bad fixture (raw
 * dangerouslySetInnerHTML, bypassing sanitizeHtml) must FAIL; the good fixture
 * (using sanitizeHtml) must PASS. Run via `npm run test:security-lint`.
 */

const path = require('path');
const { spawnSync } = require('child_process');

const root = path.join(__dirname, '..');
const eslint = path.join(root, 'node_modules', '.bin', 'eslint');

function lint(file) {
  return spawnSync(
    eslint,
    [
      '--no-eslintrc',
      '--rulesdir',
      path.join(root, 'scripts', 'eslint-rules'),
      '--config',
      path.join(root, '.eslintrc.security.cjs'),
      file,
    ],
    { encoding: 'utf8', cwd: root }
  );
}

let failures = 0;
const check = (cond, msg) => {
  if (cond) {
    console.log(`  ✓ ${msg}`);
  } else {
    console.error(`  ✗ ${msg}`);
    failures += 1;
  }
};

const badFixture = path.join(root, 'scripts', '__fixtures__', 'lint', 'bad.tsx');
const goodFixture = path.join(root, 'scripts', '__fixtures__', 'lint', 'good.tsx');

const bad = lint(badFixture);
check(bad.status === 1, 'bad fixture (raw dangerouslySetInnerHTML) FAILS lint (exit 1)');
check(/sanitizeHtml/.test((bad.stdout || '') + (bad.stderr || '')), 'lint message directs to sanitizeHtml');

const good = lint(goodFixture);
check(good.status === 0, 'good fixture (sanitizeHtml) PASSES lint (exit 0)');

if (failures > 0) {
  console.error(`\nreact-security-lint self-test FAILED (${failures} assertion[s])`);
  process.exit(1);
}
console.log('\n✓ react-security-lint self-test passed');
