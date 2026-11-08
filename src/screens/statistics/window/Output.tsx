// Gaply — the Output (Research OS, Phase 8R, decision 471; ADR-012 Part 7): every
// result of the session, newest last. An entry is mounted when the pane first goes
// to it and stays mounted while the Output is shown, so all 32 need not render at
// the start. All it shows lives in its session entry, so a remount draws the same.
// In memory only, with no autosave: Save and Export are the user's clicks, through
// today's commands, and Rust's refusal stays the boundary whatever this shows.
import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Button } from '../../../design-system';
import type { RunBridges } from '../../workspace/useWorkspaceSwitch';
import type { DescribeBridge } from '../describeBridge';
import { refusalOf } from '../../statsverifier/l1/l1Wording';
import type { RunState } from '../describeTypes';
import { saveExactly, useRunPolling } from '../flowHooks';
import { DiagnosticList, ExportCard, RunCard } from '../flowParts';
import type { ProjectBridge } from '../projects/projectBridge';
import { projectsShown } from '../projects/projectGate';
import { openProjectStore } from '../projects/projectSession';
import { PROJECT_TEXT, PROJECT_WORDS } from '../projects/projectWording';
import { SaveToProject } from '../projects/SaveToProject';
import { rowOf, type EntryBridge } from './methodOutput';
import { AskFirst } from './MethodDialog';
import './Output.css';
import { methodName, OutputOutline } from './OutputOutline';
import { entryNotSaved, runIsGoing, statisticsSession, type EntryPatch, type MethodRuns, type OutputEntry, type StatisticsSessionState } from './statisticsSession';
import { useRerunAll } from './rerunAll';
import { ASK_REMOVE, OUTPUT_EMPTY, OUTPUT_EXPORT, OUTPUT_SAVED, OUTPUT_SETTINGS, RERUNS_GOING } from './windowWording';

type SaveFile = (name: string, text: string, mime: string) => Promise<string | null>;

export const entryHeadingId = (number: number) => `sw-entry-heading-${number}`;

const EntryWatcher: React.FC<{ entry: OutputEntry; bridges: RunBridges; pollMs: number }> = ({ entry, bridges, pollMs }) => {
  const { number, runId, run, result, error, kind } = entry;
  const row = rowOf(kind);
  const bridge = row.bridge(bridges);
  const setRun = useCallback((next: MethodRuns[OutputEntry['kind']]) => statisticsSession.updateEntry(number, { run: next }), [number]);
  const setError = useCallback((sentence: string) => statisticsSession.updateEntry(number, { error: sentence }), [number]);
  useRunPolling<MethodRuns[OutputEntry['kind']]>(error === null ? run : null, setRun, bridge, setError, row.refusal, pollMs);
  const wantsResult = run.state === 'succeeded' && run.terminal && result === null && error === null;
  useEffect(() => {
    if (!wantsResult) return undefined;
    let live = true;
    bridge.result(runId).then(
      (r) => {
        if (live) statisticsSession.updateEntry(number, { result: r });
      },
      (e) => {
        if (live) setError(row.refusal(e));
      }
    );
    return () => {
      live = false;
    };
  }, [wantsResult, bridge, row, runId, number, setError]);
  return null;
};

/** Reads each run that needs reading, whichever tab is shown: mounted by the
 *  window, so a run that ended behind the Data View is not "going" for ever. */
export const OutputWatchers: React.FC<{ entries: readonly OutputEntry[]; bridges: RunBridges; pollMs: number }> = ({ entries, bridges, pollMs }) => (
  <>
    {entries
      .filter((e) => e.error === null && (!e.run.terminal || (e.run.state === 'succeeded' && e.result === null)))
      .map((e) => (
        <EntryWatcher key={e.number} entry={e} bridges={bridges} pollMs={pollMs} />
      ))}
  </>
);

interface EntryPaneProps {
  entry: OutputEntry;
  shown: boolean;
  slotOpen: boolean;
  bridge: EntryBridge;
  projects?: ProjectBridge;
  save: SaveFile;
}

/** Memoised: a run's poll or a move through the outline changes the session, and
 *  32 result tables must not redraw for it. */
const EntryPane = React.memo<EntryPaneProps>(({ entry, shown, slotOpen, bridge, projects, save }) => {
  const { number, run, result } = entry;
  const prefix = `sw-entry-${number}`;
  const row = rowOf(entry.kind);
  const update = (patch: EntryPatch) => statisticsSession.updateEntry(number, patch);

  const progress = row.progress(run);
  const diagnostics: Array<{ code: string; params?: Record<string, unknown>; severity: string }> = run.diagnostics as never;

  // While the window's list of reruns goes, no Save may change the open project
  // between two reruns (decision 538): the same disabled form, with its sentence.
  const listGoing = useRerunAll().status === 'going';
  const saveNode = !slotOpen || listGoing ? (
    projectsShown() ? (
      <div>
        <Button variant="secondary" disabled aria-describedby={`${prefix}-save-why`} data-testid={`${prefix}-save-off`}>
          {PROJECT_TEXT.saveButton}
        </Button>
        <p id={`${prefix}-save-why`} style={{ margin: '4px 0 0' }} data-testid={`${prefix}-save-why`}>
          {listGoing ? RERUNS_GOING : PROJECT_WORDS['project.data_unavailable']({ code: 'project.data_unavailable' })}
        </p>
      </div>
    ) : null
  ) : (
    <SaveToProject
      runId={entry.runId}
      bridge={projects}
      testId={`${prefix}-project`}
      prepared={entry.view.preparation !== undefined}
      onSaved={(projectId) => {
        const open = openProjectStore.get();
        update({ saved: { projectId, name: open?.projectId === projectId ? open.name : '' } });
      }}
    />
  );

  const cancel = () => {
    bridge.cancel(entry.runId).then(
      (next) => update({ run: next }),
      (e) => update({ error: row.refusal(e) })
    );
  };

  const saveExport = async () => {
    const choice = row.exportChoices.find((c) => c.format === entry.exportFormat) ?? row.exportChoices[0];
    update({ exportNote: null });
    try {
      const out = await bridge.export(entry.runId, entry.exportFormat);
      const saved = await saveExactly(out, choice, save);
      if ('error' in saved) update({ error: saved.error });
      else update({ exportNote: saved.note });
    } catch (e) {
      update({ error: row.refusal(e) });
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
          endedSentence={(state) => row.runEnded(state as RunState)}
          progress={
            progress === null ? undefined : (
              <p style={{ margin: 0 }} data-testid={`${prefix}-progress`}>
                {progress}
              </p>
            )
          }
          diagnostics={<DiagnosticList items={diagnostics} sentence={(d) => row.sentence(d, entry.view)} testId={`${prefix}-diagnostics`} />}
          busy={false}
          onCancel={cancel}
          save={saveNode}
        />
      )}
      {result !== null && (
        <>
          {row.card(result, entry.view)}
          <div className="sw-entry__actions">
            {saveNode}
            <Button variant="secondary" onClick={() => update({ exportOpen: !entry.exportOpen })} data-testid={`${prefix}-export`}>
              {OUTPUT_EXPORT}
            </Button>
          </div>
          {entry.exportOpen && (
            <ExportCard
              prefix={prefix}
              title={<h4 style={{ margin: 0 }}>{row.exportHeading}</h4>}
              text={row.exportText}
              choices={row.exportChoices}
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
  bridges: RunBridges;
  forget: Pick<DescribeBridge, 'forgetRun'>;
  projects?: ProjectBridge;
  save: SaveFile;
  /** Which result a run just moved the pane to. */
  status: string | null;
}> = ({ session, bridges, forget, projects, save, status }) => {
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
        setRefusal(rowOf(e.kind).refusal(err));
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
    if (entryNotSaved(e, slot)) setAsking(number);
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
            <EntryPane key={e.number} entry={e} shown={e.number === selected} slotOpen={e.slot === slot} bridge={rowOf(e.kind).bridge(bridges)} projects={projects} save={save} />
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
