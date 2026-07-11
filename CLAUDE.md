# Gaply — project notes for Claude Code

Gaply is a research-integrity desktop app: a CRA React frontend wrapped in a
Tauri 2.0 shell (`src-tauri/`), with all business logic in the portable,
Tauri-free Rust crate `src-tauri/gaply-core/`, plus a standalone FastAPI proxy
(`gaply-proxy/`, Tailscale-hidden) that is the ONLY path to the cloud.

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

- Commit ONLY when Rishi provides/approves the message; never push unprompted.
- `origin/main` is Vercel-deployed: single-file, verified pushes only, and only
  when explicitly requested. Feature work rides `feat/datamaestro-pro-pwa-security`.
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
