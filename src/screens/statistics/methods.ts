// Gaply — the methods on the researcher's own data, as one table (Research
// OS, Phase 8C; decision 249). Pure data, one row per method: the run
// manager's word for its runs (Rust's RunKind), its capability id, its name,
// the shell's job name (none for the summary, whose sentence has no
// parenthesis), its screen's route (null for crosstabs, the two rank tests and the linear regression, which have none), and its place in the Statistics window's
// Analyze menu (Research OS, Phase 8P, decision 451).
//
// RunKind, the capability names, the shell's job names and the Statistics
// home's routes are read from here. Each method's bridge, stored-result
// checker and saved-result renderer stay with the code that uses them,
// keyed by the same kind or capability id. A new method adds one row.
import { DESCRIBE_PATH } from './describeRoute';
import { ALPHA_PATH, ANOVA_PATH, CORR_PATH, ONE_SAMPLE_PATH, PAIRED_PATH, TTEST_PATH } from './statisticsRoute';

export const METHODS = [
  {
    kind: 'describe',
    capabilityId: 'stats.describe',
    name: 'Descriptive summary',
    jobName: null,
    path: DESCRIBE_PATH,
    menuGroup: 'Descriptive statistics',
    menuLabel: 'Summary…',
  },
  {
    kind: 'ttest',
    capabilityId: 'stats.analyze.independent_t',
    name: 'Independent-samples t-test',
    jobName: 'independent-samples t-test',
    path: TTEST_PATH,
    menuGroup: 'Compare means',
    menuLabel: 'Independent-samples t-test…',
  },
  // Research OS, Phase 8D-2b (decision 268).
  {
    kind: 'one_sample',
    capabilityId: 'stats.analyze.one_sample_t',
    name: 'One-sample t-test',
    jobName: 'one-sample t-test',
    path: ONE_SAMPLE_PATH,
    menuGroup: 'Compare means',
    menuLabel: 'One-sample t-test…',
  },
  // Research OS, Phase 8D-2c (decision 271).
  {
    kind: 'paired',
    capabilityId: 'stats.analyze.paired_t',
    name: 'Paired-samples t-test',
    jobName: 'paired-samples t-test',
    path: PAIRED_PATH,
    menuGroup: 'Compare means',
    menuLabel: 'Paired t-test…',
  },
  // Research OS, Phase 8G-2 (decision 331).
  {
    kind: 'anova',
    capabilityId: 'stats.analyze.one_way_anova',
    name: 'One-way ANOVA',
    jobName: 'one-way ANOVA',
    path: ANOVA_PATH,
    menuGroup: 'Compare means',
    menuLabel: 'One-way ANOVA…',
  },
  // Research OS, Phase 8H-2 (decision 359). The kind is Rust's RunKind word.
  {
    kind: 'correlations',
    capabilityId: 'stats.analyze.correlations',
    name: 'Correlations',
    jobName: 'correlations',
    path: CORR_PATH,
    menuGroup: 'Correlate',
    menuLabel: 'Correlations…',
  },
  // Research OS, Phase 9A-2 (decisions 685 and 687). No page and no route
  // (ADR-012:233-241). After the correlations' row, so Analyze's groups read in
  // SPSS's order, Correlate then Regression.
  {
    kind: 'linear_regression',
    capabilityId: 'stats.analyze.linear_regression',
    name: 'Linear regression',
    jobName: 'linear regression',
    path: null,
    menuGroup: 'Regression',
    menuLabel: 'Linear…',
  },
  // Research OS, Phase 8I-2 (decision 384). The kind is Rust's RunKind word.
  {
    kind: 'cronbach_alpha',
    capabilityId: 'stats.analyze.cronbach_alpha',
    name: "Cronbach's alpha",
    jobName: "Cronbach's alpha",
    path: ALPHA_PATH,
    menuGroup: 'Scale',
    menuLabel: "Reliability (Cronbach's alpha)…",
  },
  // Research OS, Phase 8X-2 (decision 591). The kind is Rust's RunKind word. No
  // page and no route: ADR-012 retires the old pages (:233-241, :737), so the
  // window's dialog is its one screen, and the old home shows the card a method
  // with no page shows. In Analyze's Descriptive statistics group, after
  // Summary…, where SPSS has it.
  {
    kind: 'crosstabs',
    capabilityId: 'stats.analyze.crosstabs',
    name: 'Crosstabs',
    jobName: 'crosstabs',
    path: null,
    menuGroup: 'Descriptive statistics',
    menuLabel: 'Crosstabs…',
  },
  // Research OS, Phase 8Y-2 (decisions 620, 622 and 624). The kind is Rust's RunKind
  // word. No page and no route (ADR-012:233-241), as crosstabs. A new Analyze group,
  // after Scale, as SPSS places Nonparametric Tests.
  {
    kind: 'independent_ranks',
    capabilityId: 'stats.analyze.independent_ranks',
    name: 'Mann-Whitney and Kruskal-Wallis',
    jobName: 'rank test',
    path: null,
    menuGroup: 'Nonparametric tests',
    menuLabel: 'Independent samples…',
  },
  // Research OS, Phase 8Z-2 (decisions 654, 656 and 659). No page and no route
  // (ADR-012:233-241); analyzeGroups puts it in the group the rank tests opened.
  {
    kind: 'related_ranks',
    capabilityId: 'stats.analyze.related_ranks',
    name: 'Wilcoxon signed-rank and Friedman',
    jobName: 'related-samples rank test',
    path: null,
    menuGroup: 'Nonparametric tests',
    menuLabel: 'Related samples…',
  },
] as const;

export type Method = (typeof METHODS)[number];

/** Which kind of Statistics run a record holds: Rust's RunKind words. */
export type MethodKind = Method['kind'];

/** The row of a capability, or undefined for one no method has. */
export function methodFor(capabilityId: string): Method | undefined {
  for (const m of METHODS) if (m.capabilityId === capabilityId) return m;
  return undefined;
}

/** The capability id of a run kind's method; '' if no row has the kind,
 *  so a table missing a row fails a test rather than a module's load. */
export function capabilityOfKind(kind: MethodKind): string {
  for (const m of METHODS) if (m.kind === kind) return m.capabilityId;
  return '';
}

/** One value per run kind, read from each row: the shape the shell's and
 *  the project page's per-kind lookups take. */
export function byKind<T>(value: (m: Method) => T): Record<MethodKind, T> {
  const out = {} as Record<MethodKind, T>;
  for (const m of METHODS) out[m.kind] = value(m);
  return out;
}
