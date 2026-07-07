#!/usr/bin/env node
'use strict';
/**
 * check:supabase-schema — build-time guard for the HARD RULE that the
 * manuscript never touches Supabase.
 *
 * Scans every SQL file under supabase/migrations/ and fails if:
 *   1. any CREATE TABLE declares a column whose NAME suggests document
 *      content (manuscript, full_text, body, abstract, findings, excerpt,
 *      chunk, paragraph, section, document, raw, content);
 *   2. any created table is missing ENABLE ROW LEVEL SECURITY;
 *   3. any created table has no RLS policy referencing auth.uid().
 *
 * Part of `npm run check:all` (7th gate). Notes: `title`, bibliographic
 * fields, and community chat `message` are allowed — they are metadata /
 * community content, not manuscript content.
 */

const fs = require('fs');
const path = require('path');

const MIGRATIONS_DIR = path.join(__dirname, '..', 'supabase', 'migrations');

// Column-NAME denylist (types like `text` are fine; names are the contract).
const FORBIDDEN_COLUMN_NAMES =
  /^(manuscript|full_?text|body|abstract|findings?|excerpt|chunk|paragraph|section|document|raw(_text)?|content)([_a-z0-9]*)?$/i;

function fail(msg) {
  console.error(`✖ check:supabase-schema FAILED — ${msg}`);
  process.exitCode = 1;
}

if (!fs.existsSync(MIGRATIONS_DIR)) {
  console.error(`check:supabase-schema — missing ${MIGRATIONS_DIR}`);
  process.exit(2);
}

const files = fs.readdirSync(MIGRATIONS_DIR).filter((f) => f.endsWith('.sql'));
if (files.length === 0) {
  console.error('check:supabase-schema — no migration files found');
  process.exit(2);
}

let tablesChecked = 0;
for (const file of files) {
  const sql = fs.readFileSync(path.join(MIGRATIONS_DIR, file), 'utf8');
  // strip SQL comments so prose can mention forbidden words freely
  const code = sql.replace(/--[^\n]*/g, '');

  // --- collect CREATE TABLE blocks -----------------------------------------
  const tableRe = /create\s+table\s+(?:if\s+not\s+exists\s+)?(?:public\.)?(\w+)\s*\(([\s\S]*?)\);/gi;
  let m;
  while ((m = tableRe.exec(code)) !== null) {
    const [, table, cols] = m;
    tablesChecked++;

    // (1) forbidden content-bearing column names
    for (const line of cols.split(',')) {
      const name = (line.trim().match(/^"?([a-z_][a-z0-9_]*)"?\s/i) || [])[1];
      if (!name) continue;
      if (['primary', 'unique', 'check', 'constraint', 'foreign'].includes(name.toLowerCase())) continue;
      if (FORBIDDEN_COLUMN_NAMES.test(name)) {
        fail(`${file}: table "${table}" column "${name}" looks like document content — the manuscript must NEVER touch Supabase`);
      }
    }

    // (2) RLS enabled for this table
    const rlsRe = new RegExp(
      `alter\\s+table\\s+(?:public\\.)?${table}\\s+enable\\s+row\\s+level\\s+security`,
      'i'
    );
    if (!rlsRe.test(code)) {
      fail(`${file}: table "${table}" does not ENABLE ROW LEVEL SECURITY`);
    }

    // (3) at least one auth.uid()-scoped policy on this table
    const policyRe = new RegExp(
      `create\\s+policy\\s+"[^"]*"\\s+on\\s+(?:public\\.)?${table}[\\s\\S]*?auth\\.uid\\(\\)`,
      'i'
    );
    if (!policyRe.test(code)) {
      fail(`${file}: table "${table}" has no RLS policy referencing auth.uid()`);
    }
  }
}

if (process.exitCode === 1) process.exit(1);
console.log(`✓ check:supabase-schema — ${tablesChecked} table(s): RLS enabled, uid-scoped policies present, no manuscript-content columns`);
