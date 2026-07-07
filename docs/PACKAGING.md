# Gaply — Packaging & Distribution

Final packaging documentation (Prompt 22). **Honest scope up front:**

| | Status |
|---|---|
| Full Rust core (6-agent swarm, debate, report compiler, security layer) | **Packaged & working today** — 171 tests, Windows CI green |
| Tauri desktop shell, MSI/NSIS installers | **Working today** — proven by CI installer artifacts |
| macOS DMG build path | **Builds today**; signing/notarization documented, needs Apple Developer Program |
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
