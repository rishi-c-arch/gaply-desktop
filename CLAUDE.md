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

### Future swarm topology (how the agents map)

The agents are already independent, message-in/message-out units with injected
seams (`HttpFetcher`, `ProxyClient`, `Embedder`, `PerplexityModel`), so they map
onto a hierarchical swarm naturally:

- **Coordinator** (queen): manuscript intake → fan-out → report assembly.
- **Worker tier** (parallel, local, no cloud): Extraction first (it produces the
  shared claims/citations), then Validation, AI-Detection, Plagiarism, and RAG
  retrieval run concurrently over its output.
- **Gated tier**: Verification runs last, consuming refverify evidence; it is
  the single cloud egress and stays behind the proxy + harness gate.
- Rate limiters and the TTL cache are shared swarm resources; provenance tags
  make agent outputs mergeable without trust confusion.

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
- **Runtime (Prompt 19, shipped):** ruv-swarm gets EMBEDDED AS A RUNTIME
  LIBRARY inside the app to coordinate the six agents in production. That is a
  separate, in-app dependency with its own review/testing; nothing from the
  dev-time MCP setup ships to users.

## Working norms (standing)

- Commit ONLY when Rishi provides/approves the message; never push unprompted.
- `origin/main` is Vercel-deployed: single-file, verified pushes only, and only
  when explicitly requested. Feature work rides `feat/datamaestro-pro-pwa-security`.
- Windows CI (`.github/workflows/windows-build-check.yml`) runs
  `cargo test -p gaply_core` — the app crate's IPC tests are
  `#[cfg_attr(windows, ignore)]` (wry/tao load crash, see comments there).
- Be explicit about what's verified locally vs. what needs real infrastructure
  (AWS Nitro, Tailscale tailnet, Windows runners).
