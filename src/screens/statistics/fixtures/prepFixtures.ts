// Gaply — typed copies of scale scores' view fixtures (Research OS, Phase
// 8T-1b, decision 499). Each is written `satisfies` its view's type, so tsc
// rejects a renamed, missing or extra key, and prepTypes.vitest.ts
// deep-compares each with the JSON fixture beside it, which a Rust test takes
// through IPC on a synthetic file and compares byte for byte
// (src-tauri/tests/stats_prep_screen_wire.rs). Through IPC the keys come out
// sorted, so they are sorted here too.
import type { PrepMadeView, PrepRecipeView } from '../prepTypes';

export const prepMadeView = {
  counts: [
    {
      column: 'C8',
      notScoredOverflow: 0,
      notScoredTooFew: 0,
      scoredFromEveryItem: 12,
      scoredFromFewer: 0
    }
  ],
  dataset: {
    displayName: 'data.csv',
    inputRef: {
      columns: 8,
      contentHash: '1e82d5f7adfbf58537d7d65f5aa5b6904e0ec7285197b1a5af548f03d6e68986',
      rows: 12,
      semanticRole: 'table'
    },
    preparation: {
      capabilityVersion: 1,
      engineVersion: 1,
      recipeHash: '133676988cd395a241873d95ea8dd63a38c69cb2268970ec4c062849073a1492',
      sourceColumns: 7,
      sourceHash: '6595abb8fb86460d0295beb802f225288a3f73762b172242cb683039d6720f9a'
    },
    profile: {
      columns: [
        {
          alias: 'C1',
          distinct: 5,
          header: 'q1',
          kind: 'numeric',
          levels: null,
          missing: 0,
          n: 12,
          nonFinite: 0,
          suspectMissingCodes: []
        },
        {
          alias: 'C2',
          distinct: 5,
          header: 'q2',
          kind: 'numeric',
          levels: null,
          missing: 0,
          n: 12,
          nonFinite: 0,
          suspectMissingCodes: []
        },
        {
          alias: 'C3',
          distinct: 5,
          header: 'q3',
          kind: 'numeric',
          levels: null,
          missing: 0,
          n: 12,
          nonFinite: 0,
          suspectMissingCodes: []
        },
        {
          alias: 'C4',
          distinct: 5,
          header: 'q4',
          kind: 'numeric',
          levels: null,
          missing: 0,
          n: 12,
          nonFinite: 0,
          suspectMissingCodes: []
        },
        {
          alias: 'C5',
          distinct: 2,
          header: 'arm',
          kind: 'categorical',
          levels: [
            {
              count: 6,
              label: 'a'
            },
            {
              count: 6,
              label: 'b'
            }
          ],
          missing: 0,
          n: 12,
          nonFinite: 0,
          suspectMissingCodes: []
        },
        {
          alias: 'C6',
          distinct: 3,
          header: 'site',
          kind: 'categorical',
          levels: [
            {
              count: 4,
              label: 'x'
            },
            {
              count: 4,
              label: 'y'
            },
            {
              count: 4,
              label: 'z'
            }
          ],
          missing: 0,
          n: 12,
          nonFinite: 0,
          suspectMissingCodes: []
        },
        {
          alias: 'C7',
          distinct: 12,
          header: 'age',
          kind: 'numeric',
          levels: null,
          missing: 0,
          n: 12,
          nonFinite: 0,
          suspectMissingCodes: []
        },
        {
          alias: 'C8',
          distinct: 10,
          header: 'C8',
          kind: 'numeric',
          levels: null,
          missing: 0,
          n: 12,
          nonFinite: 0,
          prepared: true,
          scoreName: 'wellbeing',
          suspectMissingCodes: []
        }
      ],
      rows: 12
    },
    scoreCounts: [
      {
        column: 'C8',
        notScoredOverflow: 0,
        notScoredTooFew: 0,
        scoredFromEveryItem: 12,
        scoredFromFewer: 0
      }
    ],
    slot: 1
  },
  errors: [],
  estimatedResources: {
    cells: 96,
    columns: 8,
    rows: 12
  },
  missingInformation: [],
  status: 'valid',
  warnings: []
} satisfies PrepMadeView;

export const prepMadeViewRefused = {
  counts: [],
  dataset: null,
  errors: [
    {
      code: 'stats.prep.too_few_items',
      params: {
        count: 1,
        index: 1
      },
      severity: 'error'
    },
    {
      code: 'stats.prep.reversed_not_an_item',
      params: {
        column: 2,
        index: 1
      },
      severity: 'error'
    },
    {
      code: 'stats.prep.scale_missing',
      params: {
        index: 1
      },
      severity: 'error'
    }
  ],
  estimatedResources: {
    cells: 108,
    columns: 9,
    rows: 12
  },
  missingInformation: [
    {
      code: 'stats.prep.statistic_not_chosen',
      params: {
        index: 1
      },
      severity: 'error'
    }
  ],
  status: 'invalid',
  warnings: [
    {
      code: 'stats.prep.sum_over_fewer_items',
      params: {
        index: 0
      },
      severity: 'warning'
    }
  ]
} satisfies PrepMadeView;

export const prepRecipeView = {
  counts: [
    {
      column: 'C8',
      notScoredOverflow: 0,
      notScoredTooFew: 0,
      scoredFromEveryItem: 12,
      scoredFromFewer: 0
    }
  ],
  recipe: {
    capabilityId: 'stats.prepare.scale_score',
    capabilityVersion: 1,
    engineVersion: 1,
    schemaVersion: 1,
    sourceColumns: 7,
    steps: [
      {
        alias: 'C8',
        items: [
          'C1',
          'C2',
          'C3'
        ],
        kind: 'scale_score',
        missingCodes: [
          9
        ],
        name: 'wellbeing',
        reversed: [
          'C2'
        ],
        rule: {
          at_least: 2
        },
        scale: {
          max: 5,
          min: 1
        },
        statistic: 'mean'
      }
    ]
  }
} satisfies PrepRecipeView;
