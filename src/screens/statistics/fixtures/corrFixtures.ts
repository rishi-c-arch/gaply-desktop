// Typed copies of the correlations' view fixtures (Research OS, Phase 8H-2,
// decision 356): each `satisfies` its view's type, so tsc catches a renamed or
// missing key or a word outside its closed set, and corrTypes.vitest.ts
// deep-compares each with the JSON Rust serialises
// (src-tauri/tests/stats_corr_wire.rs pins those bytes). Generated from the
// JSON once; edit both together.
import type { CorrResultView, CorrRunStateView, CorrValidationView } from '../corrTypes';

export const corrValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.corr.variable_holds_text",
      "params": {
        "column": 4,
        "count": 10
      },
      "severity": "error"
    },
    {
      "code": "stats.corr.over_budget",
      "params": {
        "count": 780,
        "found": 100000
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.corr.suspect_missing_code",
      "params": {
        "column": 3,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ],
  "missingInformation": [],
  "estimatedResources": {
    "rows": 12,
    "columns": 3,
    "cells": 36
  }
} satisfies CorrValidationView;

export const corrRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 4,
  "stepsTotal": 10,
  "diagnostics": [
    {
      "code": "stats.corr.suspect_missing_code",
      "params": {
        "column": 3,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies CorrRunStateView;

export const corrResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "descriptives": [
      {
        "variable": "C1",
        "n": 9,
        "mean": {
          "value": 4.25
        },
        "sd": {
          "value": 1.5
        }
      },
      {
        "variable": "C3",
        "n": 8,
        "mean": {
          "value": -2.5
        },
        "sd": {
          "value": 0.75
        }
      },
      {
        "variable": "C4",
        "n": 1,
        "mean": {
          "value": 6.0
        },
        "sd": {
          "not_computable": "stats.corr.too_few_values"
        }
      }
    ],
    "coefficients": [
      {
        "coefficient": "pearson",
        "pairs": [
          {
            "first": "C1",
            "second": "C3",
            "n": 8,
            "outcome": {
              "computed": {
                "estimate": 0.61,
                "test": {
                  "t": {
                    "t": {
                      "value": 1.886
                    },
                    "df": 6.0
                  }
                },
                "p": 0.0123,
                "interval": {
                  "computed": {
                    "lower": -0.18,
                    "upper": 0.92
                  }
                }
              }
            }
          },
          {
            "first": "C1",
            "second": "C4",
            "n": 1,
            "outcome": {
              "not_computable": "stats.corr.pair_too_few_rows"
            }
          },
          {
            "first": "C3",
            "second": "C4",
            "n": 1,
            "outcome": {
              "not_computable": "stats.corr.pair_too_few_rows"
            }
          }
        ]
      },
      {
        "coefficient": "spearman",
        "pairs": [
          {
            "first": "C1",
            "second": "C3",
            "n": 8,
            "outcome": {
              "computed": {
                "estimate": 1.0,
                "test": {
                  "t": {
                    "t": {
                      "infinite": "stats.corr.perfect_coefficient"
                    },
                    "df": 6.0
                  }
                },
                "p": 0.0,
                "interval": {
                  "computed": {
                    "lower": 1.0,
                    "upper": 1.0
                  }
                }
              }
            }
          }
        ]
      },
      {
        "coefficient": "kendall",
        "pairs": [
          {
            "first": "C1",
            "second": "C3",
            "n": 8,
            "outcome": {
              "computed": {
                "estimate": 0.5,
                "test": {
                  "z": 1.732
                },
                "p": 0.0833,
                "interval": {
                  "computed": {
                    "lower": -0.25,
                    "upper": 0.875
                  }
                }
              }
            }
          }
        ]
      }
    ],
    "rowsUsed": 9,
    "exclusions": [
      {
        "code": "stats.corr.missing_value",
        "count": 2,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.corr.missing_code",
        "count": 1,
        "params": {
          "column": 3
        }
      }
    ],
    "limitations": [
      "stats.corr.pearson_assumes_normality",
      "stats.corr.two_tailed_no_partial"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.corr.suspect_missing_code",
      "params": {
        "column": 3,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.correlations",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 12,
    "rowsUsed": 9,
    "exclusions": [
      {
        "code": "stats.corr.missing_value",
        "count": 2,
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
    "outputHash": "9a3f0c5be2d14e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f",
    "reproducibility": "bitwise"
  }
} satisfies CorrResultView;

export const corrResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "descriptives": [
      {
        "variable": "C2",
        "n": 4,
        "mean": {
          "value": 3.0
        },
        "sd": {
          "value": 0.5
        }
      },
      {
        "variable": "C5",
        "n": 0,
        "mean": {
          "not_computable": "stats.corr.no_values"
        },
        "sd": {
          "not_computable": "stats.corr.too_few_values"
        }
      }
    ],
    "coefficients": [
      {
        "coefficient": "kendall",
        "pairs": [
          {
            "first": "C2",
            "second": "C5",
            "n": 4,
            "outcome": {
              "computed": {
                "estimate": -1.0,
                "test": {
                  "z": -2.04
                },
                "p": 0.0415,
                "interval": {
                  "not_computable": "stats.corr.interval_too_few_rows"
                }
              }
            }
          }
        ]
      }
    ],
    "rowsUsed": 4,
    "exclusions": [
      {
        "code": "stats.corr.fewer_than_two_values",
        "count": 3,
        "params": {}
      }
    ],
    "limitations": [
      "stats.corr.two_tailed_no_partial"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.corr.suspect_missing_code",
      "params": {
        "column": 3,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.correlations",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 12,
    "rowsUsed": 9,
    "exclusions": [
      {
        "code": "stats.corr.missing_value",
        "count": 2,
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
    "outputHash": "9a3f0c5be2d14e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f",
    "reproducibility": "bitwise"
  }
} satisfies CorrResultView;
