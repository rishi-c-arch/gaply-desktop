// Vitest — design-system tests only. Deliberately scoped to *.vitest.tsx so
// CRA's Jest (react-scripts test, which matches *.test.tsx) never collides
// with these, and vice versa. Run with: npm run test:design
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['src/**/*.vitest.{ts,tsx}'],
    css: false,
  },
});
