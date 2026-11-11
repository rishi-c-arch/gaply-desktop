// Typed copies of the linear regression's view fixtures (Research OS, Phase 9A-2, decision
// 684): each `satisfies` its view's type, so tsc catches a renamed or missing key or a word
// outside its closed set, and linearRegressionTypes.vitest.ts deep-compares each with the
// JSON Rust serialises (src-tauri/tests/stats_linear_regression_wire.rs pins those bytes).
// Generated from the JSON once; edit both together.
//
// The Other result's manifest is the first result's, as the wire test writes it, so no test
// treats either as a stored result.
import type { LinearRegressionResultView, LinearRegressionRunStateView, LinearRegressionValidationView } from '../linearRegressionTypes';

export const linearRegressionValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.linear_regression.column_repeated",
      "params": {
        "column": 1
      },
      "severity": "error"
    },
    {
      "code": "stats.linear_regression.too_few_rows",
      "params": {
        "count": 3,
        "limit": 4
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.linear_regression.suspect_missing_code",
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
      "code": "stats.linear_regression.no_complete_rows",
      "params": {},
      "severity": "error"
    }
  ],
  "estimatedResources": {
    "rows": 21,
    "columns": 4,
    "cells": 84
  }
} satisfies LinearRegressionValidationView;

export const linearRegressionRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 0,
  "stepsTotal": 1,
  "diagnostics": [
    {
      "code": "stats.linear_regression.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies LinearRegressionRunStateView;

export const linearRegressionResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "outcome": "C1",
    "rowsUsed": 10,
    "columns": [
      {
        "column": "C1",
        "mean": {
          "value": 13.08
        },
        "sd": {
          "value": 4.032038359605887
        }
      },
      {
        "column": "C2",
        "mean": {
          "value": 5.83
        },
        "sd": {
          "value": 3.0265675461008814
        }
      },
      {
        "column": "C3",
        "mean": {
          "value": 0.15000000000000005
        },
        "sd": {
          "value": 1.259409032487504
        }
      },
      {
        "column": "C4",
        "mean": {
          "value": 12.4
        },
        "sd": {
          "value": 2.1959558789334137
        }
      }
    ],
    "model": {
      "r": {
        "value": 0.9855735239941341
      },
      "rSquared": {
        "value": 0.971355171198216
      },
      "adjustedRSquared": {
        "value": 0.957032756797324
      },
      "standardErrorOfEstimate": {
        "value": 0.8357827439952954
      },
      "durbinWatson": {
        "value": 2.1435729292759156
      }
    },
    "anova": {
      "regression": {
        "sumOfSquares": {
          "value": 142.12480322903815
        },
        "df": 3,
        "meanSquare": {
          "value": 47.37493440967938
        }
      },
      "residual": {
        "sumOfSquares": {
          "value": 4.191196770961833
        },
        "df": 6,
        "meanSquare": {
          "value": 0.6985327951603054
        }
      },
      "total": {
        "sumOfSquares": {
          "value": 146.31599999999997
        },
        "df": 9
      },
      "f": {
        "value": 67.82063023799387
      },
      "p": {
        "value": 0.00005085914606051898
      }
    },
    "intercept": {
      "b": {
        "value": 3.277388498378592
      },
      "standardError": {
        "value": 1.6882386848668438
      },
      "t": {
        "value": 1.9413063613318924
      },
      "p": {
        "value": 0.10026014519370459
      },
      "lower": {
        "value": -0.8535827471834807
      },
      "upper": {
        "value": 7.4083597439406645
      }
    },
    "predictors": [
      {
        "column": "C2",
        "b": {
          "value": 1.216491277270161
        },
        "standardError": {
          "value": 0.1125802822904032
        },
        "beta": {
          "value": 0.9131344227242313
        },
        "t": {
          "value": 10.805544741238046
        },
        "p": {
          "value": 0.00003717580793385196
        },
        "lower": {
          "value": 0.9410172503285277
        },
        "upper": {
          "value": 1.4919653042117944
        },
        "zeroOrder": {
          "value": 0.9807598582184702
        },
        "partial": {
          "value": 0.9752558966298728
        },
        "part": {
          "value": 0.7466106080629829
        },
        "tolerance": {
          "value": 0.6685268935452667
        },
        "vif": {
          "value": 1.4958261360240832
        }
      },
      {
        "column": "C3",
        "b": {
          "value": 0.008379051366205405
        },
        "standardError": {
          "value": 0.226679832225169
        },
        "beta": {
          "value": 0.002617200540549252
        },
        "t": {
          "value": 0.03696425607851254
        },
        "p": {
          "value": 0.971712650669277
        },
        "lower": {
          "value": -0.5462865165211133
        },
        "upper": {
          "value": 0.5630446192535241
        },
        "zeroOrder": {
          "value": -0.18533158585387063
        },
        "partial": {
          "value": 0.015088876385104281
        },
        "part": {
          "value": 0.002554050385081467
        },
        "tolerance": {
          "value": 0.9523244147440345
        },
        "vif": {
          "value": 1.050062336445275
        }
      },
      {
        "column": "C4",
        "b": {
          "value": 0.21848471753479348
        },
        "standardError": {
          "value": 0.15596270122796382
        },
        "beta": {
          "value": 0.11899261791113827
        },
        "t": {
          "value": 1.400878003616031
        },
        "p": {
          "value": 0.21079164855189259
        },
        "lower": {
          "value": -0.16314226443649282
        },
        "upper": {
          "value": 0.6001116995060798
        },
        "zeroOrder": {
          "value": 0.641003076480192
        },
        "partial": {
          "value": 0.496451362136176
        },
        "part": {
          "value": 0.09679385937020211
        },
        "tolerance": {
          "value": 0.6616915278685104
        },
        "vif": {
          "value": 1.5112782284235584
        }
      }
    ],
    "residuals": {
      "predicted": {
        "minimum": {
          "value": 6.926214732133824
        },
        "maximum": {
          "value": 18.410374207194497
        },
        "mean": {
          "value": 13.08
        },
        "standardDeviation": {
          "value": 3.9738702549563016
        }
      },
      "residual": {
        "minimum": {
          "value": -1.3271521412237788
        },
        "maximum": {
          "value": 1.0896257928055013
        },
        "mean": {
          "value": -6.66133814775094e-17
        },
        "standardDeviation": {
          "value": 0.6824137528705516
        }
      },
      "standardizedPredicted": {
        "minimum": {
          "value": -1.5485622008396058
        },
        "maximum": {
          "value": 1.341355873545779
        }
      },
      "standardizedResidual": {
        "minimum": {
          "value": -1.587915221699348
        },
        "maximum": {
          "value": 1.3037189396813327
        }
      }
    },
    "exclusions": [
      {
        "code": "stats.linear_regression.missing_value",
        "count": 1,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [],
    "limitations": [
      "stats.linear_regression.enter_only",
      "stats.linear_regression.no_categorical_predictors",
      "stats.linear_regression.no_interaction_terms",
      "stats.linear_regression.intercept_always",
      "stats.linear_regression.listwise_only",
      "stats.linear_regression.no_robust_standard_errors",
      "stats.linear_regression.no_casewise_diagnostics",
      "stats.linear_regression.durbin_watson_row_order",
      "stats.linear_regression.intervals_95_only",
      "stats.linear_regression.no_weights"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.linear_regression.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.linear_regression.enter_only",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.linear_regression",
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
        "code": "stats.linear_regression.missing_value",
        "count": 1,
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
    "outputHash": "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b",
    "reproducibility": "bitwise"
  }
} satisfies LinearRegressionResultView;

export const linearRegressionResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "outcome": "C2",
    "rowsUsed": 3,
    "columns": [
      {
        "column": "C2",
        "mean": {
          "value": 9
        },
        "sd": {
          "value": 6.244997998398398
        }
      },
      {
        "column": "C1",
        "mean": {
          "value": 2.3333333333333335
        },
        "sd": {
          "value": 2.081665999466133
        }
      }
    ],
    "model": {
      "r": {
        "value": 1
      },
      "rSquared": {
        "value": 1
      },
      "adjustedRSquared": {
        "value": 1
      },
      "standardErrorOfEstimate": {
        "not_computable": "stats.linear_regression.perfect_fit"
      },
      "durbinWatson": {
        "not_computable": "stats.linear_regression.perfect_fit"
      }
    },
    "anova": {
      "regression": {
        "sumOfSquares": {
          "value": 78
        },
        "df": 1,
        "meanSquare": {
          "value": 78
        }
      },
      "residual": {
        "sumOfSquares": {
          "value": 0
        },
        "df": 1,
        "meanSquare": {
          "not_computable": "stats.linear_regression.perfect_fit"
        }
      },
      "total": {
        "sumOfSquares": {
          "value": 78
        },
        "df": 2
      },
      "f": {
        "not_computable": "stats.linear_regression.perfect_fit"
      },
      "p": {
        "not_computable": "stats.linear_regression.perfect_fit"
      }
    },
    "intercept": {
      "b": {
        "value": 2
      },
      "standardError": {
        "not_computable": "stats.linear_regression.perfect_fit"
      },
      "t": {
        "not_computable": "stats.linear_regression.perfect_fit"
      },
      "p": {
        "not_computable": "stats.linear_regression.perfect_fit"
      },
      "lower": {
        "not_computable": "stats.linear_regression.perfect_fit"
      },
      "upper": {
        "not_computable": "stats.linear_regression.perfect_fit"
      }
    },
    "predictors": [
      {
        "column": "C1",
        "b": {
          "value": 3.000000000000001
        },
        "standardError": {
          "not_computable": "stats.linear_regression.perfect_fit"
        },
        "beta": {
          "value": 1.0000000000000004
        },
        "t": {
          "not_computable": "stats.linear_regression.perfect_fit"
        },
        "p": {
          "not_computable": "stats.linear_regression.perfect_fit"
        },
        "lower": {
          "not_computable": "stats.linear_regression.perfect_fit"
        },
        "upper": {
          "not_computable": "stats.linear_regression.perfect_fit"
        },
        "zeroOrder": {
          "value": 0.9999999999999999
        },
        "partial": {
          "not_computable": "stats.linear_regression.perfect_fit"
        },
        "part": {
          "not_computable": "stats.linear_regression.perfect_fit"
        },
        "tolerance": {
          "value": 1
        },
        "vif": {
          "value": 1
        }
      }
    ],
    "residuals": {
      "predicted": {
        "minimum": {
          "value": 2
        },
        "maximum": {
          "value": 14
        },
        "mean": {
          "value": 9
        },
        "standardDeviation": {
          "value": 6.244997998398398
        }
      },
      "residual": {
        "minimum": {
          "value": 0
        },
        "maximum": {
          "value": 0
        },
        "mean": {
          "value": 0
        },
        "standardDeviation": {
          "value": 0
        }
      },
      "standardizedPredicted": {
        "minimum": {
          "value": -1.12089707663561
        },
        "maximum": {
          "value": 0.8006407690254357
        }
      },
      "standardizedResidual": {
        "minimum": {
          "not_computable": "stats.linear_regression.perfect_fit"
        },
        "maximum": {
          "not_computable": "stats.linear_regression.perfect_fit"
        }
      }
    },
    "exclusions": [
      {
        "code": "stats.linear_regression.missing_code",
        "count": 2,
        "params": {
          "column": 1
        }
      }
    ],
    "notes": [
      "stats.linear_regression.essentially_perfect_fit"
    ],
    "limitations": [
      "stats.linear_regression.enter_only",
      "stats.linear_regression.no_categorical_predictors",
      "stats.linear_regression.no_interaction_terms",
      "stats.linear_regression.intercept_always",
      "stats.linear_regression.listwise_only",
      "stats.linear_regression.no_robust_standard_errors",
      "stats.linear_regression.no_casewise_diagnostics",
      "stats.linear_regression.durbin_watson_row_order",
      "stats.linear_regression.intervals_95_only",
      "stats.linear_regression.no_weights"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.linear_regression.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.linear_regression.enter_only",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.linear_regression",
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
        "code": "stats.linear_regression.missing_value",
        "count": 1,
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
    "outputHash": "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b",
    "reproducibility": "bitwise"
  }
} satisfies LinearRegressionResultView;
