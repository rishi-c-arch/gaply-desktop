// Gaply — one saved result, read only (Research OS, Phase 5B-3b; decision
// 150). On the project page, under a "Saved result" line with its saved date.
//
// * It reads project_run_result when it appears (a read; allowed from an
//   effect), under the page's generation counter.
// * The stored JSON is checked first (storedShape.ts). Only a result whose
//   capability and schema version this app knows, and whose every field the
//   components read has its shape, is rendered, through the live screens'
//   own components: DescribeResult and ResultCard, with the props the live
//   screens give them. Each other case shows its own sentence and nothing of
//   the stored JSON: an unknown capability, an unknown schema version, a
//   wrong shape, a damaged artifact (its code's sentence), and a saved
//   summary while REACT_APP_FEATURE_STATS_DESCRIBE, the summary's flag, is
//   off.
// * A failed run shows its code's sentence.
// * Its data file appears only as its kind, format and size.
// * No Save, re-run, edit or AI-explanation control.
// * Phase 5C-1b (decision 197): a rerun says, under its "Saved result" line,
//   which saved result it repeats and how its result compares, in the same
//   words as its line in the list (RerunWords, drawn here for both).
// * Phase 8U-3 (decision 554): given the run's scores, a column whose
//   position is a score's is named C{n} "name", found by its position and
//   never by a header's text; with none, every column is named as before.
// * Rendering sits inside an error boundary keyed to the result, whose
//   fallback is one fixed sentence; it logs nothing. The check runs before
//   rendering, so rendering cannot throw on stored data.
import React, { useEffect, useState } from 'react';
import { isFeatureEnabled } from '../../../config/featureFlags';
import { DescribeResult } from '../DescribeFlow';
import { ResultCard } from '../TTestFlow';
import { OneSampleResultCard } from '../OneSampleFlow';
import { PairedResultCard } from '../PairedFlow';
import { AnovaResultCard } from '../AnovaFlow';
import { anovaSentence, type AnovaNames } from '../anovaWording';
import type { AnovaDiagnostic, AnovaResultView } from '../anovaTypes';
import { CorrResultCard } from '../CorrFlow';
import { corrSentence } from '../corrWording';
import type { CorrDiagnostic } from '../corrTypes';
import { AlphaResultCard } from '../AlphaFlow';
import { alphaSentence } from '../alphaWording';
import type { AlphaDiagnostic } from '../alphaTypes';
import { XtabCard } from '../XtabCard';
import { xtabSentence, type XtabNames } from '../xtabWording';
import type { XtabDiagnostic } from '../xtabTypes';
import { RanksCard } from '../RanksCard';
import { ranksSentence, type RanksNames } from '../ranksWording';
import type { RanksDiagnostic, RanksResultView } from '../ranksTypes';
import { RelatedRanksCard } from '../RelatedRanksCard';
import { relatedRanksSentence } from '../relatedRanksWording';
import type { RelatedRanksDiagnostic } from '../relatedRanksTypes';
import { diagnosticSentence } from '../describeWording';
import { ttestSentence, type TTestNames } from '../ttestWording';
import { oneSampleSentence } from '../onesampleWording';
import type { OneSampleDiagnostic } from '../onesampleTypes';
import type { Diagnostic } from '../describeTypes';
import type { TTestDiagnostic, TTestResultView } from '../ttestTypes';
import type { ProjectBridge } from './projectBridge';
import type { ScoreView, StoredRunView } from './projectTypes';
import { capabilityName, PROJECT_TEXT, projectSentence, rerunLines, SAVED_TEXT, savedResultLine, scoreColumn } from './projectWording';
import {
  ANOVA_CAPABILITY,
  capabilityOfCase,
  CORR_CAPABILITY,
  ALPHA_CAPABILITY,
  checkStored,
  ONE_SAMPLE_CAPABILITY,
  PAIRED_CAPABILITY,
  RANKS_CAPABILITY, RELATED_RANKS_CAPABILITY,
  SUMMARY_CAPABILITY,
  TTEST_CAPABILITY,
  XTAB_CAPABILITY,
  type Checked,
  type StoredFailure,
} from './storedShape';
import type { Generation } from './useGeneration';

/** A column by its position: the stored result holds no headers for its
 *  diagnostics, and the data file is never read back. */
const BY_POSITION = (alias: number) => `C${alias}`;

/** How a saved result names its columns (Research OS, Phase 8U-3, decision
 *  554): `column` for a column by its position, and `row` for a summary's row
 *  by its alias, which gives a label only for a score. */
export interface SavedNaming {
  column: (alias: number) => string;
  row?: (alias: string) => string | null;
}

/** Every column by its position: a run with no scores. */
const BY_POSITION_ONLY: SavedNaming = { column: BY_POSITION };

/** A row's alias as its position: `C` and a number from 1, as Level 1 writes
 *  it; anything else is no position. */
const positionOf = (alias: string): number | null => (/^C[1-9][0-9]*$/.test(alias) ? Number(alias.slice(1)) : null);

/** A run's naming from its scores: a column whose position is a score's is
 *  named C{n} "name", found by that position and never by a header's text;
 *  every other column by its position, as before. No scores, today's. */
export function savedNaming(scores?: ScoreView[] | null): SavedNaming {
  if (!scores || scores.length === 0) return BY_POSITION_ONLY;
  const at = (position: number | null) => (position === null ? undefined : scores.find((s) => s.position === position));
  return {
    column: (alias) => {
      const s = at(alias);
      return s ? scoreColumn(alias, s.name) : BY_POSITION(alias);
    },
    row: (alias) => {
      const position = positionOf(alias);
      const s = at(position);
      return s && position !== null ? scoreColumn(position, s.name) : null;
    },
  };
}

/** The t-test's names: columns by position, levels by the result's own two
 *  groups, as the live screen names them from the same labels. */
export function ttestNames(result: TTestResultView, column: (alias: number) => string = BY_POSITION): TTestNames {
  return {
    column,
    level: (index: number) => result.result.groups.find((g) => g.level === index)?.label ?? `level ${index + 1}`,
  };
}

/** The one-way ANOVA's names: columns by position, levels by the result's
 *  own groups, as the live screen names them from the same labels. */
export function anovaNames(result: AnovaResultView, column: (alias: number) => string = BY_POSITION): AnovaNames {
  return {
    column,
    level: (index: number) => result.result.groups.find((g) => g.level === index)?.label ?? `level ${index + 1}`,
  };
}

/** Crosstabs' names (Research OS, Phase 8X-2, decision 591), on anovaNames':
 *  columns by position, or a score's C{n} "name" given the run's scores
 *  (decision 554), which for crosstabs never names a column, since it refuses a
 *  score. No level is named in a sentence: its codes carry none, and the card
 *  draws each level's label from the result as text. */
export function xtabNames(column: (alias: number) => string = BY_POSITION): XtabNames {
  return { column };
}

/** The rank tests' names (Research OS, Phase 8Y-2, decision 622), on anovaNames':
 *  columns by position, or a score's C{n} "name" given the run's scores (decision
 *  554), levels by the result's own groups. */
export function ranksNames(result: RanksResultView, column: (alias: number) => string = BY_POSITION): RanksNames {
  return {
    column,
    level: (index: number) => result.result.groups.find((g) => g.level === index)?.label ?? `level ${index + 1}`,
  };
}

interface BoundaryProps {
  children: React.ReactNode;
}

/** Shows one fixed sentence if rendering throws. It records nothing: no
 *  componentDidCatch, so this file never reads the error. */
export class SavedResultBoundary extends React.Component<BoundaryProps, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }

  render() {
    if (this.state.failed) return <p data-testid="saved-result-fallback">{SAVED_TEXT.cannotShow}</p>;
    return this.props.children;
  }
}

/** How one method's saved result is drawn (Phase 8C; decision 249): its
 *  failure's sentence, its result through the live screen's own component,
 *  and whether a flag hides its saved results. */
interface SavedRenderer {
  failedSentence: (failure: StoredFailure, naming?: SavedNaming) => string;
  result: (checked: Checked, naming?: SavedNaming) => React.ReactNode;
  hidden: () => boolean;
}

/** The renderers, keyed by capability id. A new method adds one entry. */
export const SAVED_RENDERERS: Record<string, SavedRenderer> = {
  [SUMMARY_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) => diagnosticSentence(failure as Diagnostic, n.column),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'summary' ? <DescribeResult result={checked.result} name={n.column} rowLabel={n.row} /> : null,
    // Only summaries sit behind a flag: REACT_APP_FEATURE_STATS_DESCRIBE.
    hidden: () => !isFeatureEnabled('statsDescribe'),
  },
  [TTEST_CAPABILITY]: {
    failedSentence: (failure) => ttestSentence(failure as TTestDiagnostic),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'ttest' ? <ResultCard result={checked.result} names={ttestNames(checked.result, n.column)} /> : null,
    hidden: () => false,
  },
  // Phase 8D-2b (decision 269): columns by position, as the t-test's are;
  // not behind a flag in a saved result.
  [ONE_SAMPLE_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) => oneSampleSentence(failure as OneSampleDiagnostic, { column: n.column }),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'one_sample' ? <OneSampleResultCard result={checked.result} names={{ column: n.column }} /> : null,
    hidden: () => false,
  },
  // Phase 8D-2c (decision 272): as the one-sample test's; its card names the
  // columns by role, so the saved card reads as the live one.
  [PAIRED_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) => oneSampleSentence(failure as OneSampleDiagnostic, { column: n.column }),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'paired' ? <PairedResultCard result={checked.result} names={{ column: n.column }} /> : null,
    hidden: () => false,
  },
  // Phase 8G-2 (decision 330): through the live screen's own card, its
  // levels named by the result's labels; not behind a flag.
  [ANOVA_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) =>
      anovaSentence(failure as AnovaDiagnostic, { column: n.column, level: (i) => `level ${i + 1}` }),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'anova' ? <AnovaResultCard result={checked.result} names={anovaNames(checked.result, n.column)} /> : null,
    hidden: () => false,
  },
  // Phase 8H-2 (decision 359): through the live screen's own card, columns
  // by position (C1, C2, …), as the result itself names them; not behind a
  // flag.
  [CORR_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) => corrSentence(failure as CorrDiagnostic, { column: n.column }),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'correlations' ? <CorrResultCard result={checked.result} names={{ column: n.column }} /> : null,
    hidden: () => false,
  },
  // Phase 8I-2 (decision 384): through the live screen's own card, items by
  // position (C1, C2, …), as the result itself names them; not behind a
  // flag.
  [ALPHA_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) => alphaSentence(failure as AlphaDiagnostic, { column: n.column }),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'cronbach_alpha' ? <AlphaResultCard result={checked.result} names={{ column: n.column }} /> : null,
    hidden: () => false,
  },
  // Research OS, Phase 8X-2 (decision 591): through the Output's own card,
  // XtabCard, columns by position (C1, C2, …), levels by the result's own
  // labels as text; not behind a flag.
  [XTAB_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) => xtabSentence(failure as XtabDiagnostic, xtabNames(n.column)),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'crosstabs' ? <XtabCard result={checked.result} names={xtabNames(n.column)} /> : null,
    hidden: () => false,
  },
  // Research OS, Phase 8Y-2 (decision 622): through the Output's own card, RanksCard,
  // its names as the ANOVA's are; not behind a flag.
  [RANKS_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) =>
      ranksSentence(failure as RanksDiagnostic, { column: n.column, level: (i) => `level ${i + 1}` }),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'independent_ranks' ? <RanksCard result={checked.result} names={ranksNames(checked.result, n.column)} /> : null,
    hidden: () => false,
  },
  // Research OS, Phase 8Z-2 (decision 656): through the Output's own card, a score's name
  // through savedNaming, as the rank tests' outcome is; not behind a flag.
  [RELATED_RANKS_CAPABILITY]: {
    failedSentence: (failure, n = BY_POSITION_ONLY) => relatedRanksSentence(failure as RelatedRanksDiagnostic, { column: n.column }),
    result: (checked, n = BY_POSITION_ONLY) =>
      checked.kind === 'related_ranks' ? <RelatedRanksCard result={checked.result} names={{ column: n.column }} /> : null,
    hidden: () => false,
  },
};

/** A capability's renderer, or undefined for one no method has. An own key
 *  only, never one from Object.prototype. */
function rendererFor(capabilityId: string): SavedRenderer | undefined {
  return Object.prototype.hasOwnProperty.call(SAVED_RENDERERS, capabilityId) ? SAVED_RENDERERS[capabilityId] : undefined;
}

/** What a checked result shows. Exported for the page and its tests. */
export const SavedResultBody: React.FC<{ checked: Checked; scores?: ScoreView[] | null }> = ({ checked, scores }) => {
  const naming = savedNaming(scores);
  switch (checked.kind) {
    case 'unknownCapability':
      return <p data-testid="saved-result-unknown-capability">{SAVED_TEXT.unknownCapability}</p>;
    case 'unknownSchema':
      return <p data-testid="saved-result-unknown-schema">{SAVED_TEXT.unknownSchema}</p>;
    case 'badShape':
      return <p data-testid="saved-result-bad-shape">{SAVED_TEXT.badShape}</p>;
    case 'failed':
      return (
        <>
          <p data-testid="saved-data-file">{SAVED_TEXT.dataFile(checked.dataFile.format, checked.dataFile.bytes)}</p>
          <p data-testid="saved-result-failed">
            {SAVED_TEXT.failed}{' '}
            {rendererFor(capabilityOfCase(checked.capability))?.failedSentence(checked.failure, naming)}
          </p>
        </>
      );
    // A method's own case: drawn by its renderer, so a new method adds no
    // case here (Phase 8C).
    default:
      return (
        <>
          <p data-testid="saved-data-file">{SAVED_TEXT.dataFile(checked.dataFile.format, checked.dataFile.bytes)}</p>
          {rendererFor(capabilityOfCase(checked.kind))?.result(checked, naming)}
        </>
      );
  }
};

/** A rerun's words, under its own line (decisions 191, 197 and 198): the
 *  saved result it repeats, then how its result compares. Nothing for a run
 *  that is not a rerun. The one place they are drawn, in the project's list
 *  and in the saved result view alike. */
export const RerunWords: React.FC<{ run: StoredRunView; repeated: StoredRunView | null; testId: string }> = ({
  run,
  repeated,
  testId,
}) => {
  const lines = rerunLines(run, repeated);
  if (lines.length === 0) return null;
  return (
    <p data-testid={testId} style={{ margin: 0 }}>
      {lines.join(' ')}
    </p>
  );
};

export interface SavedResultProps {
  projectId: string;
  run: StoredRunView;
  /** For a rerun, the saved run it repeats, from the page's list, or null. */
  repeated?: StoredRunView | null;
  bridge: ProjectBridge;
  /** The page's counter: this view is part of the page. */
  gen: Generation;
  /** The run's scores, from the page's read of the project's recipes (8U-3,
   *  decision 554): its score columns are named by them. None: by position. */
  scores?: ScoreView[] | null;
}

type Shown = { kind: 'loading' } | { kind: 'refused'; sentence: string } | { kind: 'hidden' } | { kind: 'checked'; checked: Checked };

export const SavedResult: React.FC<SavedResultProps> = ({ projectId, run, repeated = null, bridge, gen, scores = null }) => {
  const [shown, setShown] = useState<Shown>({ kind: 'loading' });

  useEffect(() => {
    // The summary's flag first: off, and nothing of the stored result is read.
    if (rendererFor(run.capabilityId)?.hidden()) {
      setShown({ kind: 'hidden' });
      return;
    }
    const g = gen.next();
    setShown({ kind: 'loading' });
    bridge.runResult(projectId, run.storedRunId).then(
      (view) => {
        if (gen.is(g)) setShown({ kind: 'checked', checked: checkStored(view) });
      },
      (e) => {
        if (gen.is(g)) setShown({ kind: 'refused', sentence: projectSentence(e) });
      }
    );
  }, [projectId, run.storedRunId, run.capabilityId, bridge, gen]);

  return (
    <section aria-label={PROJECT_TEXT.savedResults} data-testid="saved-result" style={{ display: 'grid', gap: 8 }}>
      <p data-testid="saved-result-line" style={{ margin: 0, fontWeight: 600 }}>
        {savedResultLine(run.savedAt)}
      </p>
      <RerunWords run={run} repeated={repeated} testId="saved-result-repeats" />
      <p data-testid="saved-result-kind" style={{ margin: 0 }}>
        {capabilityName(run.capabilityId)}
      </p>
      {shown.kind === 'loading' && <p data-testid="saved-result-loading">{PROJECT_TEXT.resultLoading}</p>}
      {shown.kind === 'hidden' && <p data-testid="saved-result-hidden">{SAVED_TEXT.summaryHidden}</p>}
      {shown.kind === 'refused' && (
        <p role="alert" data-testid="saved-result-refused">
          {shown.sentence}
        </p>
      )}
      {shown.kind === 'checked' && (
        <SavedResultBoundary key={`${projectId}:${run.storedRunId}`}>
          <SavedResultBody checked={shown.checked} scores={scores} />
        </SavedResultBoundary>
      )}
    </section>
  );
};

export default SavedResult;
