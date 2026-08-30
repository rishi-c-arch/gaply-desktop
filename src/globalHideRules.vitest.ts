// Gaply — a guard on the one global CSS rule that can delete arbitrary UI.
//
// `src/index.css` carries a "TEMP hide floating orb" block that ends in
// `display: none !important`. Some of its selectors are SUBSTRING matches
// (`[class*="…"]`), so they hide any element whose class attribute merely
// contains the fragment anywhere — including Tailwind utilities that have
// nothing to do with a decorative orb.
//
// That is not hypothetical: `[class*="cursor"]` matched Tailwind's
// `cursor-pointer`, which silently removed all eight cards from the Citation
// Manager list and the "From paper file…" control, while the counts above them
// (read from the same array) kept saying 8. A count/list disagreement with no
// error anywhere is an expensive thing to debug, so it gets a test.
//
// Scope is deliberate: this pins the interaction utilities, the ones attached
// to elements a user must be able to see and click. The decorative
// orb/blob/float fragments are left alone — hiding those is the block's job.
import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const CSS = path.join(__dirname, 'index.css');

/** Tailwind utilities that ride on real, interactive UI. */
const INTERACTION_UTILITIES = [
  'cursor-pointer',
  'cursor-default',
  'cursor-text',
  'cursor-move',
  'cursor-wait',
  'cursor-help',
  'cursor-grab',
  'cursor-grabbing',
  'cursor-not-allowed',
  'pointer-events-auto',
  'select-none',
];

/** Every `[class*="…"]` fragment in the global hide block. */
function wildcardFragments(css: string): string[] {
  const start = css.indexOf('TEMP hide floating orb');
  expect(start, 'the TEMP hide block should still be findable by its comment').toBeGreaterThan(-1);
  const block = css.slice(start, css.indexOf('}', start));
  return Array.from(block.matchAll(/\[class\*="([^"]+)"\]/g)).map((m) => m[1]);
}

describe('the global "hide decorative elements" rule', () => {
  it('never substring-matches a Tailwind interaction utility', () => {
    const fragments = wildcardFragments(fs.readFileSync(CSS, 'utf8'));
    const collisions = fragments.flatMap((f) =>
      INTERACTION_UTILITIES.filter((u) => u.includes(f)).map((u) => `[class*="${f}"] hides .${u}`),
    );
    expect(collisions).toEqual([]);
  });
});
