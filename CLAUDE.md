# Gaply — project notes for Claude Code

Gaply is a research-integrity desktop app: a CRA React frontend wrapped in a
Tauri 2.0 shell (`src-tauri/`), with all business logic in the portable,
Tauri-free Rust crate `src-tauri/gaply-core/`, plus a standalone FastAPI proxy
(`gaply-proxy/`, Tailscale-hidden) that is the ONLY path to the cloud.

## Where this checkout lives — `~/dev/gaply-react-frontend`

**THE LIVE CHECKOUT IS `~/dev/gaply-react-frontend`.** It was cloned fresh from
`desktop` at 5d0585f on 12 Sep 2026. **Any copy under `~/Desktop/` is DEAD** and
must not be built, edited or committed from: work done there is work done in the
wrong tree, and the two will diverge silently because both have the same remotes.

### Why it moved, and why it must not move back

**Never put this repo under `~/Desktop` or `~/Documents`.** If "Desktop &
Documents Folders" is on in iCloud Drive,
macOS evicts file contents to the cloud to reclaim space, and an evicted file
still `ls`es with its correct size while every read of it fails:

```
$ head -c 200 src-tauri/gaply-core/src/app_check.rs
head: Error reading ... app_check.rs
$ cargo check
error: couldn't read `gaply-core/src/cache.rs`: Need authenticator (os error 81)
```

`cargo` stops at the first one it meets, and there is no clue in the message that
this is a storage problem rather than a code problem. **Measured 11 Sep 2026: 27
tracked files were unreadable** — 11 `.rs` files under `src-tauri/` plus the 15
`icons/*.png` the Tauri build needs and `src-tauri/.gitignore` — with dozens more
across `gaply-proxy/` and `public/`. `brctl download` did not bring them back.

**The recovery, and its one precondition.** The bytes are still in `.git`, so
evicted files can be rewritten from the object store. `git checkout -- <path>`
alone does NOT do it: git compares stat metadata, sees the file as unmodified,
and skips it. The file has to be deleted first:

```bash
# 1. CHECK THIS FIRST. It must print nothing, or you are about to lose work.
git status --porcelain --untracked-files=no

# 2. Only then: delete the unreadable files and let git rewrite them.
for f in $(git ls-files src-tauri); do
  [ -f "$f" ] && head -c 1 "$f" >/dev/null 2>&1 || echo "$f"
done > /tmp/evicted.txt
while read -r f; do rm -f "$f"; done < /tmp/evicted.txt
git checkout -- src-tauri
```

Step 1 is the whole safety argument: this rewrites the working tree from HEAD, so
it is lossless ONLY while nothing tracked is modified. Uncommitted work was lost
once to a bare `git checkout .` run against a dirty tree during exactly this
recovery. Untracked files are not touched by `checkout` and survive either way.

**Commit early while a machine is behaving like this.** The safest response to an
environment that is eating files is a commit, not a longer debugging session.

### The dead tree can be reached from OUTSIDE the repo, and git will not tell you

**Fixed 13 Sep 2026.** `~/gaply-models/` is not in any checkout, so nothing above
covers it — and both of its entries were **symlinks into
`~/Desktop/gaply-react-frontend/src-tauri/bundled-models/`**:

```
~/gaply-models/stage1-lm/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf -> ~/Desktop/...
~/gaply-models/slm1-adapter/tokenizer.json                 -> ~/Desktop/...
```

They RESOLVED, which is why this survived a move and four audits: `ls -lL`
showed 398 MB and 11 MB, readable, correct. The danger was never that they were
broken; it was that the bytes lived in the tree iCloud evicts, while
`models/mod.rs:88` makes `~/gaply-models/slm1-adapter/tokenizer.json` the
PREFERRED resolution for **every SLM-1 tier**. One eviction takes out Stage-1 and
both deep tiers at once, with `os error 81` and no hint that storage is the
problem.

Replaced with real copies from the live tree after checking both ends had the
same sha256 (`6eb923e7…` and `6b4360dd…`), so the replacement lost nothing.
`find ~/gaply-models -type l` now prints nothing, and that is the check to run
when a model "disappears".

**The general rule:** `git status` describes the checkout. Model files, caches
and anything under `$HOME` that the app resolves by convention are outside it,
so a move that fixes the repo does not fix them. After relocating this repo,
`find ~ -maxdepth 4 -type l -lname '*Desktop*'` is the sweep.

## Remotes — READ BEFORE ANY PUSH

**This checkout has two remotes on two different GitHub accounts, and they are
different products.** Neither name tells you which is which.

| remote | URL | what it is |
|---|---|---|
| `desktop` | `rishi-c-arch/gaply-desktop` | **THE ONE THIS REPO'S WORK GOES TO.** The Tauri desktop app. CI is three GitHub Actions workflows (clean-checkout, windows-build-check, package-release) and no deploy job. |
| `origin` | `RishiSTARP/gaply-react-frontend` | **NEVER PUSH.** A separate live private repo on Rishi's *other* GitHub account (`RishiSTARP`). It is the **gaply.in web lineage** and pushing it triggers the live Vercel deploy. |

### The rules

- **All desktop work goes to `desktop`.** `main` tracks `desktop/main`, so a bare
  `git push` goes there. Verify with `git rev-parse --abbrev-ref main@{upstream}`
  -> `desktop/main`.
- **NEVER push to `origin`.** Not `git push origin`, not `git push origin main`,
  not a `--set-upstream` that repoints `main`. Rishi is not working on that repo
  and does not want it touched. Pushing it deploys gaply.in.
- **`origin` is NOT dead — do not "clean it up".** `git fetch origin` and `gh`
  both report *"Repository not found"*, because the active `gh` account is
  `rishi-c-arch` and the repo belongs to `RishiSTARP`. It is a **private repo the
  current credential cannot see**, not a missing one. Deleting the remote or
  repointing it would destroy a real link to a live product.
  To confirm it exists: `gh auth switch --user RishiSTARP`, query, then
  `gh auth switch --user rishi-c-arch`.
- **`origin`'s push URL is a deliberate dead end**, so this does not depend on
  anyone reading the rule above. Its *fetch* URL is the real repo and still
  works; its *push* URL was replaced with a string that is not a repository:

  ```
  $ git push origin main
  fatal: 'DO-NOT-PUSH-origin-is-the-gaply.in-web-app-see-CLAUDE.md-section-Remotes'
         does not appear to be a git repository
  ```

  The URL IS the error message — git echoes it verbatim, so the failure explains
  itself to someone who has never read this file. Set with
  `git remote set-url --push origin <that string>`; undo with
  `git remote set-url --push origin https://github.com/RishiSTARP/gaply-react-frontend.git`.
  A bare `git push` is unaffected: it goes to `desktop`.
- **The two share NO history.** `git merge-base main origin/main` returns nothing
  — unrelated lineages, not a stale copy of this one. `origin/main` is a July ref
  that cannot be refreshed under the active credential, so its staleness is not a
  signal about anything.

**Why this is written down:** `main` once had no upstream at all, so a bare
`git push` reached for `origin`, 404'd on the private repo, and the remote looked
dead. The obvious next step — prune it — would have severed a live product's
remote. A remote that looks dead and is not is worse than one that is.

## The six Gaply agents (built in gaply_core)

| Agent | Module | Role | Cloud? |
|---|---|---|---|
| Extraction | `extract/` | PDF/DOCX/TXT → IMRaD sections, statistical claims, citations/references | no |
| Validation/Maths | `validate.rs` | deterministic 5-rule statistical validator (LLM-free) | no |
| AI-Detection | `ai_detect.rs` | perplexity/burstiness scoring (swappable `PerplexityModel`) | no |
| Plagiarism | `plagiarism.rs` | per-session isolated vector store, corpus + self-plagiarism | no |
| RAG | `rag.rs` | provenance-first ingestion with poisoning defenses; semantic search | no |
| Verification | `verify_agent.rs` | citation-hallucination detection over `refverify` evidence | ONLY agent that calls the cloud, exclusively via the proxy (`ProxyClient` seam), structured summaries only |

Supporting modules: `refverify.rs` (CrossRef/OpenAlex/Retraction Watch/
Unpaywall/Semantic Scholar connectors; `UntrustedText`/`llm_safe()` is THE
mechanism for untrusted web text — reuse it, never build a parallel one),
`ratelimit.rs`, `cache.rs`, `sanitize.rs`, `secrets.rs`, `app_check.rs`.

### Swarm topology (BUILT — `swarm.rs`, Prompt 19)

The round-table (mesh) debate orchestration now exists in `gaply-core/src/swarm.rs`:

- **Mesh round-table**: all six agents produce an `Opinion` (answer +
  explanation + confidence); up to `MAX_ROUNDS = 3` discussion rounds where each
  agent sees every other opinion and may revise; zero-revision round = early
  convergence. CPU-native, no GPU; fully offline for the five local agents —
  only Verification touches the network (`AgentKind::requires_network()`), via
  the proxy seam.
- **Consensus**: confidence-weighted voting with per-agent-kind RESCALING
  (`rescale_confidence`: deterministic k=1.0 … heuristic/LLM k=0.6 — they run
  overconfident). The Validation/Maths agent's deterministic verdicts are HARD
  CONSTRAINTS (`hard_constraint: true`) — never voted on, always override soft
  consensus (recorded via `overridden_by_constraint`).
- **Gate filtering**: opinions failing their own internal gate (e.g. the
  Prompt-17 harness gate) are rejected before entering the round-table.
- **Adapters** (`swarm::adapters`) map each real agent report into an `Opinion`
  with the shared `pass`/`concern` vocabulary.
- **ruv-swarm integration decision** (recorded in the module docs):
  `ruv-swarm-core` v1.0.6 was evaluated and NOT embedded — async-only `Agent`
  trait (tokio into a sync crate), MSRV 1.85 vs our 1.77.2 pin, and it contains
  none of the debate/voting/constraint logic anyway. `SwarmAgent` is the
  documented 1:1 seam onto `ruv_swarm_core::Agent` if the app later goes
  multi-process.
- Rate limiters and the TTL cache remain shared resources; provenance tags make
  agent outputs mergeable without trust confusion.

## SPARC methodology (dev-time, via claude-flow)

For future feature work, SPARC = Specification → Pseudocode → Architecture →
Refinement → Completion, driven by:

```bash
npx claude-flow sparc tdd "<feature description>"
```

Useful variants: `npx claude-flow sparc modes` (list phases/modes) and
`npx claude-flow sparc run <mode> "<task>"`. Pair with the repo's norms: tests
accompany every feature, mocked I/O (no real network in tests), deterministic
time injection.

## Dev-time MCP vs. Prompt-19 runtime (IMPORTANT distinction)

- **Dev-time (this setup, NOT shipped):** `claude-flow` and `ruv-swarm` are
  registered as MCP servers for Claude Code sessions in this repo
  (`claude mcp add claude-flow npx claude-flow@alpha mcp start`, same pattern
  for `ruv-swarm`). They orchestrate how we build Gaply — tooling only.
- **Runtime (Prompt 19, shipped — BUILT):** the production swarm runtime is
  `gaply-core/src/swarm.rs` — a native implementation of ruv-swarm's
  mesh/round-table semantics (the `ruv-swarm-core` crate itself was evaluated
  and rejected; see the swarm-topology section above). Nothing from the
  dev-time MCP setup ships to users.

## Working norms (standing)

- **Standing verification commands (from `src-tauri/`, measured — ARCHITECTURE_TRACE §37):**
  - inner loop — `cargo check --workspace --all-targets` (~7s after a core edit, 0.3s warm)
  - before a commit — `cargo test --workspace` (~18s warm, 1302 tests,
    9 ignored — re-measured 9 Sep 2026; the previous figure said 93s/752
    and had drifted by 550 tests, §11 D124)

  **`--workspace` is load-bearing.** Without it, cargo checks the app package's
  targets and `gaply_core` only as a lib dependency, so a broken or failing
  `gaply_core` test passes. That gap hid `release_gate.rs`'s non-compilation for
  four PRs.
- **Before a merge, verify the COMMITTED tree — not your working tree:**
  `scripts/verify-clean-checkout.sh` (defaults to HEAD; takes a ref).

  It checks out the ref into a throwaway detached worktree and builds/tests
  `gaply_core` there. **Every local `cargo test --workspace` compiles your
  working tree, uncommitted files included, so it cannot see a file you forgot
  to `git add`.** That is not hypothetical: `main` did not compile for weeks
  while every run was green, because two things committed code referenced lived
  only in the tree — `gaply-core/src/scientific_model.rs` (declared by df4eef3,
  never added) and `sentences_in` in `extract/sentence.rs` (called by
  `audit_prepass.rs` and `chunk.rs`). A fresh clone failed with
  `error[E0583]: file not found for module scientific_model`.

  The script prints any uncommitted `.rs` files up front, since one of them is
  usually the answer when it fails. It skips the app crate when
  `src-tauri/bundled-models/*.gguf` is absent — that resource is gitignored by
  design (~400 MB) — and says so rather than failing ambiguously; `gaply_core`
  is the crate that must build from source alone and is always checked.

  **WHY the script scopes itself to `gaply_core`, stated plainly so nobody
  concludes an earlier commit was broken:** *the app crate cannot build in a
  fresh worktree AT ALL*, at any commit, however healthy. `tauri-build` resolves
  `tauri.conf.json`'s bundle resources at build-script time and fails hard:

  ```
  resource path `bundled-models/tokenizer.json` doesn't exist
  error: failed to run custom build command for `app v0.1.0`
  ```

  `bundled-models/` is gitignored by design (~400 MB), so a `git worktree add`
  never has it. This is a property of the CHECKOUT, not of the code, and it looks
  exactly like a broken commit if you meet it without knowing — measured 13 Sep
  2026 while capturing a golden fixture from `e5eb963`, a commit whose CI was
  fully green.

  **So any capture-from-an-earlier-commit must supply the models.** Symlinks are
  enough and avoid a second 400 MB copy:

  ```bash
  git worktree add --detach "$WT" <ref>
  mkdir -p "$WT/src-tauri/bundled-models"
  for f in ~/dev/gaply-react-frontend/src-tauri/bundled-models/*; do
    ln -sf "$f" "$WT/src-tauri/bundled-models/$(basename "$f")"
  done
  ```

  Symlinking **into the live tree** is correct here and is not the
  `~/gaply-models` mistake above — that one pointed at the DEAD `~/Desktop`
  tree. Remove the worktree when done (`git worktree remove --force`), and note
  that a scratchpad worktree does not survive a session boundary: it comes back
  as `prunable`, and `git worktree prune` is the cleanup.

  **CI now enforces this too**, which it did not before. `.github/workflows/
  clean-checkout.yml` runs on every push and PR and builds + tests `gaply_core`
  from the checkout — a CI checkout IS the committed tree, so a missing
  `git add` fails there. It is deliberately cheap: no Node, no Tauri
  prerequisites, no bundled model. It also runs the script itself, so a guard
  that rots fails rather than passing quietly.

  `windows-build-check.yml` is no longer dispatch-only either — it now runs on
  PRs and on main. Its dispatch-only setting existed to stay clear of a Vercel
  deploy from `origin`. That reasoning was sound but its premise has changed:
  the workflows run on `desktop`, which has no deploy job, and `origin` is NOT
  gone (see "Remotes"). It stays off every-push because it is heavy (npm ci, a
  full Tauri Windows build); the cheap workflow carries the per-push guard.
- **Nothing in CI parses `tauri.conf.json`. Checked 12 Sep 2026 — and it is a
  DIFFERENT situation from the lint gate, which is the part worth recording.**

  Coverage first, plainly. Of the five workflows: `clean-checkout` and
  `latex-compile` never touch the app crate; `frontend-build` names
  `npm run tauri build` only in its own header comment; `windows-build-check`
  has the one step that would parse it — **Build Tauri Windows app** — and that
  step is `if:`-gated on the bundled models, which are gitignored, so it is
  `skipped` on every run (confirmed in the run for c44157a). `package-release`
  would parse it through tauri-action, but is `workflow_dispatch` only and
  `gh run list --workflow "Package Release"` returns `[]` — it has never run.
  So a mis-cased `"macos"` key would go green on every automatic workflow.

  **But it is not unguarded the way the frontend build was, and the difference
  is worth being precise about rather than filing both under the same alarm.**
  Measured, with the key deliberately mis-cased:

  ```
  $ npx tauri build
  Error `"tauri.conf.json"` error on `bundle`: Additional properties are not
        allowed ('macos' was unexpected)
  build exit=1   elapsed=0s   crates compiled: 0
  ```

  Tauri schema-validates the config at CLI startup, before a single crate
  compiles. The config is an **unskippable precondition of the only thing that
  produces a DMG**, so it cannot be wrong and unnoticed — the first attempt to
  ship fails instantly and says exactly which key is wrong.

  The eslint gate had the opposite shape: nothing else in the workflow would
  ever surface a warning, so 15 of them accumulated across 11 files and would
  have appeared for the first time during a real release. **The question to ask
  of a skipped gate is not "does CI run it" but "is there any path on which
  this is wrong and nobody finds out".** For the lint gate the answer was yes,
  for months. For the config it is no, by zero seconds. Only the first kind is
  a gate that isn't running; the second is a gate somewhere else.

  Two consequences. Config changes are verified by running the local build and
  reading the artefact back, not by watching CI go green — that is how
  `c44157a` was checked (docs/PACKAGING.md §1a). And a green CI run on a
  config change is not evidence about the config; do not report it as though
  it were — a mistake made on c44157a and corrected in the same session.
- **`tauri dev` WITHOUT `--release` is a trap for anything touching a model.**
  Always `npm run tauri dev -- --release` when the path under test loads BGE,
  candle, or the generative judge. Candle's CPU kernels are unoptimised in a
  debug build, and the gap is not "a bit slower", it is a different order of
  magnitude — **measured 1 Sep 2026** on the same 23 chunks of the same PDF
  through the same `embed_documents` call:

  | build | 23 chunks (2 batches) | first 16-chunk batch |
  |---|---|---|
  | release | **10.9 s** total | 7.96 s |
  | debug (`tauri dev`) | never finished | **still running at 9+ min**, ~240% CPU |

  So ≥68× on the first batch, and a "Link document" that takes 11 seconds in
  the shipped app looks like a hang in dev. Two whole sessions were spent
  waiting on that, and the app was killed before it ever finished. Diagnose
  with `ps -o %cpu` (high CPU = working, not wedged) and `sample <pid>` (frames
  in `candle_core::cpu_backend` = real matmul). The same applies to
  `citation_support`, AI Check and every probe.
- **Launch the dev app with `scripts/dev-detached.sh`** — release by default,
  double-forked and setsid so it ends up owned by launchd (PPID 1) in its own
  process group, logging to `~/gaply-dev.log`:

  ```
  scripts/dev-detached.sh          # release; the correct default
  tail -f ~/gaply-dev.log
  scripts/dev-detached.sh --stop   # stops it by SESSION, not by pattern
  ```

  The log lives in `$HOME`, never in a temp or session directory — those are
  cleaned at exactly the moment you want the log, which is after something
  vanished.

  **`--stop` kills the process group it recorded, deliberately not
  `pkill -f "tauri dev"`.** Four audits were lost mid-run to the app
  "dying at session teardown", and the log eventually showed the truth: a
  `pkill -f "target/release/app"` issued at the START of the next relaunch was
  killing the instance that had just come up seconds earlier. A pattern kill
  cannot tell your run from anyone else's, including your own newer one.
  `nohup ... &` is not enough either: it survives SIGHUP but stays in the
  launching shell's process group.
- **Stage by PATH ONLY. Never `git add .`, `git add -A`, or `git add
  --renormalize .`.** This working tree almost always carries unrelated in-flight
  work, so whole-tree staging sweeps someone else's changes into your commit —
  `--renormalize .` once pulled 64 lines of `public/sitemap.xml` generator churn
  into a `.gitattributes` commit. Name every path, then read
  `git diff --cached --stat` before committing.
- Commit ONLY when Rishi provides/approves the message; never push unprompted.
- Canonical remote: `desktop` (`rishi-c-arch/gaply-desktop`); `main` there is
  the backup of local main. Push only when explicitly requested, never forced.
  **For `origin`, see the "Remotes" section at the top — it is the single
  source of truth.** This bullet used to say origin "is gone — 404s since the
  phase-0 history purge"; that was wrong, and it is the exact belief that
  nearly led to deleting a live remote. Feature work rides stacked `feat/*`
  branches off main (reviewed checkpoint commits kept unsquashed;
  fast-forward merges to main).
- Windows CI (`.github/workflows/windows-build-check.yml`) runs
  `cargo test -p gaply_core` — the app crate's IPC tests are
  `#[cfg_attr(windows, ignore)]` (wry/tao load crash, see comments there).
- MSRV note: `gaply-core` stays on the declared **1.77.2** (portable/pure). The
  **app crate** now carries the SLM-1 candle runtime (`src/models/candle_perplexity.rs`),
  and candle needs a newer Rust than 1.77.2 — so the *app crate* effectively
  requires modern stable. This is fine: CI uses `dtolnay/rust-toolchain@stable`
  and there is no `rust-toolchain.toml` pinning 1.77.2, so builds pass. The
  1.77.2 figure is a documented target for the portable core, not an enforced
  app-crate floor. All ML deps (candle/tokenizers) live in the app crate ONLY,
  so `cargo test -p gaply_core` links none of them.
- SLM-1 model tiers: **Q3_K_M = 8GB-tier**, **Q4_K_M = 16GB-tier** (Q4 + candle's
  aarch64 repacked-Q4K cache ≈ 7.5GB base → jetsam on 8GB machines). Q3↔Q4
  absolute surprisal calibration differs (~+0.13 bits mean, r = 0.975), so
  AI-detection thresholds need PER-TIER calibration. Details in
  `src-tauri/src/models/candle_perplexity.rs` module docs; equivalence gates in
  `perplexity_probe --compare`.
- Be explicit about what's verified locally vs. what needs real infrastructure
  (AWS Nitro, Tailscale tailnet, Windows runners).
- **A static trace tells you what a mechanism DOES, not that it is the
  mechanism in play. Establish which one runs before concluding from either.**
  Two wrong findings in two days, both from tracing a real mechanism accurately
  and never asking whether it was the one carrying the behaviour:

  1. **External links "are inert."** Traced `new_window_handler` through
     tauri-runtime-wry to wry's WKWebView delegate and its `else { None }`
     branch, and confirmed the app registers no handler. All true. But
     `tauri-plugin-opener` injects `init-iife.js`, a listener on **window** that
     catches `<a target="_blank">` and invokes `plugin:opener|open_url` — the
     click is consumed in JS and never reaches the native path. Most links
     worked. The real defect was 11 links killed by a `stopPropagation` (a68af2b).
  2. **"Markdown corrupts backslashes in the editor."** Measured
     `\frac{1}{x}` → `\\frac{1}{x}` and `\,` → `,` and called it live data
     loss. The probe passed markdown **source** in as editor `content`, so
     markdown-it consumed `\,` as an escape before the round trip began.
     Through the path a user takes — type, save, reopen — every case is lossless
     and stable, in the editor and through the .docx exporter. There was no bug;
     the fix commit was cancelled before it was written (faac77c).

  3. **"The release build fails on 15 eslint warnings."** Measured
     `npx react-scripts build`, which nothing in this repo runs. `npm run build`
     is `node scripts/cra-build.js`, which does `delete env.CI` and sets
     `DISABLE_ESLINT_PLUGIN=true` — the reason is in its own header (Vercel sets
     CI=true and CRA turns warnings into errors). There was no lint gate to be
     failing. The tell was one line away in `package.json`'s `scripts`.

  **This third one is the sharpest, for two reasons.** The norm above was
  already written — two commits earlier, by me — and I broke it anyway, which
  is the evidence that knowing the rule is not the same as applying it, and
  that the check has to be mechanical rather than remembered.

  **And what caught it was a NEGATIVE CONTROL FAILING TO FAIL, not a trace.**
  The build-gate commit was pushed deliberately expecting CI to go RED on those
  15 warnings, with the fix held back to the next commit so the pair would
  demonstrate the gate actually gates. It went green. A green run where red was
  predicted is a measurement result, and it was the only thing in the sequence
  pointing at the truth — no amount of further reading would have produced it,
  because every line I had read was correct.

  **So: predict the failure before you claim the gate.** A gate whose first run
  is green proves nothing about whether it gates. Push it knowing what should
  break, or break something on purpose and watch — for the LaTeX job, a corrupt
  PNG and a double-numbered bibliography; for the lint step, one unused
  variable in, exit 1, revert, exit 0. If the prediction fails, the finding is
  wrong before the gate is.

  **The tell was the same in the first two: a probe that fed the system
  something a user never would.** A synthesized new-window request the plugin
  would have intercepted first; markdown source typed into a rich text editor;
  a build command nobody invokes. So:
  - Drive the probe from the **user's entry point** — type into the editor, click
    the control — not from the seam that is convenient to call.
  - An injected plugin script is invisible to a search of the app's own source.
    When behaviour crosses a plugin boundary, read the plugin.
  - A finding that rests on `read` alone while its neighbours rest on `ran` is
    the one to distrust. The first two WERE flagged as lower-confidence and both
    flags were right — flagging is not a substitute for measuring. The third was
    not flagged at all, because it felt measured: a command had been run and had
    genuinely exited 1. Confidence tracks *having measured something*, not
    *having measured the right thing*, so it is the weaker signal of the two.
- **NEVER CHECK A FETCH WITH `curl`. Use the app's own fetcher through a probe.**

  `ReqwestFetcher` sends the same headers and the same User-Agent as a careful
  curl invocation. What differs is the **TLS and HTTP/2 fingerprint** —
  `reqwest` + `rustls` against curl's stack — and that is precisely what a
  Cloudflare-class filter keys on. So **a curl 403 is a fact about curl, and a
  curl 200 is a fact about curl.** Neither is evidence about what Gaply can
  reach.

  **It produced three wrong findings in one day (14 Sep 2026), all about the
  journal layer:**

  | claimed | actual |
  |---|---|
  | *"5 of 9 publishers return 403 to us"* | an artefact of a minimal header set; caught before it shipped |
  | *"BMJ and SAGE are blocked"* | both answer 200 to the app; caught only by re-checking |
  | *"6 of 10 journals publish no reviewer guidance"* | four of the zeros were crawl-budget and curl 403s; BMC demonstrably publishes it |

  Measured through `ReqwestFetcher` itself
  (`examples/fetch_stability_probe.rs`): **75 requests — five targets, five
  rounds over four minutes, a fresh client, a shared client and a warmed
  client — 75 × 200, not one 403.** Not intermittent, not rate-limited, not
  varying by hour. The publishers were never the problem.

  **The tell was the same all three times: a UNIFORM result across targets that
  should differ.** Elsevier, Wiley, Taylor & Francis and SAGE are four
  companies with four infrastructures; they do not decide to refuse the same
  research tool on the same afternoon. When every row of a table says the same
  thing, suspect the column.

  That puts this with the batch-known-good entry and the piped-exit-status
  entry: **an instrument reporting a property of itself as a property of the
  world.** The remedy is the same one — run the thing you are actually asking
  about, not a convenient stand-in for it. If a fetch needs checking, write the
  four-line example that calls `ReqwestFetcher`; it costs one `cargo build` and
  it answers the question asked.

  **And do not build retries around a 403 until you know which kind it is.** A
  retry loop against a fingerprint spends the budget and changes nothing.

- **A SPAN IS THE ONE FIELD THAT CANNOT BE PLAUSIBLE AND WRONG AT THE SAME
  TIME, because it quotes the source and the source says what it is.**

  Any extracted fact that carries its sentence can be checked by a human in one
  glance. Any that carries only a value has to be trusted. That is the whole
  argument for storing the sentence beside the number — not provenance for its
  own sake, but the difference between a record a reader can refute and one
  they can only believe.

  **A TRUNCATED SPAN IS NOT A SPAN.** The same rule fails from the display side
  as easily as from the storage side. The checklist's data-availability item was
  correct and its span was clipped at 150 characters by the probe that printed
  it, so it read as fast-track boilerplate with no mention of data availability
  — the item looked unsupported, and the instrument had hidden the evidence for
  it. The full sentence contains *"…funding statement, **a data availability
  statement** and in the case of studies involved custom code…"*. A span exists
  to be checked, and a clipped one cannot be: stored whole and printed short is
  the same defect as never stored at all, from the reader's side.

  **It earned itself four times in one afternoon (14 Sep 2026, the journal
  layer), and every one was invisible in a count and obvious in a row:**

  | what the count said | what the row said |
  |---|---|
  | `word_limit = 12000` | *"Features: Translation by a native Chinese speaker with an advanced degree…"* — a price list from the publisher's paid translation service |
  | `1 conflicted fact` | CONSORT / PRISMA / STROBE / STARD / TRIPOD / ARRIVE, each span naming the study design it applies to — six requirements, not one dispute |
  | `12 requirements per source document` ×2 | `/nm/content` and `/nm/about/content` — one page under two paths |
  | `word_limit = 100` | *"In the case of text-mining, individual words, concepts and quotes up to 100 words per matching sentence…"* — a re-use licence, not a submission limit |

  "46 requirements, 3 conflicts" is a plausible summary, and both of its
  numbers were wrong — 29 and 1 once the rows were read. **No count caught any
  of these; printing rows with their spans caught all four.** So: when a
  pipeline produces facts, make the probe print rows, not totals — and pick the
  row the pipeline itself would surface, not one you chose. The text-mining
  quota was found because the probe prints the NEWEST `word_limit`, and that
  happened to be the bad one.

  **A FRACTION IS A CLAIM, AND ITS DENOMINATOR IS THE HALF NOBODY CHECKS.**

  The numerator gets read, argued about and tested. The denominator is
  furniture. **Measured 15 Sep 2026**, in the reporting-standard evaluator:

  ```
  published_item_count(Consort) -> 25          // CONSORT's NUMBERED items
  items_for(Consort)            -> 1b 6a 7a 12a 17a   // five SUB-items
  row: "checks 4 of CONSORT's 25 published items"     // 16%
  truth:                          4 of 37             // 11%
  ```

  **The unit of the numerator was not the unit of the denominator**, and
  `items_for`'s own doc comment three lines above `published_item_count` had
  said so since it was written: *"CONSORT 2010 has 25 items and 37 sub-items."*
  The evidence was in the file, adjacent, and the fraction was still wrong.

  **What makes it the same family as the truncated span**: the row's ONLY JOB is
  to show how little was checked — it exists so a passing evaluator is not read
  as a passed checklist — and it understated the gap by a third. A guard that is
  wrong in the direction it was built to guard against is worse than no guard,
  because its presence is what stops anyone looking.

  It was **plausible and displayed**, which is the pair to watch for: 16% reads
  like a small honest number, so nothing about it invites a second look. Compare
  §11 D163's `word_limit = 12000` — plausible, displayed, and a translation
  price list.

  The fix is not a better number, it is **a unit on the number**:
  `PublishedCount { numbered, sub_items }`, and `coverage_phrase` compares like
  with like. Where the sub-item total is not recorded, it says so —
  *"checks 2 of STROBE's 22 numbered items — Gaply's items include sub-items, so
  the true fraction is smaller"* — rather than inventing a denominator that
  would look precise and be unsourced. **Four of the five standards are in that
  state, and that is the honest report**: only CONSORT's 37 had a source.

  So: when you print `N of M`, say what M counts, and check that the thing
  producing N counts the same thing. If you cannot source M, print the mismatch
  rather than a number.

  **A THREE-STATE VALUE MUST BE THREE STATES AT THE WIRE — AND THE TEST IS
  RENDERING THE DEFAULT, NOT TRUSTING IT.**

  Third time this week a MISSING value was read as a STATED one, and the
  cheapest of the three to have caught. `ChecklistItem` carries compliance in
  two bools, `passed` and `unevaluable`. A stored report written before
  `unevaluable` existed has no such key, `serde(default)` supplies `false`, and
  an item that was never decided arrives as `passed: false, unevaluable: false`
  — which every renderer draws as **FAIL**. A researcher would see a compliance
  failure on a check that was never run.

  The other two the same week: `ResearchState::science` being `None` meant *"the
  extractor was never asked"* and read as *"the paper has no claims"*; and a
  privacy test passed because its fixture produced zero claims, so `science:
  None` stood in for "no prose leaked".

  **Two bools cannot carry three states across a wire where one of them may be
  absent.** The representation that cannot fail is one three-valued field. Where
  the wire contract makes that too expensive — `ChecklistItem` is rendered by the
  frontend — the obligation moves to the test, and the test has a specific
  shape:

  - **Deserialise a payload with the key ABSENT**, not one you constructed with
    the default.
  - **Assert what a RENDERER draws**, computed the way the renderer computes it
    — not that the field equals `false`. Asserting the default is trusting it;
    the defect lives in what the default MEANS downstream.
  - **Pin the defect rather than hide it.** The test asserts `FAIL` is what a
    legacy row renders as, and names the containment in the same breath:
    `unevaluable` shipped WITH the evaluator, so no stored report predating the
    key can contain an undecidable item. The day that stops being true, the
    test is where it is written down.

  `skip_serializing_if` is what keeps the true state recoverable — a `true` flag
  always reaches the wire — and it was added for a different reason (the golden
  report is pinned byte-for-byte). Two requirements, one mechanism; note both
  where it is declared, or the next person removes it for the reason you did not
  write down.

  **FOUR BOUNDARIES WERE DRAWN TOO WIDE IN ONE PHASE, and each produced data
  that was not the journal's:** the HOST (`journals.plos.org` is every PLOS
  journal, so a crawl of PLOS ONE reached PLOS Genetics), the DOMAIN (a
  publisher's author-services site sells translation and editing to the same
  authors it advises), the requirement KIND (`reporting_standard` is not
  single-valued, so a second one read as a contradiction), and WHAT A NUMBER
  COUNTS (a licence's quota is not a manuscript limit). §11 D160, D161.

  **And the majority of what needed fixing was in the INSTRUMENT, not the
  corpus — for the third phase running.** The journal crawler wandered into
  research articles, deduplicated on URL where pages differ only by scheme,
  gave up on a rate limiter and called it success, and ordered its frontier so
  that the pages justifying its own design were the first to be cut off. Part
  D's equation engine had the same shape: four fabricated Tier-0 findings, all
  from the checker rather than the manuscripts (§11 D157). Before that, a
  performance prediction was 20–100x wrong because the calibration file carried
  the defect being measured.

  The habit that follows is not "distrust the corpus" — it is the opposite.
  **Run against real inputs early, print what the machinery actually produced,
  and expect the first few rounds of findings to be about your own tools.** A
  clean first result on real data is the thing to be suspicious of.

- **A PURITY CLAIM IN A DOC COMMENT IS NOT A GUARD. Counted 14 Sep 2026:
  `validate.rs`, `stats_verify.rs` and `stats_verdict.rs` each state "no model,
  no proxy, no network, no I/O" in their module headers — three claims, ZERO
  tests, and the only reason they have held is that nobody has tried.**

  This matters more than an ordinary unguarded invariant because of what the
  claim BUYS. §4.4 puts Tier 0 above every model in the system: a deterministic
  finding overrides eight agents agreeing, and `swarm.rs` implements that as
  `hard_constraint` — never voted on, always overriding. **The authority is
  granted on the strength of a sentence in a comment.** One `use` line inside
  any of those files would leave the override in place and the determinism
  gone, and nothing anywhere would say so.

  `gaply-core/tests/equation_is_llm_free.rs` is the first guard of this kind in
  the repo — a source scan over `src/equation/` for any route to a model, proxy,
  network, database, filesystem, clock or entropy source, plus an import
  allowlist pinned to `std` and `serde`. **It was NOT copied from an existing
  pattern**: the instruction that prompted it said to assert purity "the way the
  startup module asserts no HTTP client", and no such assertion exists. The
  absence of an HTTP client in `gaply_core` is real and is a property of
  `Cargo.toml` that nothing tests; the absence of a model is asserted nowhere at
  all. A reader told this follows an existing pattern would go looking for the
  pattern, so the test's own header says it is the first.

  Confirmed to gate by breaking the real tree twice — a `std::fs` call and a
  `regex` import — and watching each fail with the right message. The three
  older modules are still unguarded; extending the scan to them is a separate
  change, and this entry is here so that stays visible rather than being
  rediscovered.

- **AGREEMENT BETWEEN A SPEC, ITS IMPLEMENTATION AND ITS TEST IS EVIDENCE THAT
  THEY WERE DERIVED FROM ONE ANOTHER, NOT THAT ANY OF THEM IS CORRECT.
  Re-reading any one confirms the other two.**

  **A different family from the four entries around it.** Those are instruments
  that LIED — a swallowed exit status, a pattern matching its own watcher, a
  batch whose harness was broken, a calibration reference carrying the defect it
  was measuring. **This one is every instrument telling the truth about a false
  premise.** Nothing malfunctioned. The spec said a thing, the code did that
  thing, the test asserted the code did it, and all three were wrong together.

  Measured 13 Sep 2026, building the agent graph:

  1. **The spec.** `docs/publishready-premium-architecture.md` §3.1 gave the
     Manuscript layer a "Privacy class" of *"premium, `Manuscript` consent"* —
     under a column heading reading **"Who may read it"**.
  2. **The implementation followed it exactly.** `agent_graph.rs`'s first
     consent rule was `layer.is_premium_consented() && requires_consent.is_none()`
     → error, for any agent, cloud or local.
  3. **The test pinned the implementation and passed.**
     `reading_the_manuscript_layer_without_consent_is_rejected` asserted
     precisely that behaviour.

  Read as a read-permission, §3.1 **would have required premium consent to parse
  a file the user had just opened** — the free tier's entire extraction path.
  Three artefacts in perfect agreement, and no amount of re-reading any of them
  would have surfaced it, because each was consistent with the other two.

  **WHAT BROKE THE LOOP WAS A CASE THE RULE HAD TO DECIDE.** Writing a real
  graph for the six existing lanes, the validator rejected `extraction` — a
  local, deterministic pass that reads the manuscript and transmits nothing. The
  rejection was *obviously* wrong, and that absurdity is what made the premise
  visible. The correct rule fell out immediately: privacy classes are about
  EGRESS, not reading — any agent may read any layer; a CLOUD agent may only
  send a layer needing consent if it declares a scope COVERING it.

  **So: the thing that tests a premise is a case it has to rule on, not another
  reading of it.** When a spec, its code and its test agree, the question is not
  whether they are consistent — they will be, that is what derivation does. The
  question is **whether anything has ever forced the rule to decide something
  real**. If the only inputs it has seen were constructed from the same
  understanding that produced it, it has never been tested at all.

  **THE COMMONEST FORM HAS NO SPEC IN IT AT ALL — just a function and its
  fixtures — which is why it is the easiest to miss.** The agent-graph case
  above needed three artefacts to agree; this needs two, so it is both more
  common and quieter.

  **Second instance, 14 Sep 2026**, and it is the cheap version: the negation
  guard in `journal_extract::states_a_required_statement` was written against
  one real sentence — *"The section should also not be used to declare
  competing interests"* — and pinned by a test built from that same sentence.
  Both passed. **Deleting the guard entirely left the test GREEN**, because an
  earlier guard rejects that sentence first. The pin had never exercised the
  thing it named. A scan then asked the corpus how many sentences reach the
  guard at all: zero. It is kept, with a test that genuinely reaches it and a
  doc comment saying it is predicted rather than measured.

  **Third instance, same day, and the expensive version:**
  `journal_expect::is_reviewer_guidance` decides whether a
  guideline page is addressed to reviewers or to authors. Its four tests pass.
  They are hand-written strings — *"Guidelines for Reviewers"*, an author
  instruction about competing interests — written by the same author, in the
  same sitting, from the same idea of what a reviewer page looks like. Nothing
  disagreed with anything, and there was no spec to be wrong; the premise lived
  only in the function and was copied into the fixtures.

  Run over a real crawl it returns `true` for **20 of Nature Medicine's 36
  admitted pages**: the five genuine reviewer pages, and fifteen author pages
  with them — `preparing-your-submission`, `aip-and-formatting`,
  `matters-arising`, `clinicalresearch`, `ethics-and-biosecurity`,
  `aims/fasttrack`. **Precision 5/20, recall 5/5: a filter that catches
  everything.** The cause is one clause — `mentions >= 2 && contains("review")`
  over whole page text — and every page on a journal's site discusses peer
  review somewhere. It was about to be wired into the requirement path as a
  fix, where it would have suppressed 14 of 18 extracted rows, twelve of them
  true (§11 D163).

  **What broke the loop was the corpus, as in both cases above.** Not a
  re-read, not a more careful fixture — the function was simply pointed at 36
  real pages and asked to rule on each. Note what caught the second instance,
  though: **deleting the guard and predicting a red test.** That is cheaper
  than a corpus and available immediately, and it is the same move as the
  lint-gate entry's "predict the failure before you claim the gate". Run it on
  every guard you add; run the corpus on every classifier you trust.

  **A DELETION TEST THAT GOES GREEN IS NOT A FAILED CHECK — IT IS A FINDING
  ABOUT THE TEST.** The move was introduced to prove a guard gates. Measured
  across one session of 39 deletion tests, it did that 36 times and **three
  times told me something else entirely: the test could not reach its guard.**

  | guard deleted | why the test stayed green |
  |---|---|
  | cue-stripping in `states_its_scope` | no cue in the list could reach it — and investigating that found a real bug, `contains("in ")` matching inside `rema`**`in `**`unexplored` |
  | a declined specialist earning a strength | the criterion had a second source that had also not run, so the assertion held on the wrong reason |
  | the span fix for reporting-standard absences | the evidence policy refused the span-less concern first, so the invariant held trivially |
  | an unanchored concern being dropped | the fixture had no unanchored MINOR concerns, so a severity-selective drop was invisible |

  **Every one of those is a test that would have passed forever while the thing
  it names stopped being true.** Two of them were guards overlapping — defence
  in depth, which is good — but a test that cannot tell which of two guards is
  holding is not a test of either, and it reports green when one is removed.

  So the rule has a second half. **When a deletion test goes green, do not
  shrug and move on: find out why, and fix the TEST.** The repair is usually
  one of three —
  * the fixture does not exercise the branch (add the case: the unanchored
    MINOR, the second source that DID run);
  * another guard catches it first (assert the other guard stayed quiet too —
    `policy_rejected.is_empty()` beside "every concern has a span");
  * the guard is genuinely unreachable today (keep it, and pin the PREMISE that
    makes it unreachable, so the day that premise changes the test goes red).

  Predicting red and getting green is the same class of signal as the
  negative-control entry above: **a measurement result, not a non-event.**

  **So, operationally: A HAND-WRITTEN FIXTURE INHERITS THE AUTHOR'S PREMISE.
  ITS FIRST INDEPENDENT VOTE IS THE CORPUS.** The first REAL input — the actual
  six lanes, an actual manuscript, the actual committed graph, 36 fetched pages
  — is where a premise gets a vote it did not write. Build that input early,
  run every classifier and gate over it before trusting the green suite, and
  treat a result that looks absurd as information rather than as something to
  special-case around. A green test tells you the function does what its author
  thought; only the corpus tells you whether that was right.

- **BEFORE CALIBRATING A FIX AGAINST A "CLEAN" REFERENCE IN THE SAME CODEBASE,
  VERIFY THE REFERENCE IS CLEAN. A reference point inside the system under
  measurement is a MEASUREMENT, not a constant, and it needs the same scrutiny
  as the thing being fixed.**

  **This is a different failure family from the three entries below it.** Those
  are instruments reporting success while the thing they watched failed — a
  swallowed exit status, a pattern matching the watcher, a batch whose harness
  was broken. This one is an instrument working perfectly on a contaminated
  input: the arithmetic was right, the reasoning was right, and the number was
  wrong because the ruler was.

  Measured 13 Sep 2026. The scientific-extraction passes compiled their regexes
  once per sentence, costing 30.4 s across six manuscripts. Predicting what
  caching them would save, `variables.rs` was taken as the floor for
  "correctly-cached matching work" at **1.3–1.9 ms/sentence** — it used
  `OnceLock`, so it looked like the clean case. **It had four uncached regexes
  of its own.** The prediction inherited the defect it was measuring:

  | | predicted | measured |
  |---|---:|---:|
  | R PAPER | 250–350 ms | **2.8 ms** |
  | final final L | 2.5–3.5 s | **150 ms** |
  | corpus | 4–6 s | **196 ms** |

  **20–100x too pessimistic, all of it traceable to the calibration file.**

  **An EXTERNAL floor would not have had the defect.** "What does matching a
  compiled regex cost, independent of this codebase" is answerable from first
  principles — roughly two orders of magnitude below compiling one — and that
  is the number that turned out to be right. Prefer a floor that does not live
  in the code you are changing; when you must use an internal one, measure it
  first and say that you did.

  **WHAT EXPOSED IT WAS THE GUARD, NOT THE READING — and the source pass that
  produced the prediction had already looked at that file.** A
  `tests/regex_compilation_guard.rs` scan written afterwards found **nine
  uncached regexes the careful read had missed**, five in `datasets.rs` and
  **four in the calibration file itself**. That is the argument for building the
  instrument rather than trusting a thorough pass: the pass was thorough, was
  performed by someone who knew exactly what to look for, and missed a third of
  the instances — including the ones that invalidated its own conclusion.

  **A related honesty point from the same fix.** An O(n²) pointer-identity scan
  (`methods.rs`, recovering an index `enumerate()` already had) was corrected in
  the same change. It was **below the probe's resolution** and is not separately
  measurable — it ran only on sentences that already matched a statistic. It was
  fixed because it was WRONG, not because it was slow, and the accounting says
  so: essentially all the cost was regex compilation. Listing it as a
  contributing optimisation would have inflated the story of the fix with a
  change that bought nothing measurable.

- **A BATCH RUN MUST CONTAIN A KNOWN-GOOD CASE. A uniformly clean result is the
  signature of a broken instrument, and the known-good case is the only thing
  that can tell you.** Measured 13 Sep 2026, re-measuring the AI-detection
  severity across six real manuscripts:

  ```
  chapter3 .docx        sev=NONE   none
  final final L.pdf     sev=NONE   none
  IJAS … haemolymph.pdf sev=NONE   none      <- this one had produced a finding
  Lake Chapter 1.docx   sev=NONE   none         ten minutes earlier
  R PAPER .docx         sev=NONE   none
  Revised … FINAL.docx  sev=NONE   none
  ```

  Six clean results reading as "the fix worked everywhere". The loop used
  `timeout`, **which does not exist on macOS by default**, so every invocation
  failed instantly and `|| true` swallowed it. Nothing in the output said so —
  a failed run and a run that found nothing are the same empty string.

  The only reason it was caught is that one row was a case whose answer was
  already known from a single run minutes before. Without it, "0 of 6" would
  have gone into a decision record as a result.

  So: **every batch includes at least one input whose answer you already know,
  and you check that row first.** If it comes back clean, the instrument is
  broken and the other rows mean nothing. This is the negative-control rule
  applied to the harness rather than to the code — and it is cheap, which is
  the argument for doing it every time rather than when suspicious.

  Corollaries from the same session: prefer `|| true` nowhere near a
  measurement loop; a missing binary should be a loud failure, not an empty
  result; and `ps -o %cpu` distinguishes the other failure mode — a batch at
  **0.0% CPU** with a network- or keychain-shaped stack is blocked, not working
  (that is how §11 D154's consent hole surfaced, on the very next attempt at
  this same table).

- **NEVER READ THE EXIT STATUS OF A COMMAND YOU PIPED. Capture to a file, then
  read the status AND the file.** A shell pipeline reports the LAST stage's
  status, so `cargo build --example x 2>&1 | tail -20` exits **0** on a build
  that failed — you get `tail` succeeding at printing an error.

  Measured 13 Sep 2026, and it cost two rounds. A golden-fixture capture was
  reported as built; the binary did not exist; the log showed `Finished` because
  `tail` had only kept the last 20 lines, which were `gaply_core`'s warnings from
  BEFORE the app crate failed. Re-run as `cargo build … > build.log 2>&1; echo
  "CARGO_EXIT=$?"` it printed `CARGO_EXIT=101` and the real cause on the first
  line of the file.

  ```bash
  # WRONG — $? is tail's, and the head of the error is discarded
  cargo build --example x 2>&1 | tail -20

  # RIGHT — the status is cargo's, and the whole log survives
  cargo build --example x > build.log 2>&1; echo "CARGO_EXIT=$?"; tail -25 build.log
  ```

  **And check for the artefact, not only the status.** `CARGO_EXIT=0` was
  confirmed alongside `ls target/debug/examples/x` before the capture was
  trusted, because a status that has already lied twice is not evidence on its
  own.

  **A FILTER YOU WROTE YOURSELF IS THE SHARPER VERSION OF THIS, because you
  know what it does and that is exactly why you do not check it.** 14 Sep 2026,
  the journal screens. A new frontend file was typechecked with:

  ```bash
  npx tsc --noEmit -p tsconfig.json 2>&1 | grep -E "journal/" | head -5; echo TSC_DONE
  ```

  `tsc` **exited 2** and printed the error. The file was under
  `src/screens/publishready/`, not `journal/`, so the grep matched nothing —
  and `echo TSC_DONE` ran either way, because `;` does not care. A clean
  filtered view plus a success word the shell prints unconditionally read
  exactly like a pass. CI caught it: the CRA production build failed on
  `TS2802`, `matchAll` needing `downlevelIteration` at this project's es5
  target, while 929 vitest tests stayed green because esbuild accepts it.

  **The difference from the pipeline case above is who built the blind spot.**
  There, `tail -20` discarded the head of an error nobody chose to hide. Here
  the filter was written deliberately, three minutes earlier, to show only the
  files being worked on — and a filter written for relevance is trusted for
  completeness without anyone deciding to trust it. The narrower and more
  purposeful the filter, the less likely it is to be questioned.

  **The rule that covers both: read the exit status, and if you filter the
  output, print the status ALONGSIDE the filtered view so the two cannot
  disagree silently.**

  ```bash
  # WRONG — the filter is the only thing reporting, and TSC_DONE is uncondi-
  # tional. Both are true of a run that exited 2.
  npx tsc --noEmit 2>&1 | grep -E "journal/"; echo TSC_DONE

  # RIGHT — the status is tsc's, and the filtered view sits beside it
  npx tsc --noEmit > tsc.log 2>&1; echo "TSC_EXIT=$?"; grep -E "journal/" tsc.log
  ```

  The verification that actually worked was the norm's own second half:
  **`TSC_EXIT=0`, `BUILD_EXIT=0`, and `build/` present** — status AND artefact,
  for the real command a release runs rather than a proxy for it.

  **This is the same shape as the two entries around it, which is why they sit
  together:** `|| true` swallowing a missing binary, a pipe reporting the wrong
  stage, and `pkill -f` matching the watcher instead of the target
  (`dev-detached.sh --stop`). In every case **the instrument reported success
  while the thing it was watching had failed**, and in every case the tell was
  an artefact that should have existed and did not. Trust the artefact over the
  status.

  **A NEGATIVE GREP AS A LOOP CONDITION INVERTS ON EMPTY INPUT — the fourth
  instance, and a mechanism the other three do not have.** 15 Sep 2026, waiting
  on three CI workflows for one SHA:

  ```bash
  # WRONG — exits the loop the first time the API returns nothing
  while gh run list ... --jq '...status' | grep -qv completed; do sleep 30; done
  echo "ALL DONE"
  ```

  `grep -qv completed` means *"some line is not finished"*. An empty list has no
  line that is not finished, so `grep` fails, the loop exits, and `ALL DONE`
  prints — **while two of the three runs were still `in_progress`**. One slow or
  failed API call is enough. A second waiter written the same afternoon had the
  same inversion (`! ... | grep -q "in_progress\|queued"`).

  **The distinction from the three above is worth keeping.** `|| true` swallowed
  a STATUS; the pipe reported the wrong stage's STATUS; `pkill -f` matched the
  wrong TARGET. This one inverted a MEANING: the condition was correct about
  full input and said the opposite of what it meant about empty input. Nothing
  was swallowed and nothing mismatched — the predicate was read backwards by the
  absence of data.

  **The rule: wait on a POSITIVE COUNT, never on the absence of a pattern.**

  ```bash
  # RIGHT — an empty or failed response leaves n empty, which is not "3"
  n=$(gh run list ... --jq "[.[] | select(.status==\"completed\")] | length")
  if [ "$n" = "3" ]; then echo "ALL 3 COMPLETED"; fi
  ```

  `completed_count == 3` cannot be satisfied by an empty response. The same
  applies to any wait: count what you are waiting FOR, not what you are waiting
  to stop seeing.

  **And it reached a draft message before a live check caught it** — the same
  path the conversation-sourced figures took in §11 D166, which were tabulated
  and nearly recorded before being measured. Both were plausible, both matched
  what was expected, and both were wrong. **An instrument that agrees with what
  you expect is the one to re-run**, because agreement is what removes the
  prompt to check. The verification that worked here was the cheapest possible:
  ask the API again, directly, and read the three rows.

  **AN INSTRUMENT THAT ASSERTS ITS OWN ANSWER — the fifth instance, and the
  only one where the DOCUMENTATION is the dangerous part.** 15 Sep 2026, in a
  probe measuring which chat modes a stored run could answer:

  ```rust
  fn exists(_needle: &str) -> bool {
      // Checked by grep at build time, not at runtime — this probe does no I/O
      // over the source tree. Every one of these was ABSENT when measured
      // 15 Sep 2026; the row is here so the claim is visible, not computed.
      false
  }
  ```

  It printed a table of four "ABSENT" rows about types the codebase might
  contain. **It had checked nothing.** The answers happened to be right — a
  real grep returned 0 hits for all four — which is exactly what makes it worth
  recording: a fabricated instrument that agrees with reality teaches you
  nothing and will not be questioned.

  **The comment is the defect, not the `false`.** A bare `false` invites the
  question *"is that measured?"*. A comment saying *"checked by grep at build
  time … measured 15 Sep 2026"* answers that question before a reader asks it,
  with a claim that is not true. **It pre-empts the check it should have
  prompted** — and it would have survived into a decision record as a measured
  row, in a file whose whole discipline is that numbers name the run that
  produced them.

  This is the same shape as §11 D123's constants and D166's carried-in figures:
  a statement that is true SOMEWHERE re-aimed at a question it never answered.
  The difference is that those were re-aimed by a human reading them, and this
  one was written into the instrument.

  **The rule: a probe measures or it says nothing.** If the check belongs in the
  shell, run it in the shell and paste the output; do not stub it in the probe
  and narrate the result. And when you write a comment asserting that something
  was measured, the comment is a claim — it needs the same evidence as a number.

  **A COMMENT'S JURISDICTION IS THE FILE IT SITS IN — the sixth instance, and
  the one that makes no false claim about itself at all.** 15 Sep 2026,
  red-teaming the reviewer payload. `reviewer_agent::build_review_payload`
  applied a plain length `clamp` to `title` under this:

  ```rust
  // title only — `detail` is deliberately never read (privacy).
  ```

  Every word of that is true **of that file**. The builder does read only
  `title`, and `detail` — the field holding manuscript excerpts — never enters
  the payload. What it asserts, silently, is a fact about the CRATE: that no
  finding constructor anywhere puts manuscript text in a title. **That is false,
  in `equation_report.rs`**, whose Tier-0 titles are `format!("Arithmetic {}:
  {}", status, truncate(&f.source_line, …))` — the manuscript's own line,
  verbatim. A paper whose equation LABEL is `ignore previous instructions =
  36.5 + 28.2 + 20.1 = 100.0` put that instruction into the model payload as a
  finding title.

  **The difference from the `exists()` entry above is the whole reason this is
  a separate variant.** That one asserted its own answer and its comment lied
  about having measured. This comment lies about nothing — it describes its own
  file accurately, and I had read and approved it two commits earlier while
  looking for exactly this class of problem. **No amount of re-reading
  `reviewer_agent.rs` could have found it, because the falsifying fact is in
  another file.** It is the spec/impl/test agreement entry's shape with the
  artefacts reduced to one: a statement consistent with everything in view.

  **Second instance, same week:** `chat_scope`'s header said it *"reuses the
  `llm_safe` firewall unchanged"* — true of the field it was written about, and
  false of the three the module also copied. Both were headers I had approved.

  **The rule: a claim about what OTHER code does or does not produce is a claim
  about the crate, and only a fixture that runs the crate can check it.**
  Operationally — when a comment says *"X never contains Y"*, that is a testable
  assertion about the system, so write the test. **If it cannot be written as a
  test, it should not be written as a statement of fact**; write what the file
  itself does and stop at its boundary.

  And the test has to be a WHOLE-ARTEFACT scan, not a field-by-field one: the
  first `chat_scope` fix checked the fields someone had listed, and two of three
  leaks survived it. Serialise the thing and scan the bytes, so a field added
  tomorrow is covered by a test nobody updates.

- **A Bash call refused by the permission classifier runs NOTHING, including the
  parts you later assume ran. `git status` is the only thing that catches it.**
  13 Sep 2026: a single call combined a `python3` patch of `orchestrator.rs`
  with `cargo test --workspace`. The classifier was briefly unavailable and
  refused the whole call. The test half was re-run on its own and went green —
  so the session had a passing suite, a code comment that had never been
  written, and no error anywhere pointing at the gap.

  It surfaced only at `git add`: the file was absent from
  `git diff --cached --name-only`, and `grep -c D153 orchestrator.rs` returned
  `0`. Nothing else would have found it — the change was a comment, so no test
  could fail on its absence.

  **Do not combine an edit and its verification in one Bash call.** If the call
  is refused or interrupted, the two halves have different fates and the
  verification is the half more likely to be retried. And before staging, read
  `git diff --cached --name-only` against the list of files you believe you
  edited, rather than trusting that each edit landed.

- **A containment assertion over generated markup cannot fail on what is ADDED.
  `includes()` is blind to a wrapper.** The entry above is about measuring the
  wrong thing; this one is about measuring the right thing with an instrument
  that cannot register the defect.

  Building the journal-styled .docx, docx 7.8.2's
  `ImportedXmlComponent.fromXmlString` returned a component whose `rootKey` was
  `undefined`, so the packer wrote every formula inside a literal element:

  ```xml
  <w:p><undefined><m:oMath …>…</m:oMath></undefined></w:p>
  ```

  `xml.includes('m:oMath')` — **true**. `xml.includes('<m:f>')`, `'<m:nary>'`,
  `'<m:m>'` — all **true**. `xmllint --noout` — **clean**, because that IS
  well-formed XML. Word refused the entire file: *"Word experienced an error
  trying to open the file."* Every assertion was satisfied by a document no
  consumer would accept.

  **This is the INVERSE of the teardown's §13, and the pair is the point.** §13
  established that Word's tolerance is wide — it accepted a PNG with a bad CRC
  and rendered nothing — so a clean open proves well-formedness within that
  tolerance, not correctness. The natural next thought is "so the XML checks are
  the real floor and Word is the extra." Exactly backwards. **The XML checks
  were never a floor either.** Word accepts invalid *resources*; containment
  assertions accept invalid *structure*. Neither is sufficient and they fail in
  opposite directions, which is why both are needed and why neither can stand in
  for the other.

  **The correction that matters is to the story, not just the method.** The
  feasibility check that opened that task declared two-column layout and raw
  OMML "proved at the XML level" on exactly these assertions. That file does not
  open in Word. So the sequence was NOT *"we had XML verification, then we added
  Word verification on top"*. It was *"there was no verification, and then there
  was one"*. A record that reads the first way makes the XML stage look like a
  weaker tier of the same thing; it was not a tier, it was a null result wearing
  a green tick.

  So, for any generated packaged or binary format (.docx, .zip, images, and the
  .tex bundle's own parts):
  - Pair every "contains X" with a NEGATIVE structural assertion — *does not*
    contain a wrapper, an unknown element, an `undefined`. The pin that now
    guards this is `expect(xml).not.toContain('<undefined>')`.
  - Well-formed is not valid. `xmllint` answers a different question than a
    schema, and neither answers what a real consumer does.
  - Open it in the real consumer, and hold the claim until you have. Combined
    with the entry above: predict the failure, break it on purpose, and confirm
    the assertion actually goes red — reinstating `fromXmlString` fails the pin,
    removing the column property fails the layout pins.
