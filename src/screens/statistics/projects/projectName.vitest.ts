// A project's name on the screen, against the test vectors Rust's own test
// of the rule reads (Research OS, Phase 5B-3a; decision 121):
// src-tauri/gaply-core/tests/fixtures/research_project/name_vectors.json, run
// through ProjectTitle::from_typed by
// src-tauri/gaply-core/tests/research_project_names.rs.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { checkName, NAME_MAX_CHARS } from './projectName';

interface Vector {
  input: string;
  accepted: boolean;
  stored?: string;
  reason?: string;
  why: string;
}

const FILE = path.join(__dirname, '../../../../src-tauri/gaply-core/tests/fixtures/research_project/name_vectors.json');
const vectors = (JSON.parse(fs.readFileSync(FILE, 'utf8')) as { vectors: Vector[] }).vectors;

describe('the name rule, as Rust has it', () => {
  it('has the vectors', () => {
    expect(vectors.length).toBeGreaterThanOrEqual(20);
    expect(NAME_MAX_CHARS).toBe(100);
  });

  it.each(vectors.map((v) => [v.why, v] as const))('%s', (_why, v) => {
    const got = checkName(v.input);
    if (v.accepted) {
      expect(got).toEqual({ ok: true, name: v.stored });
    } else {
      expect(got).toEqual({ ok: false, reason: v.reason });
    }
  });

  it('differs from String.prototype.trim where Rust does (the vectors would catch a built-in)', () => {
    expect('abc\u0085'.trim()).toBe('abc\u0085');
    expect(checkName('abc\u0085')).toEqual({ ok: true, name: 'abc' });
    expect('﻿abc'.trim()).toBe('abc');
    expect(checkName('﻿abc')).toEqual({ ok: false, reason: 'hidden_character' });
  });

  it('refuses a lone surrogate, which cannot be sent as UTF-8', () => {
    expect(checkName('a\ud800b')).toEqual({ ok: false, reason: 'hidden_character' });
  });
});
