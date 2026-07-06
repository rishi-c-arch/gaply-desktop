#!/usr/bin/env node
'use strict';
/**
 * check:secrets — fail the build if anything that looks like an API key is
 * committed to the React frontend source. Scans `src/` by default (or the
 * directories passed as CLI args). Part of `npm run check:all`.
 */

const path = require('path');
const fs = require('fs');
const { scanDir } = require('./secretScanner');

const targets = process.argv.slice(2);
const dirs = targets.length ? targets : ['src'];

let findings = [];
for (const dir of dirs) {
  const abs = path.resolve(process.cwd(), dir);
  if (!fs.existsSync(abs)) {
    console.error(`check:secrets — directory not found: ${dir}`);
    process.exit(2);
  }
  findings = findings.concat(scanDir(abs));
}

if (findings.length > 0) {
  console.error(`\n✖ check:secrets FAILED — ${findings.length} possible API key(s) in source:\n`);
  for (const f of findings) {
    const rel = path.relative(process.cwd(), f.file);
    console.error(`  ${rel}:${f.line}  [${f.pattern}]  ${f.match}`);
  }
  console.error(
    '\nAPI keys must never live in frontend source. Store them in the OS keychain\n' +
      'via the Tauri store_secret command (gaply_core::secrets), or in the proxy\n' +
      "server's environment — never in React code, localStorage, or the bundle.\n"
  );
  process.exit(1);
}

console.log(`✓ check:secrets — no API-key patterns found in: ${dirs.join(', ')}`);
