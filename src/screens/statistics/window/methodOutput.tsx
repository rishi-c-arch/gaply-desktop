// Gaply — the Output for every method (Research OS, Phase 8S, decision 479; ADR-012
// Part 7): one table keyed by the method's kind, so an entry can hold any method's
// run and result. A row names its bridge in the window's RunBridges and uses only
// its status, result, cancel and export: an entry owns no file. Each result card is
// the one its page and a saved result already render. A column is named from the
// entry's own view; a level from the result's own labels.
//
// A missing kind fails tsc (the mapped type below) and a test (methods.vitest.ts).
import React from 'react';
import { AlphaResultCard } from '../AlphaFlow';
import { ALPHA_EXPORT_CHOICES, ALPHA_EXPORT_TEXT, ALPHA_STAGE_HEADINGS, alphaRefusalSentence, alphaRunEndedSentence, alphaSentence, stepsSentence as alphaSteps } from '../alphaWording';
import { AnovaResultCard } from '../AnovaFlow';
import { ANOVA_EXPORT_CHOICES, ANOVA_EXPORT_TEXT, ANOVA_STAGE_HEADINGS, anovaRefusalSentence, anovaRunEndedSentence, anovaSentence, stepsSentence as anovaSteps } from '../anovaWording';
import { CorrResultCard } from '../CorrFlow';
import { CORR_EXPORT_CHOICES, CORR_EXPORT_TEXT, CORR_STAGE_HEADINGS, corrRefusalSentence, corrRunEndedSentence, corrSentence, stepsSentence as corrSteps } from '../corrWording';
import { DescribeResult } from '../DescribeFlow';
import type { DatasetView, ExportFormat, ExportView, RunState } from '../describeTypes';
import { diagnosticSentence, EXPORT_CHOICES, EXPORT_TEXT, progressSentence, refusalSentence, runEndedSentence, STAGE_HEADINGS } from '../describeWording';
import type { MethodKind } from '../methods';
import { OneSampleResultCard } from '../OneSampleFlow';
import { ONE_SAMPLE_EXPORT_CHOICES, ONE_SAMPLE_EXPORT_TEXT, ONE_SAMPLE_STAGE_HEADINGS, oneSampleRefusalSentence, oneSampleRunEndedSentence, oneSampleSentence, PAIRED_EXPORT_CHOICES, PAIRED_EXPORT_TEXT, PAIRED_STAGE_HEADINGS } from '../onesampleWording';
import { PairedResultCard } from '../PairedFlow';
import { anovaNames, ttestNames } from '../projects/SavedResult';
import { ResultCard } from '../TTestFlow';
import { TTEST_EXPORT_CHOICES, TTEST_EXPORT_TEXT, TTEST_STAGE_HEADINGS, ttestRefusalSentence, ttestRunEndedSentence, ttestSentence } from '../ttestWording';
import type { RunBridges } from '../../workspace/useWorkspaceSwitch';
import type { MethodResults, MethodRuns } from './statisticsSession';
import { AlphaDialog } from './AlphaDialog';
import { AnovaDialog } from './AnovaDialog';
import { CorrDialog } from './CorrDialog';
import { DescribeDialog } from './DescribeDialog';
import { OneSampleDialog } from './OneSampleDialog';
import { PairedDialog } from './PairedDialog';
import { TTestDialog } from './TTestDialog';
import { columnNames } from './windowWording';

/** What an entry may do with its run: no open, no close, no validate, no run. */
export interface EntryBridge<K extends MethodKind = MethodKind> {
  status(runId: number): Promise<MethodRuns[K]>;
  result(runId: number): Promise<MethodResults[K]>;
  cancel(runId: number): Promise<MethodRuns[K]>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
}

export interface OutputRow<K extends MethodKind = MethodKind> {
  bridge: (bridges: RunBridges) => EntryBridge<K>;
  refusal: (e: unknown) => string;
  runEnded: (state: RunState) => string;
  /** Its progress in what it counts; null for a method with no check points. */
  progress: (run: MethodRuns[K]) => string | null;
  /** A diagnostic of its run, its columns named from the entry's view. */
  sentence: (d: MethodRuns[K]['diagnostics'][number], view: DatasetView) => string;
  card: (result: MethodResults[K], view: DatasetView) => React.ReactNode;
  exportHeading: string;
  exportText: string;
  exportChoices: ReadonlyArray<{ format: ExportFormat; label: string; file: string; mime: string }>;
}

/** Before there is a result, a level has only its position, as a saved failure's has. */
const levelByPosition = (index: number) => `level ${index + 1}`;

export const METHOD_OUTPUT: { [K in MethodKind]: OutputRow<K> } = {
  describe: {
    bridge: (b) => b.describe,
    refusal: refusalSentence,
    runEnded: runEndedSentence,
    progress: (run) => progressSentence(run.columnsDone, run.columnsTotal),
    sentence: (d, view) => diagnosticSentence(d, columnNames(view)),
    card: (result, view) => <DescribeResult result={result} name={columnNames(view)} />,
    exportHeading: STAGE_HEADINGS.export,
    exportText: EXPORT_TEXT,
    exportChoices: EXPORT_CHOICES,
  },
  ttest: {
    bridge: (b) => b.ttest,
    refusal: ttestRefusalSentence,
    runEnded: ttestRunEndedSentence,
    progress: () => null,
    sentence: (d, view) => ttestSentence(d, { column: columnNames(view), level: levelByPosition }),
    card: (result, view) => <ResultCard result={result} names={{ column: columnNames(view), level: ttestNames(result).level }} />,
    exportHeading: TTEST_STAGE_HEADINGS.export,
    exportText: TTEST_EXPORT_TEXT,
    exportChoices: TTEST_EXPORT_CHOICES,
  },
  one_sample: {
    bridge: (b) => b.oneSample,
    refusal: oneSampleRefusalSentence,
    runEnded: oneSampleRunEndedSentence,
    progress: () => null,
    sentence: (d, view) => oneSampleSentence(d, { column: columnNames(view) }),
    card: (result, view) => <OneSampleResultCard result={result} names={{ column: columnNames(view) }} />,
    exportHeading: ONE_SAMPLE_STAGE_HEADINGS.export,
    exportText: ONE_SAMPLE_EXPORT_TEXT,
    exportChoices: ONE_SAMPLE_EXPORT_CHOICES,
  },
  paired: {
    bridge: (b) => b.paired,
    refusal: oneSampleRefusalSentence,
    runEnded: oneSampleRunEndedSentence,
    progress: () => null,
    sentence: (d, view) => oneSampleSentence(d, { column: columnNames(view) }),
    card: (result, view) => <PairedResultCard result={result} names={{ column: columnNames(view) }} />,
    exportHeading: PAIRED_STAGE_HEADINGS.export,
    exportText: PAIRED_EXPORT_TEXT,
    exportChoices: PAIRED_EXPORT_CHOICES,
  },
  anova: {
    bridge: (b) => b.anova,
    refusal: anovaRefusalSentence,
    runEnded: anovaRunEndedSentence,
    progress: (run) => anovaSteps(run.stepsDone, run.stepsTotal),
    sentence: (d, view) => anovaSentence(d, { column: columnNames(view), level: levelByPosition }),
    card: (result, view) => <AnovaResultCard result={result} names={{ column: columnNames(view), level: anovaNames(result).level }} />,
    exportHeading: ANOVA_STAGE_HEADINGS.export,
    exportText: ANOVA_EXPORT_TEXT,
    exportChoices: ANOVA_EXPORT_CHOICES,
  },
  correlations: {
    bridge: (b) => b.corr,
    refusal: corrRefusalSentence,
    runEnded: corrRunEndedSentence,
    progress: (run) => corrSteps(run.stepsDone, run.stepsTotal),
    sentence: (d, view) => corrSentence(d, { column: columnNames(view) }),
    card: (result, view) => <CorrResultCard result={result} names={{ column: columnNames(view) }} />,
    exportHeading: CORR_STAGE_HEADINGS.export,
    exportText: CORR_EXPORT_TEXT,
    exportChoices: CORR_EXPORT_CHOICES,
  },
  cronbach_alpha: {
    bridge: (b) => b.alpha,
    refusal: alphaRefusalSentence,
    runEnded: alphaRunEndedSentence,
    progress: (run) => alphaSteps(run.stepsDone, run.stepsTotal),
    sentence: (d, view) => alphaSentence(d, { column: columnNames(view) }),
    card: (result, view) => <AlphaResultCard result={result} names={{ column: columnNames(view) }} />,
    exportHeading: ALPHA_STAGE_HEADINGS.export,
    exportText: ALPHA_EXPORT_TEXT,
    exportChoices: ALPHA_EXPORT_CHOICES,
  },
};

/** An entry's row. Its kind and its run and result were stored together, so the
 *  correlation TypeScript cannot follow through a union holds. */
export function rowOf(kind: MethodKind): OutputRow<never> & { bridge: (b: RunBridges) => EntryBridge; progress: (run: MethodRuns[MethodKind]) => string | null; card: (result: MethodResults[MethodKind], view: DatasetView) => React.ReactNode; sentence: (d: { code: string; params?: Record<string, unknown>; severity?: string }, view: DatasetView) => string } {
  return METHOD_OUTPUT[kind] as never;
}

/* ---- each method's dialog (decision 481; ADR-012 Part 5): a kind with no row is not live in the menu ---- */

export interface StartedRun {
  runId: number;
  settings: string;
  run: MethodRuns[MethodKind];
}

export interface MethodDialogRowProps {
  view: DatasetView;
  slot: number;
  bridges: RunBridges;
  outputFull: boolean;
  onStarted: (started: StartedRun) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}

export const METHOD_DIALOGS: Record<MethodKind, React.FC<MethodDialogRowProps>> = {
  describe: ({ bridges, ...p }) => <DescribeDialog {...p} bridge={bridges.describe} />,
  ttest: ({ bridges, ...p }) => <TTestDialog {...p} bridge={bridges.ttest} />,
  one_sample: ({ bridges, ...p }) => <OneSampleDialog {...p} bridge={bridges.oneSample} />,
  paired: ({ bridges, ...p }) => <PairedDialog {...p} bridge={bridges.paired} />,
  anova: ({ bridges, ...p }) => <AnovaDialog {...p} bridge={bridges.anova} />,
  correlations: ({ bridges, ...p }) => <CorrDialog {...p} bridge={bridges.corr} />,
  cronbach_alpha: ({ bridges, ...p }) => <AlphaDialog {...p} bridge={bridges.alpha} />,
};
