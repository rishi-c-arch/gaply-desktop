/**
 * Security-only ESLint config for the check:all audit gate. Used with
 * `--no-eslintrc` so it does NOT inherit the CRA (react-app) config — it runs
 * ONLY these high-signal security rules, keeping the gate free of style noise.
 * The custom rule is loaded via `--rulesdir scripts/eslint-rules`.
 */
module.exports = {
  root: true,
  parser: '@typescript-eslint/parser',
  parserOptions: {
    ecmaVersion: 2021,
    sourceType: 'module',
    ecmaFeatures: { jsx: true },
  },
  // security/react drive the rules below; the rest are loaded ONLY so that
  // existing eslint-disable directives referencing their rules resolve
  // (avoids "Definition for rule not found" under --no-eslintrc).
  plugins: ['security', 'react', '@typescript-eslint', 'react-hooks', 'jsx-a11y', 'import'],
  settings: { react: { version: 'detect' } },
  rules: {
    // no eval / Function constructor / implied eval
    'no-eval': 'error',
    'no-implied-eval': 'error',
    'no-new-func': 'error',
    'security/detect-eval-with-expression': 'error',
    // react danger + children misuse
    'react/no-danger-with-children': 'error',
    // dangerouslySetInnerHTML must use sanitizeHtml(); no dynamic innerHTML
    'no-unsanitized-danger': 'error',
    // §11 D109. A prop passed through `as any` stops the checker comparing the
    // caller's shape against the component's — which is how a screen that could
    // never draw its own highlights shipped. Not a security rule in the XSS
    // sense; it is here because this gate is the one that runs on everything.
    'no-any-cast-on-props': 'error',
  },
  overrides: [
    {
      // Fixtures may be approximate. Noted rather than silent: a test that
      // casts is exercising the vocabulary, not the path, which is exactly how
      // D109 stayed invisible — prefer a fixture built to the real wire shape.
      files: ['**/*.vitest.tsx', '**/*.vitest.ts', '**/*.test.tsx', '**/*.test.ts'],
      rules: { 'no-any-cast-on-props': 'off' },
    },
  ],
};
