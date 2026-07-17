# Bundled models (build input)

Files Tauri copies into the packaged app's `Resources/models/` (Option D, Set 1):

| File | Size | Bundled? | tauri.conf dest |
|---|---|---|---|
| `Qwen2.5-0.5B-Instruct-Q4_K_M.gguf` | ~379 MB | yes (Stage-1 LM) | `models/stage1-lm/` |
| `tokenizer.json` | ~11 MB | yes (shared Qwen2.5 tokenizer) | `models/slm1-adapter/` |
| `THIRD-PARTY-LICENSES.txt` | — | yes | `THIRD-PARTY-LICENSES.txt` |

The `.gguf` and `tokenizer.json` are **gitignored** (binary/huge). Populate before
`cargo tauri build`:

```sh
cp ~/gaply-models/stage1-lm/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf src-tauri/bundled-models/
cp ~/gaply-models/slm1-adapter/tokenizer.json                 src-tauri/bundled-models/
```

Provenance + license: bartowski GGUF quant of `Qwen/Qwen2.5-0.5B-Instruct`,
Apache-2.0 (see `THIRD-PARTY-LICENSES.txt`). The 1.5B/7B deep verifiers are NOT
bundled — they download on demand (a later set).
