# Git hooks (shared)

Enable once per clone (they live in-repo, not in `.git/hooks`, so they are NOT
auto-active after clone):

```bash
git config core.hooksPath .githooks
```

- `pre-commit` — zero-dependency secret scanner. Blocks committing real env
  files, key material, and staged content matching live Razorpay keys, Razorpay
  secrets, Supabase `service_role` keys, AWS keys, or private-key blocks.
  Bypass a false positive with `git commit --no-verify` (review first).
