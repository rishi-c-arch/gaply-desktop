// Typed copies of the factor analysis's view fixtures (Research OS, Phase 9B-3, decision
// 729): each `satisfies` its view's type, so tsc catches a renamed or missing key or a word
// outside its closed set, and factorTypes.vitest.ts deep-compares each with the JSON Rust
// serialises (src-tauri/tests/stats_factor_wire.rs pins those bytes). Generated from the JSON
// once; edit both together.
//
// The Other result's manifest is the first result's, as the wire test writes it, so no test
// treats either as a stored result.
import type { FactorResultView, FactorRunStateView, FactorValidationView } from '../factorTypes';

export const factorValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.factor.item_repeated",
      "params": {
        "column": 1
      },
      "severity": "error"
    },
    {
      "code": "stats.factor.too_few_rows",
      "params": {
        "count": 2,
        "limit": 3
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.factor.suspect_missing_code",
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
      "code": "stats.factor.no_complete_rows",
      "params": {},
      "severity": "error"
    }
  ],
  "estimatedResources": {
    "rows": 41,
    "columns": 6,
    "cells": 246
  }
} satisfies FactorValidationView;

export const factorRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 22,
  "stepsTotal": 24,
  "diagnostics": [
    {
      "code": "stats.factor.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies FactorRunStateView;

export const factorResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "items": [
      "C1",
      "C2",
      "C3",
      "C4",
      "C5",
      "C6"
    ],
    "rowsUsed": 40,
    "descriptives": [
      {
        "item": "C1",
        "mean": {
          "value": 38.424999999999955
        },
        "sd": {
          "value": 651.5430555267457
        }
      },
      {
        "item": "C2",
        "mean": {
          "value": 29.475000000000023
        },
        "sd": {
          "value": 681.782279237332
        }
      },
      {
        "item": "C3",
        "mean": {
          "value": 23.949999999999964
        },
        "sd": {
          "value": 649.5162677341361
        }
      },
      {
        "item": "C4",
        "mean": {
          "value": 64.85000000000002
        },
        "sd": {
          "value": 648.1152378539341
        }
      },
      {
        "item": "C5",
        "mean": {
          "value": 34.4
        },
        "sd": {
          "value": 645.2448869247737
        }
      },
      {
        "item": "C6",
        "mean": {
          "value": 50.375
        },
        "sd": {
          "value": 574.3786246781643
        }
      }
    ],
    "determinant": {
      "value": {
        "value": 0.005652229349015525
      },
      "log": {
        "value": -5.175705236601054
      }
    },
    "kmo": {
      "overall": {
        "value": 0.7206346004950582
      },
      "items": [
        {
          "item": "C1",
          "msa": {
            "value": 0.7170694260054213
          }
        },
        {
          "item": "C2",
          "msa": {
            "value": 0.7967188799955066
          }
        },
        {
          "item": "C3",
          "msa": {
            "value": 0.7924554559229509
          }
        },
        {
          "item": "C4",
          "msa": {
            "value": 0.7498212219774575
          }
        },
        {
          "item": "C5",
          "msa": {
            "value": 0.5954395999245529
          }
        },
        {
          "item": "C6",
          "msa": {
            "value": 0.6889048070401917
          }
        }
      ]
    },
    "bartlett": {
      "chiSquare": {
        "value": 187.18800605707145
      },
      "df": 15,
      "p": {
        "value": 8.404597011389804e-32
      }
    },
    "eigenvalues": [
      {
        "total": {
          "value": 3.279995914839406
        },
        "percent": {
          "value": 54.66659858065677
        },
        "cumulative": {
          "value": 54.66659858065677
        }
      },
      {
        "total": {
          "value": 1.9879644958741403
        },
        "percent": {
          "value": 33.13274159790234
        },
        "cumulative": {
          "value": 87.7993401785591
        }
      },
      {
        "total": {
          "value": 0.27520708768112107
        },
        "percent": {
          "value": 4.586784794685351
        },
        "cumulative": {
          "value": 92.38612497324446
        }
      },
      {
        "total": {
          "value": 0.1928079099831362
        },
        "percent": {
          "value": 3.2134651663856033
        },
        "cumulative": {
          "value": 95.59959013963007
        }
      },
      {
        "total": {
          "value": 0.1650419491770402
        },
        "percent": {
          "value": 2.75069915295067
        },
        "cumulative": {
          "value": 98.35028929258074
        }
      },
      {
        "total": {
          "value": 0.0989826424451557
        },
        "percent": {
          "value": 1.6497107074192618
        },
        "cumulative": {
          "value": 100.0
        }
      }
    ],
    "extraction": {
      "method": "principal_axis",
      "factors": 2,
      "iterations": 11,
      "communalities": [
        {
          "item": "C1",
          "initial": {
            "value": 0.754345652411321
          },
          "extraction": {
            "value": 0.7587732473960601
          }
        },
        {
          "item": "C2",
          "initial": {
            "value": 0.7375677482868201
          },
          "extraction": {
            "value": 0.8134600595179463
          }
        },
        {
          "item": "C3",
          "initial": {
            "value": 0.7601278893158581
          },
          "extraction": {
            "value": 0.8083468530250082
          }
        },
        {
          "item": "C4",
          "initial": {
            "value": 0.7461257425398923
          },
          "extraction": {
            "value": 0.7996830666383519
          }
        },
        {
          "item": "C5",
          "initial": {
            "value": 0.8082028905023271
          },
          "extraction": {
            "value": 0.9138332076312995
          }
        },
        {
          "item": "C6",
          "initial": {
            "value": 0.7819893790668069
          },
          "extraction": {
            "value": 0.8215618682339232
          }
        }
      ],
      "sums": [
        {
          "total": {
            "value": 3.0926849381622032
          },
          "percent": {
            "value": 51.544748969370055
          },
          "cumulative": {
            "value": 51.544748969370055
          }
        },
        {
          "total": {
            "value": 1.8229733642803858
          },
          "percent": {
            "value": 30.382889404673097
          },
          "cumulative": {
            "value": 81.92763837404316
          }
        }
      ],
      "loadings": [
        [
          {
            "value": 0.7235274313576412
          },
          {
            "value": 0.48505804134049146
          }
        ],
        [
          {
            "value": 0.7223957156521852
          },
          {
            "value": -0.5400041569518826
          }
        ],
        [
          {
            "value": 0.7761409487487749
          },
          {
            "value": 0.45381943623026966
          }
        ],
        [
          {
            "value": 0.734622320948299
          },
          {
            "value": -0.5099148087699416
          }
        ],
        [
          {
            "value": 0.5805074599092479
          },
          {
            "value": 0.7595026639986278
          }
        ],
        [
          {
            "value": 0.7538459965219574
          },
          {
            "value": -0.5032674058209414
          }
        ]
      ]
    },
    "rotation": {
      "method": "promax",
      "iterations": 2,
      "loadings": [
        [
          {
            "value": 0.09942098893804374
          },
          {
            "value": 0.8406616328801733
          }
        ],
        [
          {
            "value": 0.908033874944439
          },
          {
            "value": -0.025576273235629735
          }
        ],
        [
          {
            "value": 0.15916644216448167
          },
          {
            "value": 0.8456308876359264
          }
        ],
        [
          {
            "value": 0.8924280614109545
          },
          {
            "value": 0.007115824035374946
          }
        ],
        [
          {
            "value": -0.21263338365278894
          },
          {
            "value": 0.9871915803433563
          }
        ],
        [
          {
            "value": 0.8999968156118977
          },
          {
            "value": 0.02418237625724838
          }
        ]
      ],
      "transformation": null,
      "sums": null,
      "structure": [
        [
          {
            "value": 0.3115328066155252
          },
          {
            "value": 0.8657470725554571
          }
        ],
        [
          {
            "value": 0.9015805891118993
          },
          {
            "value": 0.20353459395990625
          }
        ],
        [
          {
            "value": 0.37253207899763696
          },
          {
            "value": 0.8857910210835512
          }
        ],
        [
          {
            "value": 0.8942234929037007
          },
          {
            "value": 0.2322891052326273
          }
        ],
        [
          {
            "value": 0.036450186136319784
          },
          {
            "value": 0.933540917887187
          }
        ],
        [
          {
            "value": 0.9060983998677559
          },
          {
            "value": 0.25126537017163264
          }
        ]
      ],
      "factorCorrelations": [
        [
          {
            "value": 1.0
          },
          {
            "value": 0.25231533042702275
          }
        ],
        [
          {
            "value": 0.25231533042702275
          },
          {
            "value": 1.0
          }
        ]
      ]
    },
    "exclusions": [
      {
        "code": "stats.factor.missing_value",
        "count": 1,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [],
    "limitations": [
      "stats.factor.no_ml_or_other_extraction",
      "stats.factor.no_oblique_other_than_promax",
      "stats.factor.promax_kappa_four",
      "stats.factor.no_factor_scores",
      "stats.factor.no_matrices_shown",
      "stats.factor.pearson_only",
      "stats.factor.listwise_only",
      "stats.factor.no_scree_plot",
      "stats.factor.no_weights"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.factor.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.factor.no_ml_or_other_extraction",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.factor",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 41,
    "rowsUsed": 40,
    "exclusions": [
      {
        "code": "stats.factor.missing_value",
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
} satisfies FactorResultView;

export const factorResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "items": [
      "C1",
      "C2",
      "C3",
      "C4"
    ],
    "rowsUsed": 8,
    "descriptives": [
      {
        "item": "C1",
        "mean": {
          "value": 0.0
        },
        "sd": {
          "value": 1.0690449676496976
        }
      },
      {
        "item": "C2",
        "mean": {
          "value": 0.0
        },
        "sd": {
          "value": 1.0690449676496976
        }
      },
      {
        "item": "C3",
        "mean": {
          "value": 0.0
        },
        "sd": {
          "value": 1.0690449676496976
        }
      },
      {
        "item": "C4",
        "mean": {
          "value": 0.0
        },
        "sd": {
          "value": 1.8516401995451028
        }
      }
    ],
    "determinant": {
      "value": {
        "not_computable": "stats.factor.correlations_singular"
      },
      "log": {
        "not_computable": "stats.factor.correlations_singular"
      }
    },
    "kmo": {
      "overall": {
        "not_computable": "stats.factor.correlations_singular"
      },
      "items": [
        {
          "item": "C1",
          "msa": {
            "not_computable": "stats.factor.correlations_singular"
          }
        },
        {
          "item": "C2",
          "msa": {
            "not_computable": "stats.factor.correlations_singular"
          }
        },
        {
          "item": "C3",
          "msa": {
            "not_computable": "stats.factor.correlations_singular"
          }
        },
        {
          "item": "C4",
          "msa": {
            "not_computable": "stats.factor.correlations_singular"
          }
        }
      ]
    },
    "bartlett": {
      "chiSquare": {
        "not_computable": "stats.factor.correlations_singular"
      },
      "df": 6,
      "p": {
        "not_computable": "stats.factor.correlations_singular"
      }
    },
    "eigenvalues": [
      {
        "total": {
          "value": 2.0
        },
        "percent": {
          "value": 50.0
        },
        "cumulative": {
          "value": 50.0
        }
      },
      {
        "total": {
          "value": 0.9999999999999999
        },
        "percent": {
          "value": 24.999999999999996
        },
        "cumulative": {
          "value": 75.0
        }
      },
      {
        "total": {
          "value": 0.9999999999999998
        },
        "percent": {
          "value": 24.999999999999993
        },
        "cumulative": {
          "value": 100.0
        }
      },
      {
        "total": {
          "value": -3.8622341641530597e-17
        },
        "percent": {
          "value": -9.65558541038265e-16
        },
        "cumulative": {
          "value": 100.0
        }
      }
    ],
    "extraction": {
      "method": "principal_components",
      "factors": 2,
      "iterations": 0,
      "communalities": [
        {
          "item": "C1",
          "initial": {
            "value": 1.0
          },
          "extraction": {
            "value": 0.409885723228967
          }
        },
        {
          "item": "C2",
          "initial": {
            "value": 1.0
          },
          "extraction": {
            "value": 0.9791250433792091
          }
        },
        {
          "item": "C3",
          "initial": {
            "value": 1.0
          },
          "extraction": {
            "value": 0.610989233391824
          }
        },
        {
          "item": "C4",
          "initial": {
            "value": 1.0
          },
          "extraction": {
            "value": 1.0
          }
        }
      ],
      "sums": [
        {
          "total": {
            "value": 2.0
          },
          "percent": {
            "value": 50.0
          },
          "cumulative": {
            "value": 50.0
          }
        },
        {
          "total": {
            "value": 1.0
          },
          "percent": {
            "value": 25.0
          },
          "cumulative": {
            "value": 75.0
          }
        }
      ],
      "loadings": [
        [
          {
            "value": 0.5773502691896257
          },
          {
            "value": 0.27668102554319424
          }
        ],
        [
          {
            "value": 0.5773502691896258
          },
          {
            "value": -0.803611666195729
          }
        ],
        [
          {
            "value": 0.5773502691896258
          },
          {
            "value": 0.5269306406525346
          }
        ],
        [
          {
            "value": 1.0
          },
          {
            "value": 2.2352242212375036e-17
          }
        ]
      ]
    },
    "rotation": {
      "method": "varimax",
      "iterations": 2,
      "loadings": [
        [
          {
            "value": 0.6325494213393792
          },
          {
            "value": 0.09882789480801238
          }
        ],
        [
          {
            "value": 0.020899723436166875
          },
          {
            "value": 0.9892867354511032
          }
        ],
        [
          {
            "value": 0.7742379680198513
          },
          {
            "value": -0.10744674154350004
          }
        ],
        [
          {
            "value": 0.8242755388909823
          },
          {
            "value": 0.5661888695355825
          }
        ]
      ],
      "transformation": [
        [
          {
            "value": 0.8242755388909823
          },
          {
            "value": 0.5661888695355825
          }
        ],
        [
          {
            "value": 0.5661888695355825
          },
          {
            "value": -0.8242755388909823
          }
        ]
      ],
      "sums": [
        {
          "total": {
            "value": 1.6794301640140192
          },
          "percent": {
            "value": 41.98575410035048
          },
          "cumulative": {
            "value": 41.98575410035048
          }
        },
        {
          "total": {
            "value": 1.3205698359859812
          },
          "percent": {
            "value": 33.01424589964953
          },
          "cumulative": {
            "value": 75.00000000000001
          }
        }
      ],
      "structure": null,
      "factorCorrelations": null
    },
    "exclusions": [
      {
        "code": "stats.factor.missing_code",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.factor.missing_value",
        "count": 1,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [
      "stats.factor.eigenvalues_not_distinct"
    ],
    "limitations": [
      "stats.factor.no_ml_or_other_extraction",
      "stats.factor.no_oblique_other_than_promax",
      "stats.factor.promax_kappa_four",
      "stats.factor.no_factor_scores",
      "stats.factor.no_matrices_shown",
      "stats.factor.pearson_only",
      "stats.factor.listwise_only",
      "stats.factor.no_scree_plot",
      "stats.factor.no_weights"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.factor.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.factor.no_ml_or_other_extraction",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.factor",
    "capabilityVersion": 1,
    "engineVersion": 1,
    "inputs": [
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ],
    "parametersHash": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "seed": 7,
    "rowsRead": 41,
    "rowsUsed": 40,
    "exclusions": [
      {
        "code": "stats.factor.missing_value",
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
} satisfies FactorResultView;
