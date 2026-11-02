// Typed copies of Cronbach's alpha's view fixtures (Research OS, Phase 8I-2,
// decision 381): each `satisfies` its view's type, so tsc catches a renamed or
// missing key or a word outside its closed set, and alphaTypes.vitest.ts
// deep-compares each with the JSON Rust serialises
// (src-tauri/tests/stats_alpha_wire.rs pins those bytes). Generated from the
// JSON once; edit both together.
import type { AlphaResultView, AlphaRunStateView, AlphaValidationView } from '../alphaTypes';

export const alphaValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.alpha.reversed_outside_scale",
      "params": {
        "column": 2,
        "count": 3
      },
      "severity": "error"
    },
    {
      "code": "stats.alpha.scale_missing",
      "params": {},
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.alpha.suspect_missing_code",
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
} satisfies AlphaValidationView;

export const alphaRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 4,
  "stepsTotal": 7,
  "diagnostics": [
    {
      "code": "stats.alpha.suspect_missing_code",
      "params": {
        "column": 3,
        "count": 2,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies AlphaRunStateView;

export const alphaResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "k": 3,
    "rowsUsed": 9,
    "alpha": {
      "value": 0.8125
    },
    "interval": {
      "computed": {
        "lower": 0.5,
        "upper": 0.9375
      }
    },
    "standardized": {
      "value": 0.8
    },
    "interItem": {
      "mean": {
        "value": 0.5
      },
      "min": {
        "value": 0.25
      },
      "max": {
        "value": 0.75
      }
    },
    "items": [
      {
        "item": "C1",
        "reversed": false,
        "mean": {
          "value": 3.5
        },
        "sd": {
          "value": 1.25
        },
        "scaleMeanIfDeleted": {
          "value": 7.5
        },
        "scaleVarianceIfDeleted": {
          "value": 4.75
        },
        "correctedItemTotal": {
          "value": 0.625
        },
        "alphaIfDeleted": {
          "value": 0.75
        }
      },
      {
        "item": "C2",
        "reversed": true,
        "mean": {
          "value": 2.25
        },
        "sd": {
          "value": 1.25
        },
        "scaleMeanIfDeleted": {
          "value": 7.5
        },
        "scaleVarianceIfDeleted": {
          "value": 4.75
        },
        "correctedItemTotal": {
          "value": 0.625
        },
        "alphaIfDeleted": {
          "value": 0.75
        }
      },
      {
        "item": "C4",
        "reversed": false,
        "mean": {
          "value": 4.0
        },
        "sd": {
          "value": 1.25
        },
        "scaleMeanIfDeleted": {
          "value": 7.5
        },
        "scaleVarianceIfDeleted": {
          "value": 4.75
        },
        "correctedItemTotal": {
          "value": 0.625
        },
        "alphaIfDeleted": {
          "value": 0.75
        }
      }
    ],
    "exclusions": [
      {
        "code": "stats.alpha.missing_value",
        "count": 2,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.alpha.missing_code",
        "count": 1,
        "params": {
          "column": 4
        }
      }
    ],
    "notes": [],
    "limitations": [
      "stats.alpha.assumes_one_dimension",
      "stats.alpha.no_omega"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.alpha.suspect_missing_code",
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
    "capabilityId": "stats.analyze.cronbach_alpha",
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
        "code": "stats.alpha.missing_value",
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
} satisfies AlphaResultView;

export const alphaResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "k": 2,
    "rowsUsed": 5,
    "alpha": {
      "value": -0.5
    },
    "interval": {
      "not_computable": "stats.alpha.distribution_out_of_range"
    },
    "standardized": {
      "not_computable": "stats.alpha.item_constant"
    },
    "interItem": {
      "mean": {
        "not_computable": "stats.alpha.item_constant"
      },
      "min": {
        "not_computable": "stats.alpha.item_constant"
      },
      "max": {
        "not_computable": "stats.alpha.item_constant"
      }
    },
    "items": [
      {
        "item": "C1",
        "reversed": false,
        "mean": {
          "value": 3.0
        },
        "sd": {
          "value": 0.0
        },
        "scaleMeanIfDeleted": {
          "value": 7.5
        },
        "scaleVarianceIfDeleted": {
          "value": 4.75
        },
        "correctedItemTotal": {
          "not_computable": "stats.alpha.item_constant"
        },
        "alphaIfDeleted": {
          "not_computable": "stats.alpha.two_items"
        }
      },
      {
        "item": "C2",
        "reversed": false,
        "mean": {
          "value": 2.5
        },
        "sd": {
          "value": 1.25
        },
        "scaleMeanIfDeleted": {
          "value": 7.5
        },
        "scaleVarianceIfDeleted": {
          "value": 4.75
        },
        "correctedItemTotal": {
          "value": 0.625
        },
        "alphaIfDeleted": {
          "not_computable": "stats.alpha.two_items"
        }
      }
    ],
    "exclusions": [],
    "notes": [
      "stats.alpha.negative_alpha"
    ],
    "limitations": [
      "stats.alpha.assumes_one_dimension",
      "stats.alpha.no_omega"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.alpha.suspect_missing_code",
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
    "capabilityId": "stats.analyze.cronbach_alpha",
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
        "code": "stats.alpha.missing_value",
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
} satisfies AlphaResultView;
