# Rerun TITLESENTz71

A Gaply project report. It holds fingerprints (SHA-256) of the data file, the analysis settings and the results, and none of the data itself. A column is named by its position and its role, never by its header.

- Project: `0123456789abcdef0123456789abcdef`
- Created: 2026-10-19T07:46:40Z
- Created by: Gaply 0.1.0
- Saved runs: 5

## Run 1: `stats.describe`, version 1

- Status: succeeded
- Started: 2026-10-19T07:52:41Z
- Ended: 2026-10-19T07:52:42Z
- Saved: 2026-10-19T07:52:46Z
- Carried out by: Gaply 0.1.0
- Engine version: 1
- Platform: macos aarch64
- Reproducibility: bitwise: the same inputs, engine version and platform give the same bytes

### Settings

- None recorded

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 100 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 100 | `2222222222222222222222222222222222222222222222222222222222222222` |
| generated | result_bundle | json | 100 | `3333333333333333333333333333333333333333333333333333333333333333` |
| generated | run_manifest | json | 100 | `4444444444444444444444444444444444444444444444444444444444444444` |

### Fingerprints and counts

- Input 1 SHA-256: `1111111111111111111111111111111111111111111111111111111111111111`
- Analysis settings SHA-256: `bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`
- Result SHA-256: `eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee`
- Rows read: 6; rows used: 6
- Time taken: 3 ms

### Left out

- None

### Limitations

- None

### Notes

- None

## Run 2: `stats.analyze.independent_t`, version 1

- Status: succeeded
- Started: 2026-10-19T07:52:51Z
- Ended: 2026-10-19T07:52:52Z
- Saved: 2026-10-19T07:52:56Z
- Carried out by: Gaply 0.1.0
- Engine version: 1
- Platform: macos aarch64
- Reproducibility: bitwise: the same inputs, engine version and platform give the same bytes

### Settings

- None recorded

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 100 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 100 | `5555555555555555555555555555555555555555555555555555555555555555` |
| generated | result_bundle | json | 100 | `6666666666666666666666666666666666666666666666666666666666666666` |
| generated | run_manifest | json | 100 | `7777777777777777777777777777777777777777777777777777777777777777` |

### Fingerprints and counts

- Input 1 SHA-256: `1111111111111111111111111111111111111111111111111111111111111111`
- Analysis settings SHA-256: `bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`
- Result SHA-256: `ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff`
- Rows read: 6; rows used: 6
- Time taken: 3 ms

### Left out

- None

### Limitations

- None

### Notes

- None

## Run 3: `stats.describe`, version 1

- Status: succeeded
- Repeats run 1. Its result is identical, byte for byte, to run 1's.
- Started: 2026-10-19T07:53:01Z
- Ended: 2026-10-19T07:53:02Z
- Saved: 2026-10-19T07:53:06Z
- Carried out by: Gaply 0.1.0
- Engine version: 1
- Platform: macos aarch64
- Reproducibility: bitwise: the same inputs, engine version and platform give the same bytes

### Settings

- None recorded

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 100 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 100 | `2222222222222222222222222222222222222222222222222222222222222222` |
| generated | result_bundle | json | 100 | `8888888888888888888888888888888888888888888888888888888888888888` |
| generated | run_manifest | json | 100 | `4444444444444444444444444444444444444444444444444444444444444444` |

### Fingerprints and counts

- Input 1 SHA-256: `1111111111111111111111111111111111111111111111111111111111111111`
- Analysis settings SHA-256: `bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`
- Result SHA-256: `eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee`
- Rows read: 6; rows used: 6
- Time taken: 3 ms

### Left out

- None

### Limitations

- None

### Notes

- `stats.desc.suspect_missing_code` (column 1, count 2)

## Run 4: `stats.analyze.independent_t`, version 1

- Status: succeeded
- Repeats run 2. Its result is not identical, byte for byte, to run 2's. Both are kept; Gaply does not judge which is right.
- Started: 2026-10-19T07:53:11Z
- Ended: 2026-10-19T07:53:12Z
- Saved: 2026-10-19T07:53:16Z
- Carried out by: Gaply 0.1.0
- Engine version: 1
- Platform: macos aarch64
- Reproducibility: bitwise: the same inputs, engine version and platform give the same bytes

### Settings

- None recorded

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 100 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 100 | `5555555555555555555555555555555555555555555555555555555555555555` |
| generated | result_bundle | json | 100 | `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` |
| generated | run_manifest | json | 100 | `cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc` |

### Fingerprints and counts

- Input 1 SHA-256: `1111111111111111111111111111111111111111111111111111111111111111`
- Analysis settings SHA-256: `bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`
- Result SHA-256: `9999999999999999999999999999999999999999999999999999999999999999`
- Rows read: 6; rows used: 6
- Time taken: 3 ms

### Left out

- None

### Limitations

- None

### Notes

- None

## Run 5: `stats.describe`, version 1

- Status: failed
- Repeats run 1.
- Failure code: `stats.ttest.both_groups_constant`
- Started: 2026-10-19T07:53:21Z
- Ended: 2026-10-19T07:53:22Z
- Saved: 2026-10-19T07:53:26Z
- Carried out by: Gaply 0.1.0

### Settings

- None recorded

### Inputs and outputs

| | kind | format | bytes | SHA-256 |
|---|---|---|---|---|
| used, data | data_file | csv | 100 | `1111111111111111111111111111111111111111111111111111111111111111` |
| used, spec | operation_spec | json | 100 | `2222222222222222222222222222222222222222222222222222222222222222` |
| generated | run_failure | json | 100 | `dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd` |

### Left out

- None

### Limitations

- None

### Notes

- None
