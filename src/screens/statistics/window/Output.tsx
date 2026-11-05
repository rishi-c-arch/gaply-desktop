// Gaply — the Output (Research OS, Phase 8R, decision 471; ADR-012 Part 7): every
// result of the session, newest last. An entry is mounted when the pane first goes
// to it and stays mounted while the Output is shown, so all 32 need not render at
// the start. All it shows lives in its session entry, so a remount draws the same.
// In memory only, with no autosave: Save and Export are the user's clicks, through
// today's commands, and Rust's refusal stays the boundary whatever this shows.
import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Button } from '../../../design-system';
import { AlphaResultCard } from '../AlphaFlow';
import type { AlphaBridge } from '../alphaBridge';
import { alphaNames } from '../AlphaSettings';
import type { AlphaRunStateView } from '../alphaTypes';
import { ALPHA_EXPORT_CHOICES, ALPHA_EXPORT_TEXT, ALPHA_STAGE_HEADINGS, alphaRefusalSentence, alphaRunEndedSentence, alphaSentence, stepsSentence } from '../alphaWording';
import type { DescribeBridge } from '../describeBridge';
import { refusalOf } from '../../statsverifier/l1/l1Wording';
import { saveExactly, useRunPolling } from '../flowHooks';
import { DiagnosticList, ExportCard, RunCard } from '../flowParts';
import type { ProjectBridge } from '../projects/projectBridge';
import { projectsShown } from '../projects/projectGate';
import { openProjectStore } from '../projects/projectSession';
import { PROJECT_TEXT, PROJECT_WORDS } from '../projects/projectWording';
import { SaveToProject } from '../projects/SaveToProject';
import { AskFirst } from './MethodDialog';
import './Output.css';
import { methodName, OutputOutline } from './OutputOutline';
import { runIsGoing, statisticsSession, type OutputEntry, type StatisticsSessionState } from './statisticsSession';
import { ASK_REMOVE, OUTPUT_EMPTY, OUTPUT_EXPORT, OUTPUT_SAVED, OUTPUT_SETTINGS } from './windowWording';

/** What an entry may do with its run. It owns no file, so no open and no close. */
export type OutputBridge = Omit<AlphaBridge, 'open' | 'close' | 'validate' | 'run'>;
type SaveFile = (name: string, text: string, mime: string) => Promise<string | null>;

export const entryHeadingId = (number: number) => `sw-entry-heading-${number}`;

const EntryWatcher: React.FC<{ entry: OutputEntry; bridge: OutputBridge; pollMs: number }> = ({ entry, bridge, pollMs }) => {
  const { number, runId, run, result, error } = entry;
  const setRun = useCallback((next: AlphaRunStateView) => statisticsSession.updateEntry(number, { run: next }), [number]);
  const setError = useCallback((sentence: string) => statisticsSession.updateEntry(number, { error: sentence }), [number]);
  useRunPolling(error === null ? run : null, setRun, bridge, setError, alphaRefusalSentence, pollMs);
  const wantsResult = run.state === 'succeeded' && run.terminal && result === null && error === null;
  useEffect(() => {
    if (!wantsResult) return undefined;
    let live = true;
    bridge.result(runId).then(
      (r) => {
        if (live) statisticsSession.updateEntry(number, { result: r });
      },
      (e) => {
        if (live) setError(alphaRefusalSentence(e));
      }
    );
    return () => {
      live = false;
    };
  }, [wantsResult, bridge, runId, number, setError]);
  return null;
};

/** Reads each run that needs reading, whichever tab is shown: mounted by the
 *  window, so a run that ended behind the Data View is not "going" for ever. */
export const OutputWatchers: React.FC<{ entries: readonly OutputEntry[]; bridge: OutputBridge; pollMs: number }> = ({ entries, bridge, pollMs }) => (
  <>
    {entries
      .filter((e) => e.error === null && (!e.run.terminal || (e.run.state === 'succeeded' && e.result === null)))
      .map((e) => (
        <EntryWatcher key={e.number} entry={e} bridge={bridge} pollMs={pollMs} />
      ))}
  </>
);

interface EntryPaneProps {
  entry: OutputEntry;
  shown: boolean;
  slotOpen: boolean;
  bridge: OutputBridge;
  projects?: ProjectBridge;
  save: SaveFile;
}

/** Memoised: a run's poll or a move through the outline changes the session, and
 *  32 result tables must not redraw for it. */
const EntryPane = React.memo<EntryPaneProps>(({ entry, shown, slotOpen, bridge, projects, save }) => {
  const { number, run, result } = entry;
  const prefix = `sw-entry-${number}`;
  const names = useMemo(() => alphaNames(entry.view), [entry.view]);
  const update = (patch: Partial<OutputEntry>) => statisticsSession.updateEntry(number, patch);

  const saveNode = !slotOpen ? (
    projectsShown() ? (
      <div>
        <Button variant="secondary" disabled aria-describedby={`${prefix}-save-why`} data-testid={`${prefix}-save-off`}>
          {PROJECT_TEXT.saveButton}
        </Button>
        <p id={`${prefix}-save-why`} style={{ margin: '4px 0 0' }} data-testid={`${prefix}-save-why`}>
          {PROJECT_WORDS['project.data_unavailable']({ code: 'project.data_unavailable' })}
        </p>
      </div>
    ) : null
  ) : (
    <SaveToProject
      runId={entry.runId}
      bridge={projects}
      testId={`${prefix}-project`}
      onSaved={(projectId) => {
        const open = openProjectStore.get();
        update({ saved: { projectId, name: open?.projectId === projectId ? open.name : '' } });
      }}
    />
  );

  const cancel = () => {
    bridge.cancel(entry.runId).then(
      (next) => update({ run: next }),
      (e) => update({ error: alphaRefusalSentence(e) })
    );
  };

  const saveExport = async () => {
    const choice = ALPHA_EXPORT_CHOICES.find((c) => c.format === entry.exportFormat) ?? ALPHA_EXPORT_CHOICES[0];
    update({ exportNote: null });
    try {
      const out = await bridge.export(entry.runId, entry.exportFormat);
      const saved = await saveExactly(out, choice, save);
      if ('error' in saved) update({ error: saved.error });
      else update({ exportNote: saved.note });
    } catch (e) {
      update({ error: alphaRefusalSentence(e) });
    }
  };

  return (
    <section className="sw-entry" hidden={!shown} data-testid={prefix}>
      <h3 id={entryHeadingId(number)} className="sw-entry__heading" tabIndex={-1} data-testid={`${prefix}-heading`}>
        {number} {methodName(entry.kind)}
      </h3>
      <p className="sw-entry__settings" data-testid={`${prefix}-settings`}>
        {OUTPUT_SETTINGS}
        {entry.settings}
      </p>
      {entry.saved && (
        <p style={{ margin: 0 }} data-testid={`${prefix}-saved`}>
          {OUTPUT_SAVED}
          <bdi>{entry.saved.name}</bdi>.
        </p>
      )}
      {entry.error && (
        <p role="alert" data-testid={`${prefix}-error`}>
          {entry.error}
        </p>
      )}
      {result === null && (
        <RunCard
          prefix={prefix}
          title={null}
          runView={run}
          endedSentence={alphaRunEndedSentence}
          progress={
            <p style={{ margin: 0 }} data-testid={`${prefix}-progress`}>
              {stepsSentence(run.stepsDone, run.stepsTotal)}
            </p>
          }
          diagnostics={<DiagnosticList items={run.diagnostics} sentence={(d) => alphaSentence(d, names)} testId={`${prefix}-diagnostics`} />}
          busy={false}
          onCancel={cancel}
          save={saveNode}
        />
      )}
      {result !== null && (
        <>
          <AlphaResultCard result={result} names={names} />
          <div className="sw-entry__actions">
            {saveNode}
            <Button variant="secondary" onClick={() => update({ exportOpen: !entry.exportOpen })} data-testid={`${prefix}-export`}>
              {OUTPUT_EXPORT}
            </Button>
          </div>
          {entry.exportOpen && (
            <ExportCard
              prefix={prefix}
              title={<h4 style={{ margin: 0 }}>{ALPHA_STAGE_HEADINGS.export}</h4>}
              text={ALPHA_EXPORT_TEXT}
              choices={ALPHA_EXPORT_CHOICES}
              format={entry.exportFormat}
              onFormat={(exportFormat) => update({ exportFormat })}
              busy={false}
              onBack={() => update({ exportOpen: false })}
              onSave={() => void saveExport()}
              note={null}
            />
          )}
          {entry.exportNote && (
            <p role="status" data-testid={`${prefix}-export-note`}>
              {entry.exportNote}
            </p>
          )}
        </>
      )}
    </section>
  );
});
EntryPane.displayName = 'EntryPane';

export const Output: React.FC<{
  session: StatisticsSessionState;
  bridge: OutputBridge;
  forget: Pick<DescribeBridge, 'forgetRun'>;
  projects?: ProjectBridge;
  save: SaveFile;
  /** Which result a run just moved the pane to. */
  status: string | null;
}> = ({ session, bridge, forget, projects, save, status }) => {
  const { output, selected, slot } = session;
  const [asking, setAsking] = useState<number | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const focusNext = useRef<number | null>(null);
  const seen = useRef(new Set<number>());
  if (selected !== null) seen.current.add(selected);

  useLayoutEffect(() => {
    if (selected === null || focusNext.current !== selected) return;
    focusNext.current = null;
    document.getElementById(entryHeadingId(selected))?.focus();
  }, [selected, output.length]);

  const go = (number: number) => {
    focusNext.current = number;
    if (selected === number) document.getElementById(entryHeadingId(number))?.focus();
    statisticsSession.selectEntry(number);
  };

  const removeNow = async (number: number) => {
    const e = output.find((x) => x.number === number);
    if (!e) return;
    setRefusal(null);
    try {
      await forget.forgetRun(e.runId);
    } catch (err) {
      // A record already gone is what Remove wanted.
      if (refusalOf(err)?.code !== 'run.unknown_run') {
        setRefusal(alphaRefusalSentence(err));
        return;
      }
    }
    seen.current.delete(number);
    statisticsSession.removeEntry(number);
    const next = statisticsSession.get().selected;
    if (next !== null) focusNext.current = next;
  };

  const remove = (number: number) => {
    const e = output.find((x) => x.number === number);
    if (!e || runIsGoing(e)) return;
    if (e.saved === null) setAsking(number);
    else void removeNow(number);
  };

  if (output.length === 0) return <p data-testid="sw-output-empty">{OUTPUT_EMPTY}</p>;

  return (
    <div className="sw-output" data-testid="sw-output">
      <OutputOutline entries={output} selected={selected} onGo={go} onRemove={remove} />
      <div className="sw-pane" data-testid="sw-pane" tabIndex={0}>
        <p role="status" aria-live="polite" style={{ margin: 0 }} data-testid="sw-output-status">
          {status ?? ''}
        </p>
        {refusal && (
          <p role="alert" data-testid="sw-output-refusal">
            {refusal}
          </p>
        )}
        {output
          .filter((e) => seen.current.has(e.number))
          .map((e) => (
            <EntryPane key={e.number} entry={e} shown={e.number === selected} slotOpen={e.slot === slot} bridge={bridge} projects={projects} save={save} />
          ))}
      </div>
      {asking !== null && (
        <AskFirst
          title={ASK_REMOVE.title}
          sentence={ASK_REMOVE.sentence}
          testId="sw-ask"
          onCancel={() => setAsking(null)}
          onContinue={() => {
            const n = asking;
            setAsking(null);
            void removeNow(n);
          }}
        />
      )}
    </div>
  );
};
