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
  // Phase 8G (decisions 312, 319 and 321): the one-way ANOVA, Planned in 8G
  // and Beta since 8G-2 (decision 333).
  'stats.capability.analyze.one_way_anova.title': 'One-way ANOVA on your data',
  'stats.capability.analyze.one_way_anova.description':
    "Compares the means of a numeric outcome across two or more groups in your own data file, with the ANOVA table, Welch's ANOVA, Levene's test and the post hoc tests you choose.",
  'stats.capability.limit.analyze.effect_sizes_have_no_interval': 'Eta squared and omega squared are shown without confidence intervals.',
  'stats.capability.limit.analyze.no_robust_means_trends_or_contrasts':
    'There is no Brown–Forsythe test of means, no trend test and no planned contrast.',
  // Phase 8H (decisions 336 and 344): the correlations, Planned in 8H and
  // Beta since 8H-2 (decision 351).
  'stats.capability.analyze.correlations.title': 'Correlations on your data',
  'stats.capability.analyze.correlations.description':
    "Pearson's r, Spearman's rho and Kendall's tau-b between two or more numeric columns of your own data file, with each pair's n, p and 95% interval.",
  'stats.capability.limit.analyze.pearson_normality_not_checked':
    "Pearson's p and interval assume the two columns are roughly normal together; Gaply does not check.",
  'stats.capability.limit.analyze.two_tailed_no_partial': 'p values are two-tailed only, and there are no partial correlations.',
  // Phase 8I (decisions 367 and 372): Cronbach's alpha, Planned in 8I and
  // Beta since 8I-2 (decision 377).
  'stats.capability.analyze.cronbach_alpha.title': "Cronbach's alpha on your data",
  'stats.capability.analyze.cronbach_alpha.description':
    "Cronbach's alpha for a scale of two or more numeric items in your own data file, with standardized alpha, a 95% interval, and each item's statistics if it were left out, with the items you choose reverse-scored.",
  'stats.capability.limit.analyze.alpha_one_dimension': 'Alpha assumes the items measure one thing; Gaply does not check that.',
  'stats.capability.limit.analyze.no_omega_or_smc': "McDonald's omega and squared multiple correlations are not shown yet.",
  // Phase 8X-1 (decisions 575, 578 and 579): crosstabs and chi-square,
  // Planned in Part 2 and Beta from Part 3 (decision 580).
  'stats.capability.analyze.crosstabs.title': 'Crosstabs and chi-square on your data',
  'stats.capability.analyze.crosstabs.description':
    "Counts how often each pair of levels of two columns of your own data file occurs together, with expected counts, residuals, Pearson's chi-square, the likelihood ratio, Fisher's exact test for a table of two rows and two columns, phi, Cramer's V and the contingency coefficient.",
  'stats.capability.limit.analyze.crosstabs_two_columns_only': 'Crosstabs takes two columns; a third, layer column is not offered.',
  'stats.capability.limit.analyze.fisher_two_by_two_only':
    "Fisher's exact test is given for a table of two rows and two columns only; no exact test is given for a larger table.",
  'stats.capability.limit.analyze.no_further_nominal_measures':
    "Lambda, Goodman and Kruskal's tau, the uncertainty coefficient, the linear-by-linear test and the measures for ordered levels are not given.",
  'stats.capability.limit.analyze.no_weights': 'Each row is counted once; weighting rows by a column is not offered.',
  'stats.capability.limit.analyze.levels_in_code_point_order':
    "Levels are listed in the order Gaply reads them, so a column of numbers lists 10 before 2; the sign of phi and the direction of Fisher's one-sided tests follow that order.",
  // Phase 8Y-1 (decisions 602, 605 and 606): the Mann-Whitney and
  // Kruskal-Wallis tests, Planned in Part 2 and Beta from Part 3 (decision 607).
  'stats.capability.analyze.independent_ranks.title': 'Mann-Whitney and Kruskal-Wallis tests on your data',
  'stats.capability.analyze.independent_ranks.description':
    "Compares a column of your own data file across two or more groups by the ranks of its values: the Mann-Whitney U test when two groups are included, with an exact p value for small groups without ties, and the Kruskal-Wallis H test when three or more are, with each group's count, median and mean rank, the rank-biserial correlation and epsilon-squared.",
  'stats.capability.limit.analyze.no_pairwise_rank_comparisons':
    "Pairwise comparisons after a Kruskal-Wallis test, such as Dunn's test, are not given.",
  'stats.capability.limit.analyze.rank_tests_two_sided_only': 'Each Mann-Whitney p value is two-sided; one-sided tests are not offered.',
  'stats.capability.limit.analyze.no_location_shift_estimate':
    'No estimate of the shift between two groups, such as the Hodges-Lehmann estimate, and no interval for it, is given.',
  'stats.capability.limit.analyze.exact_p_without_ties_only':
    "The Mann-Whitney test's exact p value is given only when no two values are tied and each group has fewer than 50 values; otherwise only the normal approximation's p value is given.",
  'stats.capability.limit.analyze.no_further_k_sample_tests': 'The median test and the Jonckheere-Terpstra test are not given.',
  'stats.capability.limit.analyze.rank_groups_in_code_point_order':
    'Groups are listed in the order Gaply reads them, so a group column of numbers lists 10 before 2; which group is first, and the signs of the Mann-Whitney z and of the rank-biserial correlation, follow that order.',
  // Phase 8Z-1 (decisions 636, 639 and 640): the Wilcoxon signed-rank and Friedman tests,
  // Planned in Part 2 and Beta from Part 3 (decision 641).
  'stats.capability.analyze.related_ranks.title':
    'Wilcoxon signed-rank and Friedman tests on your data',
  'stats.capability.analyze.related_ranks.description':
    "Compares two or more columns of your own data file, measured on the same rows, by the ranks of their values: the Wilcoxon signed-rank test when two columns are chosen, with an exact p value for small samples without ties or zero differences, and the Friedman test when three or more are, with each column's median and mean rank, the matched-pairs rank-biserial correlation and Kendall's W.",
  'stats.capability.limit.analyze.no_pairwise_related_comparisons':
    'Pairwise comparisons after a Friedman test are not given.',
  'stats.capability.limit.analyze.related_tests_two_sided_only':
    'Each Wilcoxon p value is two-sided; one-sided tests are not offered.',
  'stats.capability.limit.analyze.no_differences_location_estimate':
    'No estimate of the typical difference between two columns, such as the Hodges-Lehmann estimate, and no interval for it, is given.',
  'stats.capability.limit.analyze.signed_rank_exact_p_rule':
    "The Wilcoxon test's exact p value is given only when no two differences are tied in size, none is zero and there are fewer than 50 pairs; otherwise only the normal approximation's p value is given.",
  'stats.capability.limit.analyze.zero_differences_dropped':
    "A pair whose two values are equal is left out of the Wilcoxon test; Pratt's way of keeping such pairs is not offered.",
  'stats.capability.limit.analyze.no_further_related_tests':
    "The sign test, the marginal homogeneity test and Cochran's Q test are not given.",
  'stats.capability.limit.analyze.related_columns_in_chosen_order':
    'Columns are taken in the order you choose them: the Wilcoxon differences are the first column less the second, and the signs of z and of the matched-pairs rank-biserial correlation follow that order.',
  // Phase 9A-1 (decisions 669, 672 and 673): the linear regression, Planned in Part 2
  // and Beta from Part 3 (decision 674).
  'stats.capability.analyze.linear_regression.title': 'Linear regression on your data',
  'stats.capability.analyze.linear_regression.description':
    'Fits an ordinary least squares regression of one numeric column of your own data file on one or more others, entered together, with the model summary, the analysis of variance, each coefficient with its standard error, t, p and 95% confidence interval, the standardized coefficients, the zero-order, partial and part correlations, tolerance and VIF, the Durbin-Watson statistic and a summary of the residuals.',
  'stats.capability.limit.analyze.regression_enter_only':
    'Every chosen predictor is entered at once; stepwise, forward and backward methods, and blocks of predictors, are not offered.',
  'stats.capability.limit.analyze.regression_no_categorical_predictors':
    'Each predictor is taken as a number; a categorical predictor is not coded into indicator columns for you.',
  'stats.capability.limit.analyze.regression_no_interaction_terms':
    'No interaction or polynomial terms are formed; only the columns you choose enter the model.',
  'stats.capability.limit.analyze.regression_intercept_always':
    'The model always has an intercept; regression through the origin is not offered.',
  'stats.capability.limit.analyze.regression_listwise_only':
    'A row is used only when the outcome and every predictor have a value; pairwise deletion and mean substitution are not offered.',
  'stats.capability.limit.analyze.regression_no_robust_errors':
    'Standard errors assume the residuals have equal variance; heteroscedasticity-consistent standard errors are not given.',
  'stats.capability.limit.analyze.regression_no_casewise_diagnostics':
    'Casewise diagnostics, residual plots, influence statistics, and predicted values or residuals saved as columns are not given.',
  'stats.capability.limit.analyze.durbin_watson_row_order':
    "The Durbin-Watson statistic takes the rows in the file's order, so it describes serial correlation only when that order means something, such as time.",
  'stats.capability.limit.analyze.regression_intervals_95_only': 'Confidence intervals are at 95%; other levels are not offered.',
  // Phase 8M (decision 411): scale scores, Planned in 8M and Beta since 8M-2's
  // Part 2 (decision 416), with no route yet.
  'stats.capability.prepare.scale_score.title': 'Scale scores',
  'stats.capability.prepare.scale_score.description':
    "A new numeric column that is the sum or the mean of items you choose, any of them reverse-scored on the scale's ends you type, kept as a recipe. Your file is never changed.",
  'stats.capability.limit.prepare.items_are_file_columns': "A score's items are the file's columns; a score of other scores is not offered.",
  'stats.capability.limit.prepare.no_recoding_formula_or_selection': 'Recoding values, computing from a formula and selecting cases are not offered yet.',
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
  noRoute: 'This runs in Gaply, but has no screen of its own here yet.',
  unread: 'The list of methods could not be read.',
};
