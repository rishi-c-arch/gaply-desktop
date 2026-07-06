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
  },
};
