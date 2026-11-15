// Typed copies of the structural equation model's view fixtures (Research OS, Phase 9C-5,
// decision 814): each `satisfies` its view's type, so tsc catches a renamed or missing key or a
// word outside its closed set, and semTypes.vitest.ts deep-compares each with the JSON Rust
// serialises (src-tauri/tests/stats_sem_wire.rs pins those bytes). Generated from the JSON
// once; edit both together. The limits are read from semLimits.json itself, never copied.
//
// The Other result's manifest is the first result's, as the wire test writes it, so no test
// treats either as a stored result.
import type { SemResultView, SemRunStateView, SemValidationView } from '../semTypes';
import SEM_LIMITS_JSON from './semLimits.json';

/** stats_sem's seven limits (MAX_COLUMNS, MAX_COVARIANCES, MAX_EQUAL_MEMBERS, MAX_FACTORS,
 *  MAX_PATHS, MIN_COLUMNS, MIN_FACTOR_ITEMS), as stats_sem_wire.rs writes them. */
export interface SemLimits {
  maxColumns: number;
  maxCovariances: number;
  maxEqualMembers: number;
  maxFactors: number;
  maxPaths: number;
  minColumns: number;
  minFactorItems: number;
}
export const semLimits: SemLimits = SEM_LIMITS_JSON satisfies SemLimits;

export const semValidationView = {
  "status": "invalid",
  "errors": [
    {
      "code": "stats.sem.item_repeated",
      "params": {
        "column": 1
      },
      "severity": "error"
    },
    {
      "code": "stats.sem.equal_unknown_parameter",
      "params": {
        "index": 1,
        "value": 2
      },
      "severity": "error"
    },
    {
      "code": "stats.sem.model_not_identified",
      "params": {
        "count": 1
      },
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "code": "stats.sem.suspect_missing_code",
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
      "code": "stats.sem.no_complete_rows",
      "params": {},
      "severity": "error"
    }
  ],
  "estimatedResources": {
    "rows": 41,
    "columns": 6,
    "cells": 246
  }
} satisfies SemValidationView;

export const semRunStateView = {
  "runId": 5,
  "state": "running",
  "terminal": false,
  "elapsedMs": 12,
  "cancelRequested": true,
  "stepsDone": 7,
  "stepsTotal": 11,
  "diagnostics": [
    {
      "code": "stats.sem.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    }
  ]
} satisfies SemRunStateView;

export const semResultView = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "columns": [
      "C1",
      "C2",
      "C3",
      "C4",
      "C5",
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
    "weights": [
      {
        "to": {
          "column": "C1"
        },
        "from": {
          "factor": 1
        },
        "kind": "loading",
        "estimate": {
          "value": 1
        },
        "se": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "cr": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "p": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "standardized": {
          "value": 0.9677217686409765
        }
      },
      {
        "to": {
          "column": "C3"
        },
        "from": {
          "factor": 1
        },
        "kind": "loading",
        "estimate": {
          "value": 0.7763556742262634
        },
        "se": {
          "value": 0.058196965139470855
        },
        "cr": {
          "value": 13.340140200879937
        },
        "p": {
          "value": 1.3520501927542933e-40
        },
        "standardized": {
          "value": 0.9335013454159818
        }
      },
      {
        "to": {
          "column": "C5"
        },
        "from": {
          "factor": 1
        },
        "kind": "loading",
        "estimate": {
          "value": 1.5922429232557787
        },
        "se": {
          "value": 0.07978188540159506
        },
        "cr": {
          "value": 19.957449178356285
        },
        "p": {
          "value": 1.2913786989772887e-88
        },
        "standardized": {
          "value": 0.991322497504486
        }
      },
      {
        "to": {
          "column": "C2"
        },
        "from": {
          "factor": 2
        },
        "kind": "loading",
        "estimate": {
          "value": 1
        },
        "se": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "cr": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "p": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "standardized": {
          "value": 0.9817939946152112
        }
      },
      {
        "to": {
          "column": "C4"
        },
        "from": {
          "factor": 2
        },
        "kind": "loading",
        "estimate": {
          "value": 0.7834751696054985
        },
        "se": {
          "value": 0.037394574477262195
        },
        "cr": {
          "value": 20.951573338049112
        },
        "p": {
          "value": 1.8153368031877007e-97
        },
        "standardized": {
          "value": 0.979341059758458
        }
      },
      {
        "to": {
          "column": "C6"
        },
        "from": {
          "factor": 2
        },
        "kind": "loading",
        "estimate": {
          "value": 0.5183950512816626
        },
        "se": {
          "value": 0.03256073834437846
        },
        "cr": {
          "value": 15.92086290546794
        },
        "p": {
          "value": 4.5407115723023814e-57
        },
        "standardized": {
          "value": 0.9490915183826869
        }
      },
      {
        "to": {
          "factor": 2
        },
        "from": {
          "factor": 1
        },
        "kind": "path",
        "estimate": {
          "value": 0.31515146832051066
        },
        "se": {
          "value": 0.24934423767606392
        },
        "cr": {
          "value": 1.2639212008979344
        },
        "p": {
          "value": 0.20625831140734488
        },
        "standardized": {
          "value": 0.20162203080361785
        }
      }
    ],
    "covariances": [],
    "variances": [
      {
        "variable": {
          "factor": 1
        },
        "error": false,
        "estimate": {
          "value": 1041379.0177919545
        },
        "se": {
          "value": 252068.8974421773
        },
        "cr": {
          "value": 4.1313269045056975
        },
        "p": {
          "value": 0.00003606752301319878
        }
      },
      {
        "variable": {
          "factor": 2
        },
        "error": true,
        "estimate": {
          "value": 2440888.547779602
        },
        "se": {
          "value": 575140.7799094132
        },
        "cr": {
          "value": 4.243984487005166
        },
        "p": {
          "value": 0.00002195855387439561
        }
      },
      {
        "variable": {
          "column": "C1"
        },
        "error": true,
        "estimate": {
          "value": 70628.70158304865
        },
        "se": {
          "value": 25238.22462209139
        },
        "cr": {
          "value": 2.798481376587254
        },
        "p": {
          "value": 0.005134353017387353
        }
      },
      {
        "variable": {
          "column": "C2"
        },
        "error": true,
        "estimate": {
          "value": 95236.61602658506
        },
        "se": {
          "value": 46765.62086204711
        },
        "cr": {
          "value": 2.0364664099621708
        },
        "p": {
          "value": 0.04170354274144122
        }
      },
      {
        "variable": {
          "column": "C3"
        },
        "error": true,
        "estimate": {
          "value": 92609.9664615682
        },
        "se": {
          "value": 24055.28692401374
        },
        "cr": {
          "value": 3.8498799350889548
        },
        "p": {
          "value": 0.00011817573991463122
        }
      },
      {
        "variable": {
          "column": "C4"
        },
        "error": true,
        "estimate": {
          "value": 66585.97210422931
        },
        "se": {
          "value": 29601.29892688634
        },
        "cr": {
          "value": 2.2494273737342803
        },
        "p": {
          "value": 0.02448531865049037
        }
      },
      {
        "variable": {
          "column": "C5"
        },
        "error": true,
        "estimate": {
          "value": 46423.07461947616
        },
        "se": {
          "value": 50583.87793253835
        },
        "cr": {
          "value": 0.9177444774279409
        },
        "p": {
          "value": 0.358752656328852
        }
      },
      {
        "variable": {
          "column": "C6"
        },
        "error": true,
        "estimate": {
          "value": 75318.1091084791
        },
        "se": {
          "value": 20386.908232017704
        },
        "cr": {
          "value": 3.6944350880135794
        },
        "p": {
          "value": 0.00022037618000268205
        }
      }
    ],
    "smc": [
      {
        "variable": {
          "factor": 2
        },
        "value": {
          "value": 0.04065144330537496
        }
      },
      {
        "variable": {
          "column": "C1"
        },
        "value": {
          "value": 0.9364854215016195
        }
      },
      {
        "variable": {
          "column": "C2"
        },
        "value": {
          "value": 0.9639194478624933
        }
      },
      {
        "variable": {
          "column": "C3"
        },
        "value": {
          "value": 0.8714247618934484
        }
      },
      {
        "variable": {
          "column": "C4"
        },
        "value": {
          "value": 0.9591089113288194
        }
      },
      {
        "variable": {
          "column": "C5"
        },
        "value": {
          "value": 0.9827202940585317
        }
      },
      {
        "variable": {
          "column": "C6"
        },
        "value": {
          "value": 0.9007747102659541
        }
      }
    ],
    "effects": [
      {
        "to": {
          "factor": 2
        },
        "from": {
          "factor": 1
        },
        "total": {
          "value": 0.31515146832051066
        },
        "direct": {
          "value": 0.31515146832051066
        },
        "indirect": {
          "value": 0
        },
        "totalStandardized": {
          "value": 0.20162203080361785
        },
        "directStandardized": {
          "value": 0.20162203080361785
        },
        "indirectStandardized": {
          "value": 0
        }
      }
    ],
    "fit": {
      "model": {
        "npar": 13,
        "cmin": {
          "value": 26.87728275133152
        },
        "df": 8,
        "p": {
          "value": 0.0007423169672422324
        },
        "cminDf": {
          "value": 3.35966034391644
        },
        "rmr": {
          "value": 82468.88638156383
        },
        "gfi": {
          "value": 0.8538778634311099
        },
        "agfi": {
          "value": 0.6164293915066636
        },
        "pgfi": {
          "value": 0.3252868051166133
        },
        "nfi": {
          "value": 0.9307493383443801
        },
        "rfi": {
          "value": 0.8701550093957127
        },
        "ifi": {
          "value": 0.9503380867799829
        },
        "tli": {
          "value": 0.9051369686285815
        },
        "cfi": {
          "value": 0.9494063832685768
        },
        "pratio": {
          "value": 0.5333333333333333
        },
        "pnfi": {
          "value": 0.49639964711700274
        },
        "pcfi": {
          "value": 0.5063500710765743
        },
        "ncp": {
          "value": 18.87728275133152
        },
        "ncpLo90": {
          "value": 6.777338899276092
        },
        "ncpHi90": {
          "value": 38.56065106283485
        },
        "fmin": {
          "value": 0.6891610961879877
        },
        "f0": {
          "value": 0.4840328910597826
        },
        "f0Lo90": {
          "value": 0.17377792049425878
        },
        "f0Hi90": {
          "value": 0.988734642636791
        },
        "rmsea": {
          "value": 0.24597583495634856
        },
        "rmseaLo90": {
          "value": 0.14738466698331393
        },
        "rmseaHi90": {
          "value": 0.3515562975251601
        },
        "pclose": {
          "value": 0.0018023749469201844
        },
        "aic": {
          "value": 52.87728275133152
        },
        "bcc": {
          "value": 58.56478275133152
        },
        "bic": {
          "value": 74.83271565481269
        },
        "caic": {
          "value": 87.83271565481269
        },
        "ecvi": {
          "value": 1.3558277628546545
        },
        "ecviLo90": {
          "value": 1.0455727922891305
        },
        "ecviHi90": {
          "value": 1.860529514431663
        },
        "mecvi": {
          "value": 1.5016610961879877
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
          "not_computable": "stats.sem.df_zero"
        },
        "cminDf": {
          "not_computable": "stats.sem.df_zero"
        },
        "rmr": {
          "value": 0
        },
        "gfi": {
          "value": 1
        },
        "agfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "pgfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "nfi": {
          "value": 1
        },
        "rfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "ifi": {
          "value": 1
        },
        "tli": {
          "not_computable": "stats.sem.df_zero"
        },
        "cfi": {
          "value": 1
        },
        "pratio": {
          "not_computable": "stats.sem.df_zero"
        },
        "pnfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "pcfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "ncp": {
          "value": 0
        },
        "ncpLo90": {
          "not_computable": "stats.sem.df_zero"
        },
        "ncpHi90": {
          "not_computable": "stats.sem.df_zero"
        },
        "fmin": {
          "value": 0
        },
        "f0": {
          "value": 0
        },
        "f0Lo90": {
          "not_computable": "stats.sem.df_zero"
        },
        "f0Hi90": {
          "not_computable": "stats.sem.df_zero"
        },
        "rmsea": {
          "not_computable": "stats.sem.df_zero"
        },
        "rmseaLo90": {
          "not_computable": "stats.sem.df_zero"
        },
        "rmseaHi90": {
          "not_computable": "stats.sem.df_zero"
        },
        "pclose": {
          "not_computable": "stats.sem.df_zero"
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
          "not_computable": "stats.sem.df_zero"
        },
        "ecviHi90": {
          "not_computable": "stats.sem.df_zero"
        },
        "mecvi": {
          "value": 1.3125
        },
        "hoelter05": {
          "not_computable": "stats.sem.df_zero"
        },
        "hoelter01": {
          "not_computable": "stats.sem.df_zero"
        }
      },
      "independence": {
        "npar": 6,
        "cmin": {
          "value": 388.1158982276718
        },
        "df": 15,
        "p": {
          "value": 2.166913889411441e-73
        },
        "cminDf": {
          "value": 25.87439321517812
        },
        "rmr": {
          "value": 777943.5462754563
        },
        "gfi": {
          "value": 0.35363778559471437
        },
        "agfi": {
          "value": 0.09509289983260016
        },
        "pgfi": {
          "value": 0.25259841828193885
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
          "value": 373.1158982276718
        },
        "ncpLo90": {
          "value": 312.6100469479028
        },
        "ncpHi90": {
          "value": 441.05374531347775
        },
        "fmin": {
          "value": 9.95168969814543
        },
        "f0": {
          "value": 9.567074313530046
        },
        "f0Lo90": {
          "value": 8.015642229433405
        },
        "f0Hi90": {
          "value": 11.309070392653275
        },
        "rmsea": {
          "value": 0.7986269180508108
        },
        "rmseaLo90": {
          "value": 0.7310103615058364
        },
        "rmseaHi90": {
          "value": 0.8682960475418997
        },
        "pclose": {
          "value": 1.542313363943559e-68
        },
        "aic": {
          "value": 400.1158982276718
        },
        "bcc": {
          "value": 402.7408982276718
        },
        "bic": {
          "value": 410.2491749523554
        },
        "caic": {
          "value": 416.2491749523554
        },
        "ecvi": {
          "value": 10.259382005837738
        },
        "ecviLo90": {
          "value": 8.707949921741097
        },
        "ecviHi90": {
          "value": 12.001378084960967
        },
        "mecvi": {
          "value": 10.32668969814543
        },
        "hoelter05": {
          "value": 3
        },
        "hoelter01": {
          "value": 4
        }
      }
    },
    "iterations": 19,
    "attempt": 1,
    "exclusions": [
      {
        "code": "stats.sem.missing_value",
        "count": 1,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [],
    "limitations": [
      "stats.sem.ml_only",
      "stats.sem.no_means",
      "stats.sem.one_group_only",
      "stats.sem.indicators_measure_one_factor",
      "stats.sem.equal_one_kind",
      "stats.sem.marker_identification_only",
      "stats.sem.no_fixed_values",
      "stats.sem.effects_structural_only",
      "stats.sem.no_indirect_effect_tests",
      "stats.sem.no_stability_index",
      "stats.sem.no_modification_indices",
      "stats.sem.normal_theory_standard_errors",
      "stats.sem.no_srmr",
      "stats.sem.listwise_only",
      "stats.sem.no_weights"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.sem.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.sem.ml_only",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.sem",
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
        "code": "stats.sem.missing_value",
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
} satisfies SemResultView;

export const semResultViewOther = {
  "runId": 5,
  "status": "succeeded",
  "result": {
    "columns": [
      "C1",
      "C2",
      "C3",
      "C4",
      "C5",
      "C6",
      "C7",
      "C8",
      "C9",
      "C10"
    ],
    "rowsUsed": 60,
    "indicators": [
      [
        "C5",
        "C6",
        "C7"
      ],
      [
        "C8",
        "C9",
        "C10"
      ]
    ],
    "weights": [
      {
        "to": {
          "column": "C5"
        },
        "from": {
          "factor": 1
        },
        "kind": "loading",
        "estimate": {
          "value": 1
        },
        "se": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "cr": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "p": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "standardized": {
          "value": 2.3966884710211125
        }
      },
      {
        "to": {
          "column": "C6"
        },
        "from": {
          "factor": 1
        },
        "kind": "loading",
        "estimate": {
          "value": 0.140984044644895
        },
        "se": {
          "value": 0.18286482790436376
        },
        "cr": {
          "value": 0.7709740919594886
        },
        "p": {
          "value": 0.4407222883311424
        },
        "standardized": {
          "value": 0.28083051308188856
        }
      },
      {
        "to": {
          "column": "C7"
        },
        "from": {
          "factor": 1
        },
        "kind": "loading",
        "estimate": {
          "value": 0.20566298204074104
        },
        "se": {
          "value": 0.2579696349181965
        },
        "cr": {
          "value": 0.797237171366924
        },
        "p": {
          "value": 0.4253133017293674
        },
        "standardized": {
          "value": 0.3305187658221991
        }
      },
      {
        "to": {
          "column": "C8"
        },
        "from": {
          "factor": 2
        },
        "kind": "loading",
        "estimate": {
          "value": 1
        },
        "se": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "cr": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "p": {
          "not_computable": "stats.sem.fixed_parameter"
        },
        "standardized": {
          "value": 0.9787188096362948
        }
      },
      {
        "to": {
          "column": "C9"
        },
        "from": {
          "factor": 2
        },
        "kind": "loading",
        "estimate": {
          "value": 0.8941900277982475
        },
        "se": {
          "value": 0.03579139195139329
        },
        "cr": {
          "value": 24.98338228959096
        },
        "p": {
          "value": 9.266905535355637e-138
        },
        "standardized": {
          "value": 0.9785900645293798
        }
      },
      {
        "to": {
          "column": "C10"
        },
        "from": {
          "factor": 2
        },
        "kind": "loading",
        "estimate": {
          "value": 1.2138028652520645
        },
        "se": {
          "value": 0.05494328719409478
        },
        "cr": {
          "value": 22.09192291251409
        },
        "p": {
          "value": 3.779603368695936e-108
        },
        "standardized": {
          "value": 0.96592043985116
        }
      },
      {
        "to": {
          "column": "C1"
        },
        "from": {
          "column": "C2"
        },
        "kind": "path",
        "estimate": {
          "value": 0.5100249048116374
        },
        "se": {
          "value": 0.07876600546107519
        },
        "cr": {
          "value": 6.475190684434073
        },
        "p": {
          "value": 9.469228002082675e-11
        },
        "standardized": {
          "value": 0.45127874079415764
        }
      },
      {
        "to": {
          "column": "C1"
        },
        "from": {
          "column": "C3"
        },
        "kind": "path",
        "estimate": {
          "value": 0.5645671619785164
        },
        "se": {
          "value": 0.058581558397217265
        },
        "cr": {
          "value": 9.637284794481234
        },
        "p": {
          "value": 5.564019523686123e-22
        },
        "standardized": {
          "value": 0.5915139041641865
        }
      },
      {
        "to": {
          "column": "C2"
        },
        "from": {
          "column": "C1"
        },
        "kind": "path",
        "estimate": {
          "value": 0.37647304385982616
        },
        "se": {
          "value": 0.07747031107874705
        },
        "cr": {
          "value": 4.859578316100328
        },
        "p": {
          "value": 0.0000011763604157218916
        },
        "standardized": {
          "value": 0.425481218151017
        }
      },
      {
        "to": {
          "column": "C2"
        },
        "from": {
          "column": "C4"
        },
        "kind": "path",
        "estimate": {
          "value": 0.44320698552346555
        },
        "se": {
          "value": 0.06569004955280378
        },
        "cr": {
          "value": 6.746942475164393
        },
        "p": {
          "value": 1.5099310294797555e-11
        },
        "standardized": {
          "value": 0.5081648743195345
        }
      }
    ],
    "covariances": [
      {
        "a": {
          "factor": 1
        },
        "b": {
          "factor": 2
        },
        "estimate": {
          "value": -1002.0732384139313
        },
        "se": {
          "value": 1506.1926482981967
        },
        "cr": {
          "value": -0.6653021707058157
        },
        "p": {
          "value": 0.5058572424792235
        },
        "correlation": {
          "value": -0.0044512477235417285
        }
      },
      {
        "a": {
          "factor": 1
        },
        "b": {
          "column": "C3"
        },
        "estimate": {
          "value": 1017.6974255813747
        },
        "se": {
          "value": 1355.734449771361
        },
        "cr": {
          "value": 0.750661330287067
        },
        "p": {
          "value": 0.4528565009811878
        },
        "correlation": {
          "value": 0.005009153778860713
        }
      },
      {
        "a": {
          "column": "C3"
        },
        "b": {
          "column": "C4"
        },
        "estimate": {
          "value": -3242.530967862571
        },
        "se": {
          "value": 11037.546985228591
        },
        "cr": {
          "value": -0.2937727895701834
        },
        "p": {
          "value": 0.7689315288629295
        },
        "correlation": {
          "value": -0.03808165879990552
        }
      }
    ],
    "variances": [
      {
        "variable": {
          "factor": 1
        },
        "error": false,
        "estimate": {
          "value": 469399.41148130136
        },
        "se": {
          "value": 537867.5513542787
        },
        "cr": {
          "value": 0.8727044609763431
        },
        "p": {
          "value": 0.3828241856069687
        }
      },
      {
        "variable": {
          "factor": 2
        },
        "error": false,
        "estimate": {
          "value": 107967.45340855408
        },
        "se": {
          "value": 20773.264576206988
        },
        "cr": {
          "value": 5.197423496556071
        },
        "p": {
          "value": 2.0206965742528014e-7
        }
      },
      {
        "variable": {
          "column": "C1"
        },
        "error": true,
        "estimate": {
          "value": 15906.945692282457
        },
        "se": {
          "value": 3152.862965118679
        },
        "cr": {
          "value": 5.0452385239279485
        },
        "p": {
          "value": 4.5295594475529955e-7
        }
      },
      {
        "variable": {
          "column": "C2"
        },
        "error": true,
        "estimate": {
          "value": 19132.332672716737
        },
        "se": {
          "value": 3988.698209276848
        },
        "cr": {
          "value": 4.796635811708962
        },
        "p": {
          "value": 0.0000016135257268802799
        }
      },
      {
        "variable": {
          "column": "C3"
        },
        "error": false,
        "estimate": {
          "value": 87935.87350087032
        },
        "se": {
          "value": 16190.071280880016
        },
        "cr": {
          "value": 5.431469199565534
        },
        "p": {
          "value": 5.5891963819263654e-8
        }
      },
      {
        "variable": {
          "column": "C4"
        },
        "error": false,
        "estimate": {
          "value": 82446.16666666669
        },
        "se": {
          "value": 15179.569675269642
        },
        "cr": {
          "value": 5.431390245600105
        },
        "p": {
          "value": 5.591670254262935e-8
        }
      },
      {
        "variable": {
          "column": "C5"
        },
        "error": true,
        "estimate": {
          "value": -387681.1032243001
        },
        "se": {
          "value": 547050.945039915
        },
        "cr": {
          "value": -0.7086745882433552
        },
        "p": {
          "value": 0.4785264379317532
        }
      },
      {
        "variable": {
          "column": "C6"
        },
        "error": true,
        "estimate": {
          "value": 108972.47213274015
        },
        "se": {
          "value": 22775.557492822245
        },
        "cr": {
          "value": 4.784623698764915
        },
        "p": {
          "value": 0.0000017130797053804717
        }
      },
      {
        "variable": {
          "column": "C7"
        },
        "error": true,
        "estimate": {
          "value": 161890.7240984928
        },
        "se": {
          "value": 37611.651705099255
        },
        "cr": {
          "value": 4.304270532116626
        },
        "p": {
          "value": 0.000016753683895401407
        }
      },
      {
        "variable": {
          "column": "C8"
        },
        "error": true,
        "estimate": {
          "value": 4746.319688668356
        },
        "se": {
          "value": 1548.666231816536
        },
        "cr": {
          "value": 3.0647789634446116
        },
        "p": {
          "value": 0.002178309212782527
        }
      },
      {
        "variable": {
          "column": "C9"
        },
        "error": true,
        "estimate": {
          "value": 3818.757498743994
        },
        "se": {
          "value": 1240.7457335860634
        },
        "cr": {
          "value": 3.0777921659314003
        },
        "p": {
          "value": 0.002085403017174502
        }
      },
      {
        "variable": {
          "column": "C10"
        },
        "error": true,
        "estimate": {
          "value": 11422.637143382512
        },
        "se": {
          "value": 2823.493096758824
        },
        "cr": {
          "value": 4.045569353966162
        },
        "p": {
          "value": 0.0000521961094798141
        }
      }
    ],
    "smc": [
      {
        "variable": {
          "column": "C1"
        },
        "value": {
          "value": 0.8014273626165801
        }
      },
      {
        "variable": {
          "column": "C2"
        },
        "value": {
          "value": 0.6949341834072045
        }
      },
      {
        "variable": {
          "column": "C5"
        },
        "value": {
          "value": 5.7441156271255185
        }
      },
      {
        "variable": {
          "column": "C6"
        },
        "value": {
          "value": 0.07886577707783671
        }
      },
      {
        "variable": {
          "column": "C7"
        },
        "value": {
          "value": 0.10924265456062965
        }
      },
      {
        "variable": {
          "column": "C8"
        },
        "value": {
          "value": 0.9578905083358857
        }
      },
      {
        "variable": {
          "column": "C9"
        },
        "value": {
          "value": 0.9576385143956151
        }
      },
      {
        "variable": {
          "column": "C10"
        },
        "value": {
          "value": 0.9330022961222583
        }
      }
    ],
    "effects": [
      {
        "to": {
          "column": "C1"
        },
        "from": {
          "column": "C2"
        },
        "total": {
          "value": 0.6312272447045176
        },
        "direct": {
          "value": 0.5100249048116374
        },
        "indirect": {
          "value": 0.1212023398928802
        },
        "totalStandardized": {
          "value": 0.5585206397919426
        },
        "directStandardized": {
          "value": 0.45127874079415764
        },
        "indirectStandardized": {
          "value": 0.1072418989977849
        }
      },
      {
        "to": {
          "column": "C1"
        },
        "from": {
          "column": "C3"
        },
        "total": {
          "value": 0.6987309261651895
        },
        "direct": {
          "value": 0.5645671619785164
        },
        "indirect": {
          "value": 0.13416376418667308
        },
        "totalStandardized": {
          "value": 0.7320812933005071
        },
        "directStandardized": {
          "value": 0.5915139041641865
        },
        "indirectStandardized": {
          "value": 0.1405673891363206
        }
      },
      {
        "to": {
          "column": "C1"
        },
        "from": {
          "column": "C4"
        },
        "total": {
          "value": 0.2797643243057722
        },
        "direct": {
          "value": 0
        },
        "indirect": {
          "value": 0.2797643243057722
        },
        "totalStandardized": {
          "value": 0.2838205707247385
        },
        "directStandardized": {
          "value": 0
        },
        "indirectStandardized": {
          "value": 0.2838205707247385
        }
      },
      {
        "to": {
          "column": "C2"
        },
        "from": {
          "column": "C1"
        },
        "total": {
          "value": 0.46593811388274536
        },
        "direct": {
          "value": 0.37647304385982616
        },
        "indirect": {
          "value": 0.0894650700229192
        },
        "totalStandardized": {
          "value": 0.5265925927797166
        },
        "directStandardized": {
          "value": 0.425481218151017
        },
        "indirectStandardized": {
          "value": 0.1011113746286995
        }
      },
      {
        "to": {
          "column": "C2"
        },
        "from": {
          "column": "C3"
        },
        "total": {
          "value": 0.26305335861240436
        },
        "direct": {
          "value": 0
        },
        "indirect": {
          "value": 0.26305335861240436
        },
        "totalStandardized": {
          "value": 0.3114868404590717
        },
        "directStandardized": {
          "value": 0
        },
        "indirectStandardized": {
          "value": 0.3114868404590717
        }
      },
      {
        "to": {
          "column": "C2"
        },
        "from": {
          "column": "C4"
        },
        "total": {
          "value": 0.5485307122582471
        },
        "direct": {
          "value": 0.44320698552346555
        },
        "indirect": {
          "value": 0.10532372673478158
        },
        "totalStandardized": {
          "value": 0.6289251964878131
        },
        "directStandardized": {
          "value": 0.5081648743195345
        },
        "indirectStandardized": {
          "value": 0.12076032216827857
        }
      }
    ],
    "fit": {
      "model": {
        "npar": 23,
        "cmin": {
          "value": 49.4201285680456
        },
        "df": 32,
        "p": {
          "value": 0.02533261756260896
        },
        "cminDf": {
          "value": 1.544379017751425
        },
        "rmr": {
          "value": 19785.512075308954
        },
        "gfi": {
          "value": 0.8760673337171209
        },
        "agfi": {
          "value": 0.7869907298263015
        },
        "pgfi": {
          "value": 0.5097119032535976
        },
        "nfi": {
          "value": 0.9343464095572522
        },
        "rfi": {
          "value": 0.907674638439886
        },
        "ifi": {
          "value": 0.9758302410866594
        },
        "tli": {
          "value": 0.965386961132366
        },
        "cfi": {
          "value": 0.9753862834719047
        },
        "pratio": {
          "value": 0.7111111111111111
        },
        "pnfi": {
          "value": 0.664424113462935
        },
        "pcfi": {
          "value": 0.6936080238022434
        },
        "ncp": {
          "value": 17.420128568045598
        },
        "ncpLo90": {
          "value": 2.2612534502705115
        },
        "ncpHi90": {
          "value": 40.51037806135618
        },
        "fmin": {
          "value": 0.8376292977634847
        },
        "f0": {
          "value": 0.2952564164075525
        },
        "f0Lo90": {
          "value": 0.03832632966560189
        },
        "f0Hi90": {
          "value": 0.6866165773111217
        },
        "rmsea": {
          "value": 0.0960560410007409
        },
        "rmseaLo90": {
          "value": 0.034607770833297816
        },
        "rmseaHi90": {
          "value": 0.14648128904734745
        },
        "pclose": {
          "value": 0.08991984938850214
        },
        "aic": {
          "value": 95.4201285680456
        },
        "bcc": {
          "value": 105.96179523471227
        },
        "bic": {
          "value": 143.5900534991539
        },
        "caic": {
          "value": 166.5900534991539
        },
        "ecvi": {
          "value": 1.6172903147126372
        },
        "ecviLo90": {
          "value": 1.3603602279706868
        },
        "ecviHi90": {
          "value": 2.0086504756162062
        },
        "mecvi": {
          "value": 1.7959626310968182
        },
        "hoelter05": {
          "value": 56
        },
        "hoelter01": {
          "value": 64
        }
      },
      "saturated": {
        "npar": 55,
        "cmin": {
          "value": 0
        },
        "df": 0,
        "p": {
          "not_computable": "stats.sem.df_zero"
        },
        "cminDf": {
          "not_computable": "stats.sem.df_zero"
        },
        "rmr": {
          "value": 0
        },
        "gfi": {
          "value": 1
        },
        "agfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "pgfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "nfi": {
          "value": 1
        },
        "rfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "ifi": {
          "value": 1
        },
        "tli": {
          "not_computable": "stats.sem.df_zero"
        },
        "cfi": {
          "value": 1
        },
        "pratio": {
          "not_computable": "stats.sem.df_zero"
        },
        "pnfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "pcfi": {
          "not_computable": "stats.sem.df_zero"
        },
        "ncp": {
          "value": 0
        },
        "ncpLo90": {
          "not_computable": "stats.sem.df_zero"
        },
        "ncpHi90": {
          "not_computable": "stats.sem.df_zero"
        },
        "fmin": {
          "value": 0
        },
        "f0": {
          "value": 0
        },
        "f0Lo90": {
          "not_computable": "stats.sem.df_zero"
        },
        "f0Hi90": {
          "not_computable": "stats.sem.df_zero"
        },
        "rmsea": {
          "not_computable": "stats.sem.df_zero"
        },
        "rmseaLo90": {
          "not_computable": "stats.sem.df_zero"
        },
        "rmseaHi90": {
          "not_computable": "stats.sem.df_zero"
        },
        "pclose": {
          "not_computable": "stats.sem.df_zero"
        },
        "aic": {
          "value": 110
        },
        "bcc": {
          "value": 135.20833333333334
        },
        "bic": {
          "value": 225.18895092221553
        },
        "caic": {
          "value": 280.1889509222155
        },
        "ecvi": {
          "value": 1.8644067796610169
        },
        "ecviLo90": {
          "not_computable": "stats.sem.df_zero"
        },
        "ecviHi90": {
          "not_computable": "stats.sem.df_zero"
        },
        "mecvi": {
          "value": 2.291666666666667
        },
        "hoelter05": {
          "not_computable": "stats.sem.df_zero"
        },
        "hoelter01": {
          "not_computable": "stats.sem.df_zero"
        }
      },
      "independence": {
        "npar": 10,
        "cmin": {
          "value": 752.740683864681
        },
        "df": 45,
        "p": {
          "value": 3.702988948109226e-129
        },
        "cminDf": {
          "value": 16.727570752548466
        },
        "rmr": {
          "value": 39096.68316857566
        },
        "gfi": {
          "value": 0.4233513716433994
        },
        "agfi": {
          "value": 0.2952072320085992
        },
        "pgfi": {
          "value": 0.3463783949809632
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
          "value": 707.740683864681
        },
        "ncpLo90": {
          "value": 622.5357999371347
        },
        "ncpHi90": {
          "value": 800.39090794419
        },
        "fmin": {
          "value": 12.75831667567256
        },
        "f0": {
          "value": 11.99560481126578
        },
        "f0Lo90": {
          "value": 10.551454236222622
        },
        "f0Hi90": {
          "value": 13.565947592274407
        },
        "rmsea": {
          "value": 0.5163032014290694
        },
        "rmseaLo90": {
          "value": 0.48422800497797225
        },
        "rmseaHi90": {
          "value": 0.5490587418942916
        },
        "pclose": {
          "value": 1.6023034409802315e-115
        },
        "aic": {
          "value": 772.740683864681
        },
        "bcc": {
          "value": 777.3240171980144
        },
        "bic": {
          "value": 793.6841294869021
        },
        "caic": {
          "value": 803.6841294869021
        },
        "ecvi": {
          "value": 13.097299726520017
        },
        "ecviLo90": {
          "value": 11.653149151476859
        },
        "ecviHi90": {
          "value": 14.667642507528644
        },
        "mecvi": {
          "value": 13.174983342339228
        },
        "hoelter05": {
          "value": 5
        },
        "hoelter01": {
          "value": 6
        }
      }
    },
    "iterations": 49,
    "attempt": 1,
    "exclusions": [
      {
        "code": "stats.sem.missing_code",
        "count": 1,
        "params": {
          "column": 1
        }
      },
      {
        "code": "stats.sem.missing_value",
        "count": 1,
        "params": {
          "column": 2
        }
      }
    ],
    "notes": [
      "stats.sem.negative_variance",
      "stats.sem.nonrecursive"
    ],
    "limitations": [
      "stats.sem.ml_only",
      "stats.sem.no_means",
      "stats.sem.one_group_only",
      "stats.sem.indicators_measure_one_factor",
      "stats.sem.equal_one_kind",
      "stats.sem.marker_identification_only",
      "stats.sem.no_fixed_values",
      "stats.sem.effects_structural_only",
      "stats.sem.no_indirect_effect_tests",
      "stats.sem.no_stability_index",
      "stats.sem.no_modification_indices",
      "stats.sem.normal_theory_standard_errors",
      "stats.sem.no_srmr",
      "stats.sem.listwise_only",
      "stats.sem.no_weights"
    ]
  },
  "diagnostics": [
    {
      "code": "stats.sem.suspect_missing_code",
      "params": {
        "column": 1,
        "count": 3,
        "value": 99
      },
      "severity": "warning"
    },
    {
      "code": "stats.sem.ml_only",
      "params": {},
      "severity": "note"
    }
  ],
  "manifest": {
    "schemaVersion": 1,
    "capabilityId": "stats.analyze.sem",
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
        "code": "stats.sem.missing_value",
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
} satisfies SemResultView;
