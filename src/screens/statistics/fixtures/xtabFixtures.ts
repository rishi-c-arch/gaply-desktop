// Typed copies of crosstabs' view fixtures (Research OS, Phase 8X-2, decision
// 588): each `satisfies` its view's type, so tsc catches a renamed or missing key
// or a word outside its closed set, and xtabTypes.vitest.ts deep-compares each with
// the JSON Rust serialises (src-tauri/tests/stats_xtab_wire.rs pins those bytes).
// Generated from the JSON once; edit both together.
import type { XtabResultView, XtabRunStateView, XtabValidationView } from '../xtabTypes';

export const xtabValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.xtab.too_many_levels",
      "params": {
        "column": 1,
        "count": 51,
        "limit": 50
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.xtab.suspect_missing_code",
      "params": {
        "column": 2,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ],
  "missingInformation": [
    {
      "code": "stats.xtab.same_column",
      "params": {},
      "severity": "error"
    }
  ],
  "estimatedResources": {
    "rows": 22,
    "columns": 2,
    "cells": 44
  }
} satisfies XtabValidationView;

export const xtabRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 0,
  "stepsTotal": 1,
  "diagnostics": [
    {
      "code": "stats.xtab.suspect_missing_code",
      "params": {
        "column": 2,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies XtabRunStateView;

export const xtabResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "rowLevels": [
      {
        "level": 0,
        "label": "no"
      },
      {
        "level": 1,
        "label": "yes"
      }
    ],
    "columnLevels": [
      {
        "level": 0,
        "label": "a"
      },
      {
        "level": 1,
        "label": "b"
      }
    ],
    "frequencies": [
      {
        "row": 0,
        "column": 0,
        "observed": 5,
        "expected": 4.5,
        "rowProportion": 0.625,
        "columnProportion": 0.5,
        "totalProportion": 0.25,
        "residual": 0.5,
        "pearsonResidual": 0.25,
        "adjustedResidual": -1.5
      },
      {
        "row": 0,
        "column": 1,
        "observed": 3,
        "expected": 3.5,
        "rowProportion": 0.625,
        "columnProportion": 0.5,
        "totalProportion": 0.25,
        "residual": -0.5,
        "pearsonResidual": 0.25,
        "adjustedResidual": -1.5
      },
      {
        "row": 1,
        "column": 0,
        "observed": 4,
        "expected": 4.5,
        "rowProportion": 0.625,
        "columnProportion": 0.5,
        "totalProportion": 0.25,
        "residual": -0.5,
        "pearsonResidual": 0.25,
        "adjustedResidual": -1.5
      },
      {
        "row": 1,
        "column": 1,
        "observed": 4,
        "expected": 3.5,
        "rowProportion": 0.625,
        "columnProportion": 0.5,
        "totalProportion": 0.25,
        "residual": 0.5,
        "pearsonResidual": 0.25,
        "adjustedResidual": -1.5
      }
    ],
    "rowTotals": [
      8,
      8
    ],
    "columnTotals": [
      9,
      7
    ],
    "n": 16,
    "pearson": {
      "computed": {
        "statistic": 0.254,
        "df": 1.0,
        "p": 0.614
      }
    },
    "likelihoodRatio": {
      "computed": {
        "statistic": 0.255,
        "df": 1.0,
        "p": 0.613
      }
    },
    "continuityCorrected": {
      "computed": {
        "statistic": 0.0,
        "df": 1.0,
        "p": 1.0
      }
    },
    "fisher": {
      "twoSided": 1.0,
      "less": 0.807,
      "greater": 0.5
    },
    "phi": {
      "value": 0.126
    },
    "cramersV": {
      "value": 0.126
    },
    "contingencyCoefficient": {
      "value": 0.125
    },
    "expectedBelowFive": 4,
    "cellCount": 4,
    "smallestExpected": 3.5,
    "exclusions": [
      {
        "code": "stats.xtab.missing_row",
        "count": 2,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.xtab.missing_code",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.xtab.missing_column",
        "count": 1,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.xtab.missing_code",
        "count": 3,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [
      "stats.xtab.expected_below_five"
    ],
    "limitations": [
      "stats.xtab.no_layers",
      "stats.xtab.fisher_two_by_two_only",
      "stats.xtab.no_further_nominal_measures",
      "stats.xtab.no_weights",
      "stats.xtab.code_point_order"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.xtab.suspect_missing_code",
      "params": {
        "column": 2,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.xtab.no_layers",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.crosstabs",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 22,
    "rowsUsed": 16,
    "exclusions": [
      {
        "code": "stats.xtab.missing_row",
        "count": 2,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.xtab.missing_code",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.xtab.missing_column",
        "count": 1,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.xtab.missing_code",
        "count": 3,
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
} satisfies XtabResultView;

export const xtabResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "rowLevels": [
      {
        "level": 0,
        "label": "a"
      },
      {
        "level": 1,
        "label": "b"
      },
      {
        "level": 2,
        "label": "c"
      }
    ],
    "columnLevels": [
      {
        "level": 0,
        "label": "a"
      },
      {
        "level": 1,
        "label": "b"
      }
    ],
    "frequencies": [
      {
        "row": 0,
        "column": 0,
        "observed": 5,
        "expected": 4.5,
        "rowProportion": 0.625,
        "columnProportion": 0.5,
        "totalProportion": 0.25,
        "residual": 0.5,
        "pearsonResidual": 0.25,
        "adjustedResidual": -1.5
      },
      {
        "row": 0,
        "column": 1,
        "observed": 3,
        "expected": 3.5,
        "rowProportion": 0.625,
        "columnProportion": 0.5,
        "totalProportion": 0.25,
        "residual": -0.5,
        "pearsonResidual": 0.25,
        "adjustedResidual": -1.5
      },
      {
        "row": 1,
        "column": 0,
        "observed": 4,
        "expected": 4.5,
        "rowProportion": 0.625,
        "columnProportion": 0.5,
        "totalProportion": 0.25,
        "residual": -0.5,
        "pearsonResidual": 0.25,
        "adjustedResidual": -1.5
      },
      {
        "row": 1,
        "column": 1,
        "observed": 4,
        "expected": 3.5,
        "rowProportion": 0.625,
        "columnProportion": 0.5,
        "totalProportion": 0.25,
        "residual": 0.5,
        "pearsonResidual": 0.25,
        "adjustedResidual": -1.5
      }
    ],
    "rowTotals": [
      8,
      8
    ],
    "columnTotals": [
      9,
      7
    ],
    "n": 16,
    "pearson": {
      "not_computable": "stats.xtab.arithmetic_refused"
    },
    "likelihoodRatio": {
      "computed": {
        "statistic": 0.255,
        "df": 1.0,
        "p": 0.613
      }
    },
    "continuityCorrected": null,
    "fisher": null,
    "phi": {
      "not_computable": "stats.xtab.arithmetic_refused"
    },
    "cramersV": {
      "not_computable": "stats.xtab.arithmetic_refused"
    },
    "contingencyCoefficient": {
      "not_computable": "stats.xtab.arithmetic_refused"
    },
    "expectedBelowFive": 0,
    "cellCount": 4,
    "smallestExpected": 3.5,
    "exclusions": [],
    "notes": [],
    "limitations": [
      "stats.xtab.no_layers",
      "stats.xtab.fisher_two_by_two_only",
      "stats.xtab.no_further_nominal_measures",
      "stats.xtab.no_weights",
      "stats.xtab.code_point_order"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.xtab.suspect_missing_code",
      "params": {
        "column": 2,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.xtab.no_layers",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.crosstabs",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 22,
    "rowsUsed": 16,
    "exclusions": [
      {
        "code": "stats.xtab.missing_row",
        "count": 2,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.xtab.missing_code",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.xtab.missing_column",
        "count": 1,
        "params": {
          "column": 2
        }
      },
      {
        "code": "stats.xtab.missing_code",
        "count": 3,
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
} satisfies XtabResultView;
