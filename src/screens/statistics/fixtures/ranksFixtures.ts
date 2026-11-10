// Typed copies of the rank tests' view fixtures (Research OS, Phase 8Y-2, decision
// 619): each `satisfies` its view's type, so tsc catches a renamed or missing key or
// a word outside its closed set, and ranksTypes.vitest.ts deep-compares each with the
// JSON Rust serialises (src-tauri/tests/stats_ranks_wire.rs pins those bytes).
// Generated from the JSON once; edit both together.
import type { RanksResultView, RanksRunStateView, RanksValidationView } from '../ranksTypes';

export const ranksValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.ranks.group_empty",
      "params": {
        "index": 1
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.ranks.suspect_missing_code",
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
      "code": "stats.ranks.same_column",
      "params": {},
      "severity": "error"
    }
  ],
  "estimatedResources": {
    "rows": 21,
    "columns": 2,
    "cells": 42
  }
} satisfies RanksValidationView;

export const ranksRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 0,
  "stepsTotal": 1,
  "diagnostics": [
    {
      "code": "stats.ranks.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies RanksRunStateView;

export const ranksResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "groups": [
      {
        "level": 0,
        "label": "control",
        "n": 4,
        "median": 2.5,
        "rankSum": 13.0,
        "meanRank": 3.25
      },
      {
        "level": 2,
        "label": "treated",
        "n": 6,
        "median": 6.25,
        "rankSum": 42.0,
        "meanRank": 7.0
      }
    ],
    "n": 10,
    "tieRuns": 0,
    "test": {
      "mann_whitney": {
        "first": 0,
        "second": 2,
        "u1": 3.0,
        "u2": 21.0,
        "z": {
          "value": -1.917
        },
        "p": {
          "value": 0.0552
        },
        "exactP": 0.0667,
        "rankBiserial": -0.75
      }
    },
    "exclusions": [
      {
        "code": "stats.ranks.missing_group",
        "count": 2,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.ranks.other_level",
        "count": 5,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.ranks.missing_outcome",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.ranks.missing_code",
        "count": 3,
        "params": {
          "column": 1
        }
      }
    ],
    "notes": [
      "stats.ranks.group_below_five"
    ],
    "limitations": [
      "stats.ranks.no_pairwise_comparisons",
      "stats.ranks.two_sided_only",
      "stats.ranks.no_location_shift_estimate",
      "stats.ranks.exact_without_ties_only",
      "stats.ranks.no_further_k_sample_tests",
      "stats.ranks.no_weights",
      "stats.ranks.code_point_order"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.ranks.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.ranks.no_pairwise_comparisons",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.independent_ranks",
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
        "code": "stats.ranks.missing_group",
        "count": 2,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.ranks.other_level",
        "count": 5,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.ranks.missing_outcome",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.ranks.missing_code",
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
} satisfies RanksResultView;

export const ranksResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "groups": [
      {
        "level": 0,
        "label": "a",
        "n": 3,
        "median": 7.0,
        "rankSum": 15.0,
        "meanRank": 5.0
      },
      {
        "level": 1,
        "label": "b",
        "n": 3,
        "median": 7.0,
        "rankSum": 15.0,
        "meanRank": 5.0
      },
      {
        "level": 2,
        "label": "c",
        "n": 3,
        "median": 7.0,
        "rankSum": 15.0,
        "meanRank": 5.0
      }
    ],
    "n": 9,
    "tieRuns": 1,
    "test": {
      "kruskal_wallis": {
        "h": {
          "not_computable": "stats.ranks.all_values_tied"
        },
        "df": 2.0,
        "p": {
          "not_computable": "stats.ranks.all_values_tied"
        },
        "epsilonSquared": {
          "not_computable": "stats.ranks.all_values_tied"
        }
      }
    },
    "exclusions": [],
    "notes": [
      "stats.ranks.group_below_five"
    ],
    "limitations": [
      "stats.ranks.no_pairwise_comparisons",
      "stats.ranks.two_sided_only",
      "stats.ranks.no_location_shift_estimate",
      "stats.ranks.exact_without_ties_only",
      "stats.ranks.no_further_k_sample_tests",
      "stats.ranks.no_weights",
      "stats.ranks.code_point_order"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.ranks.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.ranks.no_pairwise_comparisons",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.independent_ranks",
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
        "code": "stats.ranks.missing_group",
        "count": 2,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.ranks.other_level",
        "count": 5,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.ranks.missing_outcome",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.ranks.missing_code",
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
} satisfies RanksResultView;
