// CI-only: emits the golden LaTeX bundles that the latexmk job compiles.
// Deliberately separate from the root vitest.config.ts so `emit.ts` never runs
// in the ordinary test suite — it writes files, it does not assert anything.
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['scripts/latex-goldens/emit.ts'],
    css: false,
    testTimeout: 60000,
  },
});
