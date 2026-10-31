// Gaply — what the method selector says about each Statistics capability
// (Research OS, Phase 4B-2). Rust's descriptors carry closed wording codes,
// never sentences (decision 46); this table turns each into words. The code
// list is capabilityCodes.json, which src-tauri/tests/stats_ttest_wire.rs
// pins to StatsWording exactly, and capabilityWording.vitest.ts fails if a
// code has no sentence.
import type { Lifecycle } from './ttestTypes';

export const CAPABILITY_WORDS: Record<string, string> = {
  'stats.capability.check.title': 'Statistical Analysis Check',
  'stats.capability.check.description':
    "Reads a manuscript and checks that the statistics it reports agree with each other: each test statistic, its degrees of freedom and its p-value.",
  'stats.capability.verify.independent_t.title': 'Independent-samples t-test',
  'stats.capability.verify.paired_t.title': 'Paired t-test',
  'stats.capability.verify.one_way_anova.title': 'One-way ANOVA',
  'stats.capability.verify.pearson.title': 'Pearson correlation',
  'stats.capability.verify.spearman.title': 'Spearman correlation',
  'stats.capability.verify.chi_square_independence.title': 'Chi-square test of independence',
  'stats.capability.verify.ols.title': 'Linear regression (ordinary least squares)',
  'stats.capability.verify.description': "Recomputes a result a paper reports from the paper's own data, and says whether it is reproduced.",
  'stats.capability.describe.title': 'Descriptive summary',
  'stats.capability.describe.description':
    'For each column you choose: n, missing, mean, standard deviation, minimum, quartiles and maximum, or the count of each level.',
  'stats.capability.license.statrs_mit': 'p-values are computed with the statrs library, version 0.18.0, under the MIT licence.',
  'stats.capability.limit.check.summary_methods_not_on_manuscripts':
    'The GRIM, percentage and count checks do not run on a manuscript: Gaply cannot read their inputs from one.',
  'stats.capability.limit.check.tables_from_docx_only':
    'Tables are read as tables only from a .docx file: a PDF or plain text has no table Gaply can read.',
  'stats.capability.limit.table.text_files_utf8_only': 'A CSV or TSV file must be UTF-8: a file in another encoding is refused.',
  'stats.capability.limit.table.cell_text_cap':
    'A table may hold at most 2 MiB of cell text, which can be reached before the row and column limits on a wide table.',
  'stats.capability.limit.table.limits_by_memory':
    "How large a file Gaply opens depends on this computer's memory. Each method's screen states the limits before you choose a file.",
  'stats.capability.limit.describe.levels_follow_level_1':
    "Levels follow Level 1's reading of cells: 1 and 1.0 are one level, and text is trimmed of spaces at either end.",
  'stats.capability.limit.describe.rows_used_equals_rows_read':
    'Cells are left out, never whole rows, so rows used always equals rows read.',
  'stats.capability.analyze.independent_t.title': 'Independent-samples t-test on your data',
  'stats.capability.analyze.independent_t.description':
    "Compares the mean of a numeric outcome between two groups in your own data file, with Student's or Welch's t-test, which you choose.",
  'stats.capability.limit.analyze.levels_follow_level_1':
    "The group's levels follow Level 1's reading of cells and are listed in the order Gaply reads them; you choose the two to compare.",
  'stats.capability.limit.analyze.normality_not_assessed': 'Normality is not assessed: Gaply does not check whether each group is roughly normal.',
  'stats.capability.limit.analyze.cohens_d_has_no_interval': "Cohen's d is shown without a confidence interval.",
  // Phase 8D-1 (decision 253): the one-sample and paired t-tests, Beta since
  // Phase 8D-2a (decision 265).
  'stats.capability.analyze.one_sample_t.title': 'One-sample t-test on your data',
  'stats.capability.analyze.one_sample_t.description': 'Compares the mean of one numeric column with a value you give.',
  'stats.capability.analyze.paired_t.title': 'Paired-samples t-test on your data',
  'stats.capability.analyze.paired_t.description':
    'Compares two numeric columns measured on the same rows, such as before and after, through the differences between them.',
  'stats.capability.limit.analyze.values_normality_not_assessed':
    'Normality is not assessed: the t-test assumes the values come from a roughly normal distribution, or that there are many of them, and Gaply does not check either.',
  'stats.capability.limit.analyze.differences_normality_not_assessed':
    'Normality is not assessed: the paired t-test assumes the differences come from a roughly normal distribution, or that there are many pairs, and Gaply does not check either.',
  // Phase 8G (decisions 312, 319 and 321): the one-way ANOVA, Planned.
  'stats.capability.analyze.one_way_anova.title': 'One-way ANOVA on your data',
  'stats.capability.analyze.one_way_anova.description':
    "Compares the means of a numeric outcome across two or more groups in your own data file, with the ANOVA table, Welch's ANOVA, Levene's test and the post hoc tests you choose.",
  'stats.capability.limit.analyze.effect_sizes_have_no_interval': 'Eta squared and omega squared are shown without confidence intervals.',
  'stats.capability.limit.analyze.no_robust_means_trends_or_contrasts':
    'There is no Brown–Forsythe test of means, no trend test and no planned contrast.',
};

/** The words for a code; a code with no entry is shown as itself, never
 *  hidden. */
export function capabilitySentence(code: string): string {
  return CAPABILITY_WORDS[code] ?? code;
}

export const LIFECYCLE_LABELS: Record<Lifecycle, string> = {
  planned: 'Planned',
  experimental: 'Experimental',
  beta: 'Beta',
  stable: 'Stable',
  deprecated: 'Retired',
};

export const HOME_TEXT = {
  heading: 'Statistics',
  intro:
    'Choose a method. Each one reads your file on this device and says what it cannot do. Gaply lists here only the methods this version can run.',
  open: 'Open',
  roadmap: 'Planned',
  roadmapIntro: 'These are planned and cannot run yet.',
  retired: 'Retired methods',
  retiredIntro: 'These no longer run in this version.',
  limitations: 'What it does not do',
  verifierMethods: 'Methods it can reproduce:',
  noRoute: 'This method runs in Gaply, but has no screen of its own here yet.',
  unread: 'The list of methods could not be read.',
};
