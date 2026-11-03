# Quokka &lt;3&gt; &amp; \*zebra\* \[draft\]\(x\) \`\#1\`

A Gaply project report. It holds fingerprints (SHA-256) of the data file, the analysis settings and the results, and none of the data itself. A column is named by its position and its role, never by its header.

- Project: `0123456789abcdef0123456789abcdef`
- Created: 2026-10-19T06:52:31Z
- Created by: Gaply 0.1.0
- Saved runs: 2

## Run 1: `stats.describe`, version 1

- Status: succeeded
- Started: 2026-10-19T07:52:31Z
- Ended: 2026-10-19T07:52:32Z
- Saved: 2026-10-19T07:53:31Z
- Carried out by: Gaply 0.1.0
- Engine version: 1
- Platform: macos aarch64
- Reproducibility: bitwise: the same inputs, engine version and platform give the same bytes

### Settings

- Data file as read: 12 rows, 7 columns; 1 prepared column
- Prepared by stats.prepare.scale_score version 1, engine version 1
- Prepared column 8: mean of columns 3, 4 and 7; column 4 reversed on 1 to 5; at least 2 items; 1 missing code
- Column 8: summarised_numeric
- Missing-value codes: 0 (their values are in the settings fingerprint)
- Random seed: none

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 812 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 301 | `2222222222222222222222222222222222222222222222222222222222222222` |
| used, recipe | recipe | json | 266 | `3333333333333333333333333333333333333333333333333333333333333333` |
| generated | result_bundle | json | 2048 | `4444444444444444444444444444444444444444444444444444444444444444` |
| generated | run_manifest | json | 640 | `5555555555555555555555555555555555555555555555555555555555555555` |

### Fingerprints and counts

- Input 1 SHA-256: `8888888888888888888888888888888888888888888888888888888888888888`
- Analysis settings SHA-256: `bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`
- Result SHA-256: `cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc`
- Rows read: 12; rows used: 12
- Time taken: 3 ms

### Left out

- None

### Limitations

- None

### Notes

- None

## Run 2: `stats.analyze.independent_t`, version 1

- Status: failed
- Failure code: `stats.ttest.both_groups_constant`
- Started: 2026-10-19T07:52:51Z
- Ended: 2026-10-19T07:52:52Z
- Saved: 2026-10-19T07:53:51Z
- Carried out by: Gaply 0.1.0

### Settings

- Data file as read: 12 rows, 7 columns; 1 prepared column
- Prepared by stats.prepare.scale_score version 1, engine version 1
- Prepared column 8: mean of columns 3, 4 and 7; column 4 reversed on 1 to 5; at least 2 items; 1 missing code
- Column 8: outcome
- Column 5: group
- Levels compared: the group's two levels, in code-point order
- Variant: welch
- Missing-value codes: 0 (their values are in the settings fingerprint)
- Random seed: none

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 812 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 290 | `6666666666666666666666666666666666666666666666666666666666666666` |
| used, recipe | recipe | json | 266 | `3333333333333333333333333333333333333333333333333333333333333333` |
| generated | run_failure | json | 120 | `7777777777777777777777777777777777777777777777777777777777777777` |

### Left out

- None

### Limitations

- None

### Notes

- None
