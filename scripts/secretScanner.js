'use strict';
/**
 * Secret scanner: detects strings that look like API keys.
 *
 * Security rule (see gaply_core::secrets): API keys must NEVER appear in
 * frontend source, localStorage, or the build bundle. This module powers the
 * `check:secrets` gate that fails the build if a key literal is committed.
 *
 * Reusable: `scanText` / `scanFile` / `scanDir` return findings; the CLI
 * (check-no-secrets.js) and the self-test (test-secret-scanner.js) both use it.
 */

const fs = require('fs');
const path = require('path');

// Each pattern targets a real provider key format. Kept specific (known
// prefixes + length/charset) to avoid flagging ordinary code.
const PATTERNS = [
  { name: 'Anthropic API key', regex: /\bsk-ant-[A-Za-z0-9_-]{20,}/ },
  { name: 'OpenAI API key', regex: /\bsk-(?:proj-)?[A-Za-z0-9]{20,}/ },
  { name: 'AWS access key id', regex: /\bAKIA[0-9A-Z]{16}\b/ },
  { name: 'Google API key', regex: /\bAIza[0-9A-Za-z_-]{35}\b/ },
  { name: 'Slack token', regex: /\bxox[baprs]-[0-9A-Za-z-]{10,}/ },
  { name: 'Generic bearer secret', regex: /\bsecret_[A-Za-z0-9]{24,}/ },
  // Supabase key material — the URL + anon key come from env, NEVER source.
  { name: 'Supabase secret/publishable key', regex: /\bsb_(?:secret|publishable)_[A-Za-z0-9_-]{20,}/ },
  { name: 'Supabase personal access token', regex: /\bsbp_[a-f0-9]{40}\b/ },
  // Hardcoded HS256 JWT (the Supabase anon/service-role key shape).
  { name: 'Hardcoded JWT (HS256)', regex: /\beyJhbGciOiJIUzI1NiI[A-Za-z0-9_-]+\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}/ },
];

const DEFAULT_EXTENSIONS = ['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '.json', '.env', '.html', '.css'];
const SKIP_DIRS = new Set(['node_modules', 'build', 'dist', '.git', 'coverage', '.cache']);

/** Scan a string; returns [{ pattern, match, line }]. */
function scanText(text) {
  const findings = [];
  const lines = text.split(/\r?\n/);
  lines.forEach((line, i) => {
    for (const { name, regex } of PATTERNS) {
      const m = line.match(regex);
      if (m) {
        findings.push({ pattern: name, match: redact(m[0]), line: i + 1 });
      }
    }
  });
  return findings;
}

/** Redact the middle of a matched secret so the scanner never prints one. */
function redact(s) {
  if (s.length <= 12) return `${s.slice(0, 4)}…`;
  return `${s.slice(0, 8)}…${s.slice(-2)} (${s.length} chars)`;
}

/** Scan a single file; returns [{ file, pattern, match, line }]. */
function scanFile(file) {
  const text = fs.readFileSync(file, 'utf8');
  return scanText(text).map((f) => ({ file, ...f }));
}

/** Recursively scan a directory. Returns all findings across files. */
function scanDir(dir, extensions = DEFAULT_EXTENSIONS) {
  const findings = [];
  const walk = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) walk(path.join(current, entry.name));
      } else if (extensions.includes(path.extname(entry.name))) {
        findings.push(...scanFile(path.join(current, entry.name)));
      }
    }
  };
  walk(dir);
  return findings;
}

module.exports = { PATTERNS, scanText, scanFile, scanDir, redact };
