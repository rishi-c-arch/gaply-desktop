# AI Check eval fixtures (Phase 1, Set D)

The first cases of the detection eval set — pinned regressions for the
false-negative fix. Measured 0.5B perplexity (release) and expected placement:

| fixture | provenance | 0.5B perplexity | placement |
|---|---|---|---|
| `dense_ai.txt` | AI-written dense academic prose (proxy scored 0.00%) | 11.46 | below human median |
| `human_ref.txt` | genuinely human (Turing 1950 + Watson–Crick 1953) | 18.54 | within/above human |
| `mixed_ai.txt` | mostly-dense AI + sparse simple sentences | 21.99 (short, noisy) | — |
| `dense_ai_full.txt` | AI-written 5-section doc — STAND-IN for the original (not captured) | — | below human median |

Human norm (0.5B): median 17.5, p10 8.6, p90 48.3 (see calibration/stage1_norms.json).
