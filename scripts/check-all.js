#!/usr/bin/env node
'use strict';
/**
 * check:all — the security gate. Runs every step (no short-circuit) so a
 * failure in one is never obscured by another, prints a clearly-labeled
 * summary, and exits non-zero if any HARD gate fails.
 *
 * Hard gates (block):   secret-scan, react-security-lint, + the two self-tests.
 * Advisory (flags only): dependency-audit — the codebase carries pre-existing
 *   high/critical advisories, so it reports but does not block (flip to a hard
 *   gate once that debt is triaged). tsc --noEmit remains intentionally out.
 */

const path = require('path');
const { spawnSync } = require('child_process');

const root = path.join(__dirname, '..');
const bin = (name) => path.join(root, 'node_modules', '.bin', name);

function run(label, cmd, args, { advisory = false } = {}) {
  console.log(`\n──────── check:all › ${label}${advisory ? ' (advisory)' : ''} ────────`);
  const r = spawnSync(cmd, args, { stdio: 'inherit', cwd: root });
  return { label, ok: r.status === 0, advisory };
}

const results = [];
results.push(run('secret-scan', 'node', ['scripts/check-no-secrets.js']));
results.push(
  run('react-security-lint', bin('eslint'), [
    '--no-eslintrc',
    '--rulesdir',
    'scripts/eslint-rules',
    '--config',
    '.eslintrc.security.cjs',
    '--ext',
    '.ts,.tsx,.js,.jsx',
    'src',
  ])
);
results.push(run('secret-scanner-selftest', 'node', ['scripts/test-secret-scanner.js']));
results.push(run('react-security-lint-selftest', 'node', ['scripts/test-react-security-lint.js']));
results.push(
  run('dependency-audit', 'npm', ['audit', '--omit=dev', '--audit-level=high'], { advisory: true })
);

console.log('\n════════ check:all summary ════════');
for (const r of results) {
  const status = r.ok ? '✓ pass' : r.advisory ? '⚠ flagged' : '✗ FAIL';
  const note = r.advisory ? ' (advisory, non-blocking)' : '';
  console.log(`  ${status.padEnd(9)} ${r.label}${note}`);
}

const failed = results.filter((r) => !r.ok && !r.advisory);
if (failed.length > 0) {
  console.error(`\ncheck:all FAILED: ${failed.map((f) => f.label).join(', ')}`);
  process.exit(1);
}
console.log('\n✓ check:all passed  (review any ⚠ advisory findings above)');
