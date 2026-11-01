// Typed copies of the one-way ANOVA's view fixtures (Research OS, Phase 8G-2,
// decision 318): each `satisfies` its view's type, so tsc catches a renamed or
// missing key or a word outside its closed set, and anovaTypes.vitest.ts
// deep-compares each with the JSON Rust serialises
// (src-tauri/tests/stats_anova_wire.rs pins those bytes). Generated from the
// JSON once; edit both together.
import type { AnovaResultView, AnovaRunStateView, AnovaValidationView } from '../anovaTypes';

export const anovaValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.anova.group_too_small",
      "params": {
        "count": 1,
        "index": 1
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.anova.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ],
  "missingInformation": [
    {
      "code": "stats.anova.too_few_levels",
      "params": {
        "count": 1
      },
      "severity": "note"
    }
  ],
  "estimatedResources": {
    "rows": 17,
    "columns": 2,
    "cells": 34
  }
} satisfies AnovaValidationView;

export const anovaRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 20,
  "stepsTotal": 46,
  "diagnostics": [
    {
      "code": "stats.anova.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies AnovaRunStateView;

export const anovaResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "groups": [
      {
        "level": 0,
        "label": "ctrl",
        "n": 4,
        "mean": 2.5,
        "sd": 1.25,
        "se": 0.5,
        "ciLower": 1.5,
        "ciUpper": 3.5,
        "min": 0.5,
        "max": 4.5
      },
      {
        "level": 2,
        "label": "dose_hi",
        "n": 4,
        "mean": 11.5,
        "sd": 1.25,
        "se": 0.5,
        "ciLower": 10.5,
        "ciUpper": 12.5,
        "min": 9.5,
        "max": 13.5
      },
      {
        "level": 3,
        "label": "dose_lo",
        "n": 5,
        "mean": 5.75,
        "sd": 1.25,
        "se": 0.5,
        "ciLower": 4.75,
        "ciUpper": 6.75,
        "min": 3.75,
        "max": 7.75
      }
    ],
    "table": {
      "computed": {
        "ssBetween": 163.5,
        "ssWithin": 39.25,
        "ssTotal": 202.75,
        "dfBetween": 2.0,
        "dfWithin": 10.0,
        "msBetween": 81.75,
        "msWithin": 3.925,
        "f": {
          "value": 20.8
        },
        "p": {
          "value": 0.000271
        }
      }
    },
    "welch": {
      "not_computable": "stats.anova.welch_variance_zero"
    },
    "leveneMean": {
      "computed": {
        "f": 6.5,
        "df1": 2.0,
        "df2": 10.0,
        "p": 0.0155
      }
    },
    "leveneMedian": {
      "not_computable": "stats.anova.levene_deviations_equal"
    },
    "etaSquared": {
      "value": 0.806
    },
    "omegaSquared": {
      "not_computable": "stats.anova.within_variance_zero"
    },
    "postHoc": [
      {
        "test": "tukey",
        "pairs": {
          "computed": [
            {
              "first": 0,
              "second": 2,
              "outcome": {
                "computed": {
                  "difference": -9.0,
                  "se": 0.75,
                  "statistic": 4.5,
                  "df": 9.5,
                  "p": {
                    "below": 1e-10
                  },
                  "ciLower": -10.5,
                  "ciUpper": -7.5
                }
              }
            },
            {
              "first": 0,
              "second": 3,
              "outcome": {
                "computed": {
                  "difference": -3.25,
                  "se": 0.75,
                  "statistic": 4.5,
                  "df": 9.5,
                  "p": {
                    "value": 0.032
                  },
                  "ciLower": -4.75,
                  "ciUpper": -1.75
                }
              }
            }
          ]
        }
      },
      {
        "test": "games_howell",
        "pairs": {
          "computed": [
            {
              "first": 2,
              "second": 3,
              "outcome": {
                "not_computable": "stats.anova.pair_both_constant"
              }
            }
          ]
        }
      },
      {
        "test": "bonferroni",
        "pairs": {
          "not_computable": "stats.anova.within_variance_zero"
        }
      }
    ],
    "exclusions": [
      {
        "code": "stats.anova.other_level",
        "count": 4,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [
      "stats.anova.p_below_floor",
      "stats.anova.omega_squared_negative"
    ],
    "limitations": [
      "stats.anova.normality_not_assessed",
      "stats.anova.effect_sizes_have_no_interval",
      "stats.anova.no_contrasts"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.anova.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.one_way_anova",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 17,
    "rowsUsed": 13,
    "exclusions": [
      {
        "code": "stats.anova.other_level",
        "count": 4,
        "params": {
          "column": 2
        }
      }
    ],
    "platform": {
      "os": "macos",
      "arch": "aarch64"
    },
    "elapsedMs": 3,
    "outputHash": "5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a",
    "reproducibility": "bitwise"
  }
} satisfies AnovaResultView;

export const anovaResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "groups": [
      {
        "level": 0,
        "label": "a",
        "n": 3,
        "mean": 1.0,
        "sd": 1.25,
        "se": 0.5,
        "ciLower": 0.0,
        "ciUpper": 2.0,
        "min": -1.0,
        "max": 3.0
      },
      {
        "level": 1,
        "label": "b",
        "n": 3,
        "mean": 2.0,
        "sd": 1.25,
        "se": 0.5,
        "ciLower": 1.0,
        "ciUpper": 3.0,
        "min": 0.0,
        "max": 4.0
      }
    ],
    "table": {
      "not_computable": "stats.anova.outcome_constant"
    },
    "welch": {
      "computed": {
        "f": 3.5,
        "df1": 1.0,
        "df2": 3.75,
        "p": 0.14
      }
    },
    "leveneMean": {
      "not_computable": "stats.anova.outcome_constant"
    },
    "leveneMedian": {
      "computed": {
        "f": 0.5,
        "df1": 1.0,
        "df2": 4.0,
        "p": 0.52
      }
    },
    "etaSquared": {
      "not_computable": "stats.anova.outcome_constant"
    },
    "omegaSquared": {
      "value": -0.05
    },
    "postHoc": [
      {
        "test": "tukey",
        "pairs": {
          "not_computable": "stats.anova.outcome_constant"
        }
      },
      {
        "test": "games_howell",
        "pairs": {
          "computed": [
            {
              "first": 0,
              "second": 1,
              "outcome": {
                "computed": {
                  "difference": -1.0,
                  "se": 0.75,
                  "statistic": 4.5,
                  "df": 9.5,
                  "p": {
                    "value": 0.21
                  },
                  "ciLower": -2.5,
                  "ciUpper": 0.5
                }
              }
            }
          ]
        }
      },
      {
        "test": "bonferroni",
        "pairs": {
          "computed": [
            {
              "first": 0,
              "second": 1,
              "outcome": {
                "computed": {
                  "difference": -1.0,
                  "se": 0.75,
                  "statistic": 4.5,
                  "df": 9.5,
                  "p": {
                    "below": 1e-10
                  },
                  "ciLower": -2.5,
                  "ciUpper": 0.5
                }
              }
            },
            {
              "first": 0,
              "second": 1,
              "outcome": {
                "not_computable": "stats.anova.overflow"
              }
            }
          ]
        }
      }
    ],
    "exclusions": [],
    "notes": [],
    "limitations": [
      "stats.anova.normality_not_assessed"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.anova.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.one_way_anova",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 17,
    "rowsUsed": 13,
    "exclusions": [
      {
        "code": "stats.anova.other_level",
        "count": 4,
        "params": {
          "column": 2
        }
      }
    ],
    "platform": {
      "os": "macos",
      "arch": "aarch64"
    },
    "elapsedMs": 3,
    "outputHash": "5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a",
    "reproducibility": "bitwise"
  }
} satisfies AnovaResultView;
