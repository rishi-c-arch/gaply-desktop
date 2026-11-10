// Typed copies of the related rank tests' view fixtures (Research OS, Phase 8Z-2, decision
// 653): each `satisfies` its view's type, so tsc catches a renamed or missing key or a
// word outside its closed set, and relatedRanksTypes.vitest.ts deep-compares each with the
// JSON Rust serialises (src-tauri/tests/stats_related_ranks_wire.rs pins those bytes).
// Generated from the JSON once; edit both together.
import type { RelatedRanksResultView, RelatedRanksRunStateView, RelatedRanksValidationView } from '../relatedRanksTypes';

export const relatedRanksValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.related_ranks.column_repeated",
      "params": {
        "column": 1
      },
      "severity": "error"
    },
    {
      "code": "stats.related_ranks.too_few_columns",
      "params": {
        "count": 1
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.related_ranks.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ],
  "missingInformation": [
    {
      "code": "stats.related_ranks.no_complete_rows",
      "params": {},
      "severity": "error"
    }
  ],
  "estimatedResources": {
    "rows": 21,
    "columns": 2,
    "cells": 42
  }
} satisfies RelatedRanksValidationView;

export const relatedRanksRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 0,
  "stepsTotal": 1,
  "diagnostics": [
    {
      "code": "stats.related_ranks.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies RelatedRanksRunStateView;

export const relatedRanksResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "columns": [
      {
        "column": "C1",
        "median": 2.5
      },
      {
        "column": "C2",
        "median": 4.75
      }
    ],
    "rowsUsed": 6,
    "test": {
      "wilcoxon": {
        "first": "C1",
        "second": "C2",
        "n": 6,
        "zeroDifferences": 0,
        "tieRuns": 0,
        "negative": {
          "count": 5,
          "rankSum": 20,
          "meanRank": 4
        },
        "positive": {
          "count": 1,
          "rankSum": 1,
          "meanRank": 1
        },
        "v": 1,
        "tMinus": 20,
        "z": {
          "value": -1.992
        },
        "p": {
          "value": 0.0464
        },
        "exactP": 0.0625,
        "rankBiserial": {
          "value": -0.904
        },
        "medianDifference": {
          "value": -2.25
        }
      }
    },
    "exclusions": [
      {
        "code": "stats.related_ranks.missing_value",
        "count": 2,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.related_ranks.missing_code",
        "count": 3,
        "params": {
          "column": 1
        }
      }
    ],
    "notes": [
      "stats.related_ranks.rows_below_five"
    ],
    "limitations": [
      "stats.related_ranks.no_pairwise_comparisons",
      "stats.related_ranks.two_sided_only",
      "stats.related_ranks.no_location_estimate",
      "stats.related_ranks.exact_without_ties_or_zeros_only",
      "stats.related_ranks.pratt_not_offered",
      "stats.related_ranks.no_further_related_tests",
      "stats.related_ranks.no_weights",
      "stats.related_ranks.column_order"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.related_ranks.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.related_ranks.no_pairwise_comparisons",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.related_ranks",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 21,
    "rowsUsed": 10,
    "exclusions": [
      {
        "code": "stats.related_ranks.missing_value",
        "count": 2,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.related_ranks.missing_code",
        "count": 3,
        "params": {
          "column": 1
        }
      }
    ],
    "platform": {
      "os": "macos",
      "arch": "aarch64"
    },
    "elapsedMs": 3,
    "outputHash": "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b",
    "reproducibility": "bitwise"
  }
} satisfies RelatedRanksResultView;

export const relatedRanksResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "columns": [
      {
        "column": "C1",
        "median": 7
      },
      {
        "column": "C2",
        "median": 7
      },
      {
        "column": "C3",
        "median": 7
      }
    ],
    "rowsUsed": 3,
    "test": {
      "friedman": {
        "columns": [
          {
            "column": "C1",
            "rankSum": 6,
            "meanRank": 2
          },
          {
            "column": "C2",
            "rankSum": 6,
            "meanRank": 2
          },
          {
            "column": "C3",
            "rankSum": 6,
            "meanRank": 2
          }
        ],
        "q": {
          "not_computable": "stats.related_ranks.all_values_tied"
        },
        "df": 2,
        "p": {
          "not_computable": "stats.related_ranks.all_values_tied"
        },
        "kendallsW": {
          "not_computable": "stats.related_ranks.all_values_tied"
        }
      }
    },
    "exclusions": [],
    "notes": [
      "stats.related_ranks.rows_below_five"
    ],
    "limitations": [
      "stats.related_ranks.no_pairwise_comparisons",
      "stats.related_ranks.two_sided_only",
      "stats.related_ranks.no_location_estimate",
      "stats.related_ranks.exact_without_ties_or_zeros_only",
      "stats.related_ranks.pratt_not_offered",
      "stats.related_ranks.no_further_related_tests",
      "stats.related_ranks.no_weights",
      "stats.related_ranks.column_order"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.related_ranks.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.related_ranks.no_pairwise_comparisons",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.related_ranks",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 21,
    "rowsUsed": 10,
    "exclusions": [
      {
        "code": "stats.related_ranks.missing_value",
        "count": 2,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.related_ranks.missing_code",
        "count": 3,
        "params": {
          "column": 1
        }
      }
    ],
    "platform": {
      "os": "macos",
      "arch": "aarch64"
    },
    "elapsedMs": 3,
    "outputHash": "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b",
    "reproducibility": "bitwise"
  }
} satisfies RelatedRanksResultView;
