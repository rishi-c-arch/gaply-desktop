# Gaply — Packaging & Distribution

Final packaging documentation (Prompt 22). **Honest scope up front:**

| | Status |
|---|---|
| Full Rust core (6-agent swarm, debate, report compiler, security layer) | **Packaged & working today** — 171 tests, Windows CI green |
| Tauri desktop shell, MSI/NSIS installers | **Working today** — proven by CI installer artifacts |
| macOS DMG build path | **Measured 12 Sep 2026** — `npm run tauri build` completes; 480 MB DMG, arm64 only, models bundled, validly ad-hoc signed. Notarization still needs the Developer Program. See §1a. |
| Tauri updater | **Configured** (keys generated, pubkey embedded, endpoint set); activation is a documented 3-step flip |
| Microsoft Store submission | **Documented** — MSI/NSIS acceptable as Win32 app; MSIX wrap is the zero-cert path |
| Real local SLM inference (Ollama / Qwen / MiniLM / GPT-2) | **NOT wired in** — future path documented here; the app ships on interim proxies (`HeuristicModel`, `HashEmbedder`) by design |

## 1. Installers (CI: `.github/workflows/package-release.yml`)

Manual-dispatch-only (same Vercel isolation as the Windows check). Choose
`windows`, `macos`, or `both`. Produces MSI + NSIS on Windows and DMG + .app on
macOS via `tauri-apps/tauri-action`, runs a **launch smoke test** on the packaged
binary (must stay alive 12s), and uploads artifacts.

### Windows code signing (Authenticode)
Unsigned installers work but show SmartScreen/unidentified-publisher warnings.
To sign: obtain a code-signing certificate (OV/EV from a CA, or **Azure Trusted
Signing** — the affordable modern route), then add repo secrets
`WINDOWS_CERTIFICATE` (base64 PFX) + `WINDOWS_CERTIFICATE_PASSWORD`. The
workflow passes them through; builds sign automatically once present.

### macOS signing + notarization (documented; requires Apple Developer Program, $99/yr)
CI cannot execute Apple's notarization without credentials. The steps once enrolled:
1. Create a **Developer ID Application** certificate; export as .p12.
2. Repo secrets: `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`,
   `APPLE_SIGNING_IDENTITY` ("Developer ID Application: Name (TEAMID)"),
   `APPLE_ID`, `APPLE_PASSWORD` (app-specific password), `APPLE_TEAM_ID`.
3. tauri-action then signs the .app and submits the DMG via `notarytool`
   (`xcrun notarytool submit gaply.dmg --apple-id … --team-id … --wait`),
   then staples: `xcrun stapler staple gaply.dmg`.
Without these, the DMG builds but Gatekeeper will block it on other Macs.

### 1a. What the macOS build actually produces (measured 12 Sep 2026)

Everything above §1a was written from Tauri's documentation. This section was
written from the artefact. The two disagreed on the point that matters most.

`npm run tauri build` on `main` at 47d3692, on an M-series Mac:

| | |
|---|---|
| `gaply_0.1.0_aarch64.dmg` | **480 MB** |
| `gaply.app` | 523 MB |
| architecture | **`arm64` only** — no `x86_64-apple-darwin` target is installed, so there is nothing here for an Intel Mac |
| bundled models | yes — `Contents/Resources/models/stage1-lm/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf` (379 MB) and `slm1-adapter/tokenizer.json` (11 MB). They are ~75% of the download. |
| build time | ~1m25s of cargo on a warm `target/`, plus bundling |

**It first failed on disk, not on code:** `error: failed to build archive …
No space left on device (os error 28)`, with 1.8 GiB free. `src-tauri/target/debug`
was 13 GB. Nothing in the norms needs that directory — the standing rule is
`tauri dev -- --release` for anything touching a model — so deleting it is the
cheap fix, and the release build needs roughly 3 GB of headroom.

#### The signature: "damaged, move to Trash" was a four-character config fix

**Unsigned and invalidly signed are not the same state, and the default was the
second one.** With no `bundle.macOS.signingIdentity`, Tauri ships whatever the
Rust linker left on the binary and never seals the bundle:

```
$ codesign -dv --verbose=4 gaply.app
CodeDirectory ... flags=0x20002(adhoc,linker-signed)
Signature=adhoc
Sealed Resources=none                 # and no Contents/_CodeSignature/ at all

$ codesign --verify --deep --strict gaply.app
gaply.app: code has no resources but signature indicates they must be present
```

An embedded signature that claims resources are sealed, over a bundle where
none are. `codesign --verify`, `spctl --assess` and macOS's own
`syspolicy_check distribution` all reject it, the last calling it a **Fatal
"Codesign Error"**. That is the state that gets a recipient *"gaply is damaged
and can't be opened. You should move it to the Trash."* — which reads as a
corrupted download, not as an unsigned app, so the recipient's instinct is to
delete it rather than to ask.

Adding `"signingIdentity": "-"` makes Tauri re-sign the bundle ad-hoc during
bundling, and everything above inverts:

```
CodeDirectory ... flags=0x10002(adhoc,runtime)    # hardened runtime, too
Identifier=ai.gaply.app                            # was app-9b8499b4806e28fa
Sealed Resources version=2 rules=13 files=5
$ codesign --verify --deep --strict gaply.app     # exit 0
gaply.app: valid on disk
gaply.app: satisfies its Designated Requirement
```

`syspolicy_check distribution` then reports exactly one Fatal — **"A Notarization
ticket is not stapled"** — which is the ordinary unsigned-app path: *"Apple could
not verify 'gaply' is free of malware"*, with **Open Anyway** in System Settings
→ Privacy & Security. Verified inside the mounted DMG, not just on the loose
`.app`, since the DMG is what a recipient opens.

Ad-hoc is **not** a substitute for a Developer ID and does not skip §1's
`$99/yr`. It buys one thing: the app becomes honestly *unsigned* instead of
*invalidly signed*, and the recipient gets a dialog with a way through it.

Hardened runtime arrives with it (Tauri passes `--options runtime`), which can
break an app that needs JIT or unsigned executable memory. It does not break
this one: the signed bundle launches, reaches `AI engine device: metal`, and
stays up.

#### How the two-line change was verified — locally, not by CI

**Nothing in CI parses `tauri.conf.json`.** `clean-checkout` and `latex-compile`
never build the app crate; `frontend-build` mentions `tauri build` only in a
comment; `windows-build-check`'s **Build Tauri Windows app** step is `if:`-gated
on the gitignored bundled models and is `skipped` on every run; `package-release`
would parse it, but is dispatch-only and has never run. All three workflows went
green on this change and none of them read the file.

So the evidence is the artefact: `npm run tauri build` logging
`Signing with identity "-"`, `codesign --verify --deep --strict` exiting 0 on
the app **inside the mounted DMG**, and `LSMinimumSystemVersion` read back as
`11.0` from the installed plist.

That is weaker CI coverage but not a missing gate, and the distinction is
measured rather than argued: with the key deliberately mis-cased to `"macos"`,
`npx tauri build` exits 1 in **0 seconds with 0 crates compiled** —
*"Additional properties are not allowed ('macos' was unexpected)"*. Tauri
schema-validates the config before compiling anything, so a broken config
cannot reach a DMG. The config is guarded by the release build itself; see
CLAUDE.md, "Nothing in CI parses `tauri.conf.json`".

#### `LSMinimumSystemVersion` was wrong twice over

Tauri's default is `10.13`, hardcoded in `tauri-utils`
(`fn macos_minimum_system_version() -> Some("10.13")`), and it was reaching
`Info.plist` unchallenged. But the binary has never agreed with it:

```
$ vtool -show-build-version gaply.app/Contents/MacOS/app
   minos 11.0
     sdk 15.2
```

`tauri-build` does emit `MACOSX_DEPLOYMENT_TARGET=10.13`; the linker then clamps
`aarch64-apple-darwin` to 11.0, because **arm64 macOS did not exist before Big
Sur**. So 10.13 described no build that was ever produced, and on an Intel Mac
old enough to believe it, the app would install and not run. `11.0` is now set
explicitly, which changes no code — it makes the plist agree with the Mach-O.

#### What a recipient sees on a machine that is not Rishi's

Simulated by launching with `HOME` pointed at an empty directory, which is
exactly the input `models/mod.rs` branches on — no `~/gaply-models`, no
Application Support, nothing:

1. **The bundled model is found.** `model resolution: stage-1 LM source="bundled"`,
   both files present, resolved out of `Contents/Resources`. The `env → ~/gaply-models
   → bundled` precedence is real and the last leg is now exercised, not just
   documented. (On Rishi's own machine it never is — `~/gaply-models` wins, so
   every local launch tests the wrong branch.)
2. **Semantic search is off until they turn it on.** `embedding engine
   state=NotInstalled`. BGE is not bundled and is never downloaded at startup
   by design (`load_if_installed` performs no network call, guarded by a test);
   acquisition is an explicit in-app action needing network. So RAG, semantic
   search and plagiarism are inert on first run and the app says so. Worth
   telling three researchers in advance rather than letting them find it.
3. **Two `ERROR r2d2: database is locked` lines on first run only**, during the
   21-migration burst against the empty DB. It recovers and boots (`applied=21`,
   then `gaply desktop started`), but a fresh install logs errors it does not
   act on.

## 2. Tauri updater (configured for future auto-update)

Done already:
- Keypair generated (`~/.tauri/gaply-updater.key` — **private, NOT in the repo**;
  keep it safe: losing it means updates can never be signed again).
- Public key embedded in `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
- Endpoint set to the GitHub Releases `latest.json` convention.

To ACTIVATE auto-updates (3 steps, deliberately not flipped yet — the updater
plugin pulls a full HTTP stack into the app and deserves its own CI round-trip):
1. `cargo add tauri-plugin-updater` in `src-tauri/` + `.plugin(tauri_plugin_updater::Builder::new().build())`
   in the shell builder, and grant `updater:default` in the capability file.
2. Set `bundle.createUpdaterArtifacts: true` in `tauri.conf.json`.
3. Add repo secret `TAURI_SIGNING_PRIVATE_KEY` (contents of the private key
   file; password secret empty). CI then emits signed `.sig` files + `latest.json`
   to attach to a GitHub Release.

## 3. Microsoft Store (free individual developer account)

The Store accepts **Win32 apps packaged as EXE/MSI** — our existing MSI/NSIS
CI output is submission-compatible, with one caveat: **Win32 submissions are
not signed by the Store**, so a real Authenticode signature (§1) is required
first. The alternative — and the **zero-certificate path**: wrap the app as
**MSIX** (MSIX Packaging Tool or `makeappx` + `signtool`), submit that; the
Store signs MSIX packages itself. Tauri does not emit MSIX natively, so that is
the one additional packaging step needed. Identity values (Package/Publisher
name) come from Partner Center after reserving the app name.

## 4. Ollama first-run setup (FUTURE SLM path — not wired in yet)

`scripts/ollama-first-run.sh` (tested by `scripts/test-ollama-first-run.sh`):
detects Ollama (guides install if missing), persists the env pair, applies
RAM-aware guidance, and pre-pulls `all-minilm`.

### CRITICAL environment pair
```
OLLAMA_FLASH_ATTENTION=1
OLLAMA_KV_CACHE_TYPE=q8_0
```
**Why both:** Ollama ships with Flash Attention **off** by default, and KV-cache
quantization **only takes effect when Flash Attention is enabled** — setting
`q8_0` alone silently does nothing. The script sets and persists both
(launchctl on macOS; systemd override on Linux; `[Environment]::SetEnvironmentVariable`
on Windows — exact commands printed by the script).

### RAM-aware model guidance (for when real SLM integration lands)
- **≥ 16 GB free RAM → Qwen2.5-7B** (`ollama pull qwen2.5:7b`)
- **below that → Qwen3-4B** (`ollama pull qwen3:4b`)

### Model map (interim proxy → future real model)
| Seam in gaply_core | Ships today | Future |
|---|---|---|
| `Embedder` | `HashEmbedder` (384-dim, MiniLM-shaped) | `all-MiniLM-L6-v2` (pre-pulled by the script) |
| `PerplexityModel` | `HeuristicModel` | GPT-2-class model — **not in the Ollama library**; will ship via candle/GGUF or a custom Modelfile |
| Verification LLM | Claude via proxy (cloud) | optionally local Qwen2.5-7B / Qwen3-4B once trained |

Nothing in `gaply_core` calls Ollama today — the seams are the swap points, and
the first-run script just stages the machine so those swaps are instant.

## 5. What to run

```bash
# package Windows installers (proven path):
gh workflow run "Package Release" --ref <branch> -f platform=windows
# package macOS DMG (unsigned until Apple secrets exist):
gh workflow run "Package Release" --ref <branch> -f platform=macos
# local first-run staging (safe; app doesn't use the models yet):
./scripts/ollama-first-run.sh
```
