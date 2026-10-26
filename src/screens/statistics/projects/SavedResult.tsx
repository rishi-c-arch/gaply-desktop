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
// * Rendering sits inside an error boundary keyed to the result, whose
//   fallback is one fixed sentence; it logs nothing. The check runs before
//   rendering, so rendering cannot throw on stored data.
import React, { useEffect, useState } from 'react';
import { isFeatureEnabled } from '../../../config/featureFlags';
import { DescribeResult } from '../DescribeFlow';
import { ResultCard } from '../TTestFlow';
import { diagnosticSentence } from '../describeWording';
import { ttestSentence, type TTestNames } from '../ttestWording';
import type { Diagnostic } from '../describeTypes';
import type { TTestDiagnostic, TTestResultView } from '../ttestTypes';
import type { ProjectBridge } from './projectBridge';
import type { StoredRunView } from './projectTypes';
import { capabilityName, PROJECT_TEXT, projectSentence, rerunLines, SAVED_TEXT, savedResultLine } from './projectWording';
import { capabilityOfCase, checkStored, SUMMARY_CAPABILITY, TTEST_CAPABILITY, type Checked } from './storedShape';
import type { Generation } from './useGeneration';

/** A column by its position: the stored result holds no headers for its
 *  diagnostics, and the data file is never read back. */
const BY_POSITION = (alias: number) => `C${alias}`;

/** The t-test's names: columns by position, levels by the result's own two
 *  groups, as the live screen names them from the same labels. */
function ttestNames(result: TTestResultView): TTestNames {
  return {
    column: BY_POSITION,
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
  failedSentence: (failure: Diagnostic | TTestDiagnostic) => string;
  result: (checked: Checked) => React.ReactNode;
  hidden: () => boolean;
}

/** The renderers, keyed by capability id. A new method adds one entry. */
export const SAVED_RENDERERS: Record<string, SavedRenderer> = {
  [SUMMARY_CAPABILITY]: {
    failedSentence: (failure) => diagnosticSentence(failure as Diagnostic, BY_POSITION),
    result: (checked) => (checked.kind === 'summary' ? <DescribeResult result={checked.result} name={BY_POSITION} /> : null),
    // Only summaries sit behind a flag: REACT_APP_FEATURE_STATS_DESCRIBE.
    hidden: () => !isFeatureEnabled('statsDescribe'),
  },
  [TTEST_CAPABILITY]: {
    failedSentence: (failure) => ttestSentence(failure as TTestDiagnostic),
    result: (checked) => (checked.kind === 'ttest' ? <ResultCard result={checked.result} names={ttestNames(checked.result)} /> : null),
    hidden: () => false,
  },
};

/** A capability's renderer, or undefined for one no method has. An own key
 *  only, never one from Object.prototype. */
function rendererFor(capabilityId: string): SavedRenderer | undefined {
  return Object.prototype.hasOwnProperty.call(SAVED_RENDERERS, capabilityId) ? SAVED_RENDERERS[capabilityId] : undefined;
}

/** What a checked result shows. Exported for the page and its tests. */
export const SavedResultBody: React.FC<{ checked: Checked }> = ({ checked }) => {
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
            {rendererFor(capabilityOfCase(checked.capability))?.failedSentence(checked.failure)}
          </p>
        </>
      );
    // A method's own case: drawn by its renderer, so a new method adds no
    // case here (Phase 8C).
    default:
      return (
        <>
          <p data-testid="saved-data-file">{SAVED_TEXT.dataFile(checked.dataFile.format, checked.dataFile.bytes)}</p>
          {rendererFor(capabilityOfCase(checked.kind))?.result(checked)}
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
}

type Shown = { kind: 'loading' } | { kind: 'refused'; sentence: string } | { kind: 'hidden' } | { kind: 'checked'; checked: Checked };

export const SavedResult: React.FC<SavedResultProps> = ({ projectId, run, repeated = null, bridge, gen }) => {
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
          <SavedResultBody checked={shown.checked} />
        </SavedResultBoundary>
      )}
    </section>
  );
};

export default SavedResult;
