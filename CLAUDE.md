# Gaply — project notes for Claude Code

Gaply is a research-integrity desktop app: a CRA React frontend wrapped in a
Tauri 2.0 shell (`src-tauri/`), with all business logic in the portable,
Tauri-free Rust crate `src-tauri/gaply-core/`, plus a standalone FastAPI proxy
(`gaply-proxy/`, Tailscale-hidden) that is the ONLY path to the cloud.

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
  - before a commit — `cargo test --workspace` (~93s, 752 tests)

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

  **CI now enforces this too**, which it did not before. `.github/workflows/
  clean-checkout.yml` runs on every push and PR and builds + tests `gaply_core`
  from the checkout — a CI checkout IS the committed tree, so a missing
  `git add` fails there. It is deliberately cheap: no Node, no Tauri
  prerequisites, no bundled model. It also runs the script itself, so a guard
  that rots fails rather than passing quietly.

  `windows-build-check.yml` is no longer dispatch-only either — it now runs on
  PRs and on main. Its dispatch-only setting existed to stay clear of a Vercel
  deploy from the old `origin`, which was deleted in the phase-0 purge and 404s;
  that constraint is gone. It stays off every-push because it is heavy (npm ci,
  a full Tauri Windows build); the cheap workflow carries the per-push guard.
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
  The old `origin` (RishiSTARP/gaply-react-frontend, Vercel-deployed) is gone —
  404s since the phase-0 history purge; its stale local refs don't reflect
  reality. Feature work rides stacked `feat/*` branches off main (reviewed
  checkpoint commits kept unsquashed; fast-forward merges to main).
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
