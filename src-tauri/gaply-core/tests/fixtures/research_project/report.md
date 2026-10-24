# Quokka &lt;3&gt; &amp; \*zebra\* \[draft\]\(x\) \`\#1\`

A Gaply project report. It holds fingerprints (SHA-256) of the data file, the analysis settings and the results, and none of the data itself. A column is named by its position and its role, never by its header.

- Project: `0123456789abcdef0123456789abcdef`
- Created: 2026-10-19T06:52:31Z
- Created by: Gaply 0.1.0
- Saved runs: 3

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

- Data file as read: 12 rows, 3 columns
- Column 1: summarised_numeric
- Column 2: summarised_categorical
- Missing-value codes: 1 (their values are in the settings fingerprint)
- Random seed: none

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 812 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 301 | `2222222222222222222222222222222222222222222222222222222222222222` |
| generated | result_bundle | json | 2048 | `3333333333333333333333333333333333333333333333333333333333333333` |
| generated | run_manifest | json | 640 | `4444444444444444444444444444444444444444444444444444444444444444` |

### Fingerprints and counts

- Input 1 SHA-256: `1111111111111111111111111111111111111111111111111111111111111111`
- Analysis settings SHA-256: `bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`
- Result SHA-256: `cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc`
- Rows read: 12; rows used: 12
- Time taken: 3 ms

### Left out

- `stats.desc.missing_cell` (column 2): 4

### Limitations

- None

### Notes

- `stats.desc.suspect_missing_code` (column 2, count 3)

## Run 2: `stats.analyze.independent_t`, version 1

- Status: succeeded
- Started: 2026-10-19T07:52:41Z
- Ended: 2026-10-19T07:52:42Z
- Saved: 2026-10-19T07:53:41Z
- Carried out by: Gaply 0.1.0
- Engine version: 1
- Platform: macos aarch64
- Reproducibility: bitwise: the same inputs, engine version and platform give the same bytes

### Settings

- Data file as read: 12 rows, 3 columns
- Column 1: outcome
- Column 3: group
- Levels compared: index 0 (group 1) and index 2 (group 2), in code-point order
- Variant: welch
- Missing-value codes: 0 (their values are in the settings fingerprint)
- Random seed: 42

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 812 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 290 | `5555555555555555555555555555555555555555555555555555555555555555` |
| generated | result_bundle | json | 1500 | `6666666666666666666666666666666666666666666666666666666666666666` |
| generated | run_manifest | json | 650 | `7777777777777777777777777777777777777777777777777777777777777777` |

### Fingerprints and counts

- Input 1 SHA-256: `1111111111111111111111111111111111111111111111111111111111111111`
- Analysis settings SHA-256: `dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd`
- Result SHA-256: `eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee`
- Rows read: 12; rows used: 10
- Time taken: 3 ms

### Left out

- None

### Limitations

- `stats.ttest.normality_not_assessed`
- `stats.ttest.cohens_d_has_no_interval`

### Notes

- None

## Run 3: `stats.analyze.independent_t`, version 1

- Status: failed
- Failure code: `stats.ttest.both_groups_constant`
- Started: 2026-10-19T07:52:51Z
- Ended: 2026-10-19T07:52:52Z
- Saved: 2026-10-19T07:53:51Z
- Carried out by: Gaply 0.1.0

### Settings

- Data file as read: 4 rows, 2 columns
- Column 1: outcome
- Column 2: group
- Levels compared: the group's two levels, in code-point order
- Variant: student
- Missing-value codes: 0 (their values are in the settings fingerprint)
- Random seed: none

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | xlsx | 9001 | `8888888888888888888888888888888888888888888888888888888888888888` |
| used, spec | operation_spec | json | 288 | `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` |
| generated | run_failure | json | 120 | `9999999999999999999999999999999999999999999999999999999999999999` damaged: project.artifact_wrong_hash |

### Left out

- None

### Limitations

- None

### Notes

- None
