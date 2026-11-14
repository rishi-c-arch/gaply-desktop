// Typed copies of the confirmatory factor analysis's view fixtures (Research OS, Phase 9C-3,
// decision 782): each `satisfies` its view's type, so tsc catches a renamed or missing key or a
// word outside its closed set, and cfaTypes.vitest.ts deep-compares each with the JSON Rust
// serialises (src-tauri/tests/stats_cfa_wire.rs pins those bytes). Generated from the JSON
// once; edit both together. The limits are read from cfaLimits.json itself, never copied.
//
// The Other result's manifest is the first result's, as the wire test writes it, so no test
// treats either as a stored result.
import type { CfaResultView, CfaRunStateView, CfaValidationView } from '../cfaTypes';
import CFA_LIMITS_JSON from './cfaLimits.json';

/** stats_cfa's four limits (MAX_ITEMS, MAX_FACTORS, MIN_FACTOR_ITEMS, MIN_ITEMS), as
 *  stats_cfa_wire.rs writes them. */
export interface CfaLimits {
  maxItems: number;
  maxFactors: number;
  minFactorItems: number;
  minItems: number;
}
export const cfaLimits: CfaLimits = CFA_LIMITS_JSON satisfies CfaLimits;

export const cfaValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.cfa.item_repeated",
      "params": {
        "column": 1
      },
      "severity": "error"
    },
    {
      "code": "stats.cfa.too_few_rows",
      "params": {
        "count": 2,
        "limit": 4
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.cfa.suspect_missing_code",
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
      "code": "stats.cfa.no_complete_rows",
      "params": {},
      "severity": "error"
    }
  ],
  "estimatedResources": {
    "rows": 41,
    "columns": 6,
    "cells": 246
  }
} satisfies CfaValidationView;

export const cfaRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 7,
  "stepsTotal": 9,
  "diagnostics": [
    {
      "code": "stats.cfa.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies CfaRunStateView;

export const cfaResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "items": [
      "C1",
      "C3",
      "C5",
      "C2",
      "C4",
      "C6"
    ],
    "rowsUsed": 40,
    "indicators": [
      [
        "C1",
        "C3",
        "C5"
      ],
      [
        "C2",
        "C4",
        "C6"
      ]
    ],
    "loadings": [
      {
        "factor": 1,
        "item": "C1",
        "estimate": {
          "value": 1
        },
        "se": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "cr": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "p": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "standardized": {
          "value": 0.9677217686405407
        }
      },
      {
        "factor": 1,
        "item": "C3",
        "estimate": {
          "value": 0.7763556742260725
        },
        "se": {
          "value": 0.05819696513977522
        },
        "cr": {
          "value": 13.34014020080689
        },
        "p": {
          "value": 1.3520501940791632e-40
        },
        "standardized": {
          "value": 0.9335013454153324
        }
      },
      {
        "factor": 1,
        "item": "C5",
        "estimate": {
          "value": 1.5922429232572348
        },
        "se": {
          "value": 0.07978188540172723
        },
        "cr": {
          "value": 19.957449178341474
        },
        "p": {
          "value": 1.2913786993599556e-88
        },
        "standardized": {
          "value": 0.9913224975049445
        }
      },
      {
        "factor": 2,
        "item": "C2",
        "estimate": {
          "value": 1
        },
        "se": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "cr": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "p": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "standardized": {
          "value": 0.9817939946152037
        }
      },
      {
        "factor": 2,
        "item": "C4",
        "estimate": {
          "value": 0.7834751696055253
        },
        "se": {
          "value": 0.03739457447725465
        },
        "cr": {
          "value": 20.951573338054057
        },
        "p": {
          "value": 1.8153368029991583e-97
        },
        "standardized": {
          "value": 0.9793410597584835
        }
      },
      {
        "factor": 2,
        "item": "C6",
        "estimate": {
          "value": 0.5183950512816469
        },
        "se": {
          "value": 0.03256073834438717
        },
        "cr": {
          "value": 15.920862905463197
        },
        "p": {
          "value": 4.5407115726465583e-57
        },
        "standardized": {
          "value": 0.9490915183826515
        }
      }
    ],
    "factorVariances": [
      {
        "factor": 1,
        "estimate": {
          "value": 1041379.0177910187
        },
        "se": {
          "value": 252068.89744216794
        },
        "cr": {
          "value": 4.131326904502139
        },
        "p": {
          "value": 0.00003606752301375721
        }
      },
      {
        "factor": 2,
        "estimate": {
          "value": 2544318.7783483854
        },
        "se": {
          "value": 598791.9294522635
        },
        "cr": {
          "value": 4.249086624590557
        },
        "p": {
          "value": 0.000021464391007290476
        }
      }
    ],
    "factorCovariances": [
      {
        "factors": [
          1,
          2
        ],
        "estimate": {
          "value": 328192.12655478506
        },
        "se": {
          "value": 270114.7157393302
        },
        "cr": {
          "value": 1.2150101694996192
        },
        "p": {
          "value": 0.2243622127586616
        },
        "correlation": {
          "value": 0.20162203081567737
        }
      }
    ],
    "errorVariances": [
      {
        "item": "C1",
        "estimate": {
          "value": 70628.70158398642
        },
        "se": {
          "value": 25238.224622255835
        },
        "cr": {
          "value": 2.7984813766061767
        },
        "p": {
          "value": 0.0051343530170865175
        }
      },
      {
        "item": "C3",
        "estimate": {
          "value": 92609.96646244181
        },
        "se": {
          "value": 24055.286924195534
        },
        "cr": {
          "value": 3.8498799350961765
        },
        "p": {
          "value": 0.00011817573991114681
        }
      },
      {
        "item": "C5",
        "estimate": {
          "value": 46423.07461703442
        },
        "se": {
          "value": 50583.877932605654
        },
        "cr": {
          "value": 0.9177444773784488
        },
        "p": {
          "value": 0.3587526563547687
        }
      },
      {
        "item": "C2",
        "estimate": {
          "value": 95236.61602662316
        },
        "se": {
          "value": 46765.62086204267
        },
        "cr": {
          "value": 2.0364664099631784
        },
        "p": {
          "value": 0.04170354274134012
        }
      },
      {
        "item": "C4",
        "estimate": {
          "value": 66585.97210414747
        },
        "se": {
          "value": 29601.298926873566
        },
        "cr": {
          "value": 2.249427373732486
        },
        "p": {
          "value": 0.024485318650604392
        }
      },
      {
        "item": "C6",
        "estimate": {
          "value": 75318.10910852949
        },
        "se": {
          "value": 20386.90823202662
        },
        "cr": {
          "value": 3.6944350880144357
        },
        "p": {
          "value": 0.0002203761800019396
        }
      }
    ],
    "smc": [
      {
        "item": "C1",
        "value": {
          "value": 0.9364854215007764
        }
      },
      {
        "item": "C3",
        "value": {
          "value": 0.8714247618922357
        }
      },
      {
        "item": "C5",
        "value": {
          "value": 0.9827202940594406
        }
      },
      {
        "item": "C2",
        "value": {
          "value": 0.9639194478624786
        }
      },
      {
        "item": "C4",
        "value": {
          "value": 0.9591089113288696
        }
      },
      {
        "item": "C6",
        "value": {
          "value": 0.9007747102658872
        }
      }
    ],
    "fit": {
      "model": {
        "npar": 13,
        "cmin": {
          "value": 26.877282751332075
        },
        "df": 8,
        "p": {
          "value": 0.0007423169672420698
        },
        "cminDf": {
          "value": 3.3596603439165094
        },
        "rmr": {
          "value": 82468.88638350322
        },
        "gfi": {
          "value": 0.853877863430405
        },
        "agfi": {
          "value": 0.616429391504813
        },
        "pgfi": {
          "value": 0.3252868051163447
        },
        "nfi": {
          "value": 0.9307493383443788
        },
        "rfi": {
          "value": 0.8701550093957101
        },
        "ifi": {
          "value": 0.9503380867799814
        },
        "tli": {
          "value": 0.9051369686285788
        },
        "cfi": {
          "value": 0.9494063832685754
        },
        "pratio": {
          "value": 0.5333333333333333
        },
        "pnfi": {
          "value": 0.496399647117002
        },
        "pcfi": {
          "value": 0.5063500710765736
        },
        "ncp": {
          "value": 18.877282751332075
        },
        "ncpLo90": {
          "value": 6.7773388992762325
        },
        "ncpHi90": {
          "value": 38.560651062835646
        },
        "fmin": {
          "value": 0.6891610961880019
        },
        "f0": {
          "value": 0.4840328910597968
        },
        "f0Lo90": {
          "value": 0.17377792049426238
        },
        "f0Hi90": {
          "value": 0.9887346426368114
        },
        "rmsea": {
          "value": 0.24597583495635217
        },
        "rmseaLo90": {
          "value": 0.14738466698331545
        },
        "rmseaHi90": {
          "value": 0.3515562975251637
        },
        "pclose": {
          "value": 0.0018023749469198262
        },
        "aic": {
          "value": 52.877282751332075
        },
        "bcc": {
          "value": 58.564782751332075
        },
        "bic": {
          "value": 74.83271565481326
        },
        "caic": {
          "value": 87.83271565481324
        },
        "ecvi": {
          "value": 1.3558277628546687
        },
        "ecviLo90": {
          "value": 1.045572792289134
        },
        "ecviHi90": {
          "value": 1.8605295144316831
        },
        "mecvi": {
          "value": 1.501661096188002
        },
        "hoelter05": {
          "value": 23
        },
        "hoelter01": {
          "value": 30
        }
      },
      "saturated": {
        "npar": 21,
        "cmin": {
          "value": 0
        },
        "df": 0,
        "p": {
          "not_computable": "stats.cfa.df_zero"
        },
        "cminDf": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmr": {
          "value": 0
        },
        "gfi": {
          "value": 1
        },
        "agfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pgfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "nfi": {
          "value": 1
        },
        "rfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ifi": {
          "value": 1
        },
        "tli": {
          "not_computable": "stats.cfa.df_zero"
        },
        "cfi": {
          "value": 1
        },
        "pratio": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pnfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pcfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ncp": {
          "value": 0
        },
        "ncpLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ncpHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "fmin": {
          "value": 0
        },
        "f0": {
          "value": 0
        },
        "f0Lo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "f0Hi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmsea": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmseaLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmseaHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pclose": {
          "not_computable": "stats.cfa.df_zero"
        },
        "aic": {
          "value": 42
        },
        "bcc": {
          "value": 51.1875
        },
        "bic": {
          "value": 77.46646853639267
        },
        "caic": {
          "value": 98.46646853639265
        },
        "ecvi": {
          "value": 1.0769230769230769
        },
        "ecviLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ecviHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "mecvi": {
          "value": 1.3125
        },
        "hoelter05": {
          "not_computable": "stats.cfa.df_zero"
        },
        "hoelter01": {
          "not_computable": "stats.cfa.df_zero"
        }
      },
      "independence": {
        "npar": 6,
        "cmin": {
          "value": 388.1158982276723
        },
        "df": 15,
        "p": {
          "value": 2.166913889410884e-73
        },
        "cminDf": {
          "value": 25.874393215178152
        },
        "rmr": {
          "value": 777943.5462754564
        },
        "gfi": {
          "value": 0.3536377855947145
        },
        "agfi": {
          "value": 0.09509289983260039
        },
        "pgfi": {
          "value": 0.2525984182819389
        },
        "nfi": {
          "value": 0
        },
        "rfi": {
          "value": 0
        },
        "ifi": {
          "value": 0
        },
        "tli": {
          "value": 0
        },
        "cfi": {
          "value": 0
        },
        "pratio": {
          "value": 1
        },
        "pnfi": {
          "value": 0
        },
        "pcfi": {
          "value": 0
        },
        "ncp": {
          "value": 373.1158982276723
        },
        "ncpLo90": {
          "value": 312.61004694790324
        },
        "ncpHi90": {
          "value": 441.0537453134783
        },
        "fmin": {
          "value": 9.951689698145444
        },
        "f0": {
          "value": 9.567074313530059
        },
        "f0Lo90": {
          "value": 8.015642229433416
        },
        "f0Hi90": {
          "value": 11.30907039265329
        },
        "rmsea": {
          "value": 0.7986269180508113
        },
        "rmseaLo90": {
          "value": 0.731010361505837
        },
        "rmseaHi90": {
          "value": 0.8682960475419003
        },
        "pclose": {
          "value": 1.5423133639431886e-68
        },
        "aic": {
          "value": 400.1158982276723
        },
        "bcc": {
          "value": 402.7408982276723
        },
        "bic": {
          "value": 410.24917495235593
        },
        "caic": {
          "value": 416.24917495235593
        },
        "ecvi": {
          "value": 10.25938200583775
        },
        "ecviLo90": {
          "value": 8.70794992174111
        },
        "ecviHi90": {
          "value": 12.001378084960983
        },
        "mecvi": {
          "value": 10.326689698145444
        },
        "hoelter05": {
          "value": 3
        },
        "hoelter01": {
          "value": 4
        }
      }
    },
    "iterations": 18,
    "exclusions": [
      {
        "code": "stats.cfa.missing_value",
        "count": 1,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [],
    "limitations": [
      "stats.cfa.ml_only",
      "stats.cfa.no_means",
      "stats.cfa.one_group_only",
      "stats.cfa.simple_structure_only",
      "stats.cfa.marker_identification_only",
      "stats.cfa.no_modification_indices",
      "stats.cfa.normal_theory_standard_errors",
      "stats.cfa.no_srmr",
      "stats.cfa.listwise_only",
      "stats.cfa.no_weights"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.cfa.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.cfa.ml_only",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.cfa",
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
        "code": "stats.cfa.missing_value",
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
} satisfies CfaResultView;

export const cfaResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "items": [
      "C1",
      "C2",
      "C3"
    ],
    "rowsUsed": 60,
    "indicators": [
      [
        "C1",
        "C2",
        "C3"
      ]
    ],
    "loadings": [
      {
        "factor": 1,
        "item": "C1",
        "estimate": {
          "value": 1
        },
        "se": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "cr": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "p": {
          "not_computable": "stats.cfa.fixed_parameter"
        },
        "standardized": {
          "value": 2.2999686185989052
        }
      },
      {
        "factor": 1,
        "item": "C2",
        "estimate": {
          "value": 0.15418362053619597
        },
        "se": {
          "value": 0.18120182166139406
        },
        "cr": {
          "value": 0.8508944287785024
        },
        "p": {
          "value": 0.3948279999665571
        },
        "standardized": {
          "value": 0.2947245199798206
        }
      },
      {
        "factor": 1,
        "item": "C3",
        "estimate": {
          "value": 0.22373040709194725
        },
        "se": {
          "value": 0.253678203701134
        },
        "cr": {
          "value": 0.8819457242591123
        },
        "p": {
          "value": 0.377806158930232
        },
        "standardized": {
          "value": 0.3450394429117015
        }
      }
    ],
    "factorVariances": [
      {
        "factor": 1,
        "estimate": {
          "value": 432264.688825408
        },
        "se": {
          "value": 443185.510879637
        },
        "cr": {
          "value": 0.975358350428575
        },
        "p": {
          "value": 0.32938253580996335
        }
      }
    ],
    "factorCovariances": [],
    "errorVariances": [
      {
        "item": "C1",
        "estimate": {
          "value": -350548.9072032451
        },
        "se": {
          "value": 452236.88173138286
        },
        "cr": {
          "value": -0.7751444461167636
        },
        "p": {
          "value": 0.43825431080179145
        }
      },
      {
        "item": "C2",
        "estimate": {
          "value": 108026.38899256426
        },
        "se": {
          "value": 22556.807164360394
        },
        "cr": {
          "value": 4.789081548883622
        },
        "p": {
          "value": 0.0000016754638764874161
        }
      },
      {
        "item": "C3",
        "estimate": {
          "value": 160107.79066199422
        },
        "se": {
          "value": 37026.42666834143
        },
        "cr": {
          "value": 4.324149129921052
        },
        "p": {
          "value": 0.0000153121693568922
        }
      }
    ],
    "smc": [
      {
        "item": "C1",
        "value": {
          "value": 5.289855646539755
        }
      },
      {
        "item": "C2",
        "value": {
          "value": 0.08686254267733562
        }
      },
      {
        "item": "C3",
        "value": {
          "value": 0.11905221716481729
        }
      }
    ],
    "fit": {
      "model": {
        "npar": 6,
        "cmin": {
          "value": 0
        },
        "df": 0,
        "p": {
          "not_computable": "stats.cfa.df_zero"
        },
        "cminDf": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmr": {
          "value": 6.178423498310332e-8
        },
        "gfi": {
          "value": 1
        },
        "agfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pgfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "nfi": {
          "value": 1
        },
        "rfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ifi": {
          "value": 1
        },
        "tli": {
          "not_computable": "stats.cfa.df_zero"
        },
        "cfi": {
          "value": 1
        },
        "pratio": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pnfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pcfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ncp": {
          "value": 0
        },
        "ncpLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ncpHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "fmin": {
          "value": 0
        },
        "f0": {
          "value": 0
        },
        "f0Lo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "f0Hi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmsea": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmseaLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmseaHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pclose": {
          "not_computable": "stats.cfa.df_zero"
        },
        "aic": {
          "value": 12
        },
        "bcc": {
          "value": 12.872727272727273
        },
        "bic": {
          "value": 24.5660673733326
        },
        "caic": {
          "value": 30.5660673733326
        },
        "ecvi": {
          "value": 0.2033898305084746
        },
        "ecviLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ecviHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "mecvi": {
          "value": 0.2181818181818182
        },
        "hoelter05": {
          "not_computable": "stats.cfa.df_zero"
        },
        "hoelter01": {
          "not_computable": "stats.cfa.df_zero"
        }
      },
      "saturated": {
        "npar": 6,
        "cmin": {
          "value": 0
        },
        "df": 0,
        "p": {
          "not_computable": "stats.cfa.df_zero"
        },
        "cminDf": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmr": {
          "value": 0
        },
        "gfi": {
          "value": 1
        },
        "agfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pgfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "nfi": {
          "value": 1
        },
        "rfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ifi": {
          "value": 1
        },
        "tli": {
          "not_computable": "stats.cfa.df_zero"
        },
        "cfi": {
          "value": 1
        },
        "pratio": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pnfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pcfi": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ncp": {
          "value": 0
        },
        "ncpLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ncpHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "fmin": {
          "value": 0
        },
        "f0": {
          "value": 0
        },
        "f0Lo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "f0Hi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmsea": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmseaLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "rmseaHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "pclose": {
          "not_computable": "stats.cfa.df_zero"
        },
        "aic": {
          "value": 12
        },
        "bcc": {
          "value": 12.872727272727273
        },
        "bic": {
          "value": 24.5660673733326
        },
        "caic": {
          "value": 30.5660673733326
        },
        "ecvi": {
          "value": 0.2033898305084746
        },
        "ecviLo90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "ecviHi90": {
          "not_computable": "stats.cfa.df_zero"
        },
        "mecvi": {
          "value": 0.2181818181818182
        },
        "hoelter05": {
          "not_computable": "stats.cfa.df_zero"
        },
        "hoelter01": {
          "not_computable": "stats.cfa.df_zero"
        }
      },
      "independence": {
        "npar": 3,
        "cmin": {
          "value": 272.86000225323716
        },
        "df": 3,
        "p": {
          "value": 7.424997729628903e-59
        },
        "cminDf": {
          "value": 90.95333408441239
        },
        "rmr": {
          "value": 48334.40505656776
        },
        "gfi": {
          "value": 0.5770117295928581
        },
        "agfi": {
          "value": 0.15402345918571614
        },
        "pgfi": {
          "value": 0.28850586479642903
        },
        "nfi": {
          "value": 0
        },
        "rfi": {
          "value": 0
        },
        "ifi": {
          "value": 0
        },
        "tli": {
          "value": 0
        },
        "cfi": {
          "value": 0
        },
        "pratio": {
          "value": 1
        },
        "pnfi": {
          "value": 0
        },
        "pcfi": {
          "value": 0
        },
        "ncp": {
          "value": 269.86000225323716
        },
        "ncpLo90": {
          "value": 219.32646713393584
        },
        "ncpHi90": {
          "value": 327.80881369645357
        },
        "fmin": {
          "value": 4.624745800902325
        },
        "f0": {
          "value": 4.573898343275206
        },
        "f0Lo90": {
          "value": 3.717397748032811
        },
        "f0Hi90": {
          "value": 5.556081588075484
        },
        "rmsea": {
          "value": 1.2347602119811505
        },
        "rmseaLo90": {
          "value": 1.1131633225531659
        },
        "rmseaHi90": {
          "value": 1.360892058917665
        },
        "pclose": {
          "value": 1.6632837345850975e-55
        },
        "aic": {
          "value": 278.86000225323716
        },
        "bcc": {
          "value": 279.2963658896008
        },
        "bic": {
          "value": 285.1430359399035
        },
        "caic": {
          "value": 288.1430359399035
        },
        "ecvi": {
          "value": 4.726440716156562
        },
        "ecviLo90": {
          "value": 3.869940120914167
        },
        "ecviHi90": {
          "value": 5.7086239609568405
        },
        "mecvi": {
          "value": 4.733836709993234
        },
        "hoelter05": {
          "value": 2
        },
        "hoelter01": {
          "value": 3
        }
      }
    },
    "iterations": 11,
    "exclusions": [
      {
        "code": "stats.cfa.missing_code",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.cfa.missing_value",
        "count": 1,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [
      "stats.cfa.negative_variance",
      "stats.cfa.df_zero"
    ],
    "limitations": [
      "stats.cfa.ml_only",
      "stats.cfa.no_means",
      "stats.cfa.one_group_only",
      "stats.cfa.simple_structure_only",
      "stats.cfa.marker_identification_only",
      "stats.cfa.no_modification_indices",
      "stats.cfa.normal_theory_standard_errors",
      "stats.cfa.no_srmr",
      "stats.cfa.listwise_only",
      "stats.cfa.no_weights"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.cfa.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.cfa.ml_only",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.cfa",
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
        "code": "stats.cfa.missing_value",
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
} satisfies CfaResultView;
