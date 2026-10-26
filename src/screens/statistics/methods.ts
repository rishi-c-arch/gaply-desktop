// Gaply — the methods on the researcher's own data, as one table (Research
// OS, Phase 8C; decision 249). Pure data, one row per method: the run
// manager's word for its runs (Rust's RunKind), its capability id, its name,
// the shell's job name (none for the summary, whose sentence has no
// parenthesis) and its screen's route.
//
// RunKind, the capability names, the shell's job names and the Statistics
// home's routes are read from here. Each method's bridge, stored-result
// checker and saved-result renderer stay with the code that uses them,
// keyed by the same kind or capability id. A new method adds one row.
import { DESCRIBE_PATH } from './describeRoute';
import { TTEST_PATH } from './statisticsRoute';

export const METHODS = [
  {
    kind: 'describe',
    capabilityId: 'stats.describe',
    name: 'Descriptive summary',
    jobName: null,
    path: DESCRIBE_PATH,
  },
  {
    kind: 'ttest',
    capabilityId: 'stats.analyze.independent_t',
    name: 'Independent-samples t-test',
    jobName: 'independent-samples t-test',
    path: TTEST_PATH,
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
