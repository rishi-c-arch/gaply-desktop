// Vitest — design-system tests only. Deliberately scoped to *.vitest.tsx so
// CRA's Jest (react-scripts test, which matches *.test.tsx) never collides
// with these, and vice versa. Run with: npm run test:design
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['src/**/*.vitest.{ts,tsx}'],
    css: false,
    // §11 D86. NOT the vitest default of 5000ms, and the numbers are the reason.
    //
    // These tests do real citeproc formatting against the full bundled CSL
    // repository. `chicago-author-date.csl` is 167 KB and compiling it takes
    // 713 ms when its file runs alone — but 2,871 ms in the full 61-file suite,
    // because vitest runs files in parallel and they contend. That is 57% of
    // the default ceiling before any external load, and it produced a 1-in-6
    // failure that passed on every retry: a timeout, reported as
    // "Test timed out in 5000ms", pointing at nothing actionable.
    //
    // Six further tests exceed 1 s in the parallel suite, so annotating the one
    // that lost the race first would just defer this. 20 s is ~7x the measured
    // worst case. The cost is that a genuinely hung test takes 20 s instead of
    // 5 s to fail, which against a 30 s suite is worth a guard that stops
    // failing for reasons unrelated to the code.
    testTimeout: 20000,
  },
});
