// Gaply — "Check a paper against its data" (Level 1, Phase 2, Step 7).
//
// Add -> Review claims -> Bind -> Confirm -> Results, over the Step 6 bridge.
// Rust owns every decision: this screen passes paths, ids, the user's ticks
// and edits, and shows what comes back.
//
// What lives here, and for how long: the column headers and level labels the
// open view carries (shown on this device, never sent by this screen), the
// claims' sentences and the results. No data row ever reaches the webview.
// Everything is held in this component's state only, never in browser
// storage, and is dropped, with the Rust run, when the check is closed or the
// screen unmounts.
//
// AI help appears only when its setting is on, and never calls anything
// without a click: opening files makes no helper call.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Badge, Button, Card } from '../../../design-system';
import { basenameOf, pickManuscriptPath } from '../../common/pickFile';
import { isTauri } from '../../../utils/isTauri';
import type { StatsL1Bridge } from '../statsL1Bridge';
import type {
  AiSettings,
  BindingEdits,
  HelperTaskInput,
  L1ClaimInput,
  L1ClaimView,
  L1ColumnView,
  L1ConfirmView,
  L1Handle,
  L1HelperResultView,
  L1MethodView,
  L1OpenView,
  L1ProposalView,
  L1UsageView,
  L1WholeNumberView,
  ReproductionView,
  WireCode,
} from '../statsL1Types';
import { FocusButton } from './FocusButton';
import { L1AiPanel } from './L1AiPanel';
import { L1ResultCard } from './L1ResultCard';
import { L1Settings, L1UsageLine } from './L1Settings';
import {
  COLUMN_TYPE_NAMES,
  columnIssueSentence,
  dataReadPhrase,
  datasetIssueSentence,
  ENTRY_TEXT,
  FAMILY_SYMBOL,
  level0Phrase,
  METHOD_NAMES,
  methodPhrase,
  notCompared,
  refusalSentence,
  ROLE_NAMES,
  sentenceFor,
  skippedPhrase,
  WHOLE_NUMBER_ANSWERS,
  WHOLE_NUMBER_QUESTION,
  WordContext,
} from './l1Wording';

/** Reproduced claims with a printed number that was not compared. */
function partlyReproduced(view: ReproductionView): number {
  return view.claims.filter((c) => c.verdict.kind === 'reproduced' && notCompared(c.verdict.notes).length > 0).length;
}

type Stage = 'add' | 'claims' | 'bind' | 'confirm' | 'results';
const STAGES: Array<{ id: Stage; label: string }> = [
  { id: 'add', label: 'Add' },
  { id: 'claims', label: 'Review claims' },
  { id: 'bind', label: 'Bind' },
  { id: 'confirm', label: 'Confirm' },
  { id: 'results', label: 'Results' },
];

const SOURCE_NAMES: Record<L1ProposalView['source'], string> = { local: 'Local', ai_suggestion: 'AI', manual: 'You' };

/** The user's binding form for one claim. */
export interface Draft {
  method: string;
  /** role -> column alias numbers */
  roles: Record<string, number[]>;
  /** group role -> the level codes included */
  levels: Record<string, string[]>;
  /** A further filter: keep only rows whose column is one of these levels. */
  filter: { alias: number; levels: string[] } | null;
  /** The variant the user ticks as stated in the paper. */
  variantTicked: string | null;
  /** The variant picked in the list, before or without the tick. */
  variantPick: string | null;
  olsTarget: 'overall_f' | number | null;
  /** Bound whole-number columns the user says hold exact whole numbers
   *  (evaluation fixes, part C, B-1); every other one is rounded. */
  exactWholeNumbers: number[];
  /** The bound whole-number columns of the proposal on screen, as Rust
   *  listed them: the Confirm step asks about each. */
  wholeNumbers: L1WholeNumberView[];
  /** The proposal this draft started from, and what Rust said about it.
   *  Confirm sends this handle only while `dirty` is false: a changed or
   *  refused draft is not what Rust holds (part C, M-S1). */
  from: L1Handle | null;
  source: L1ProposalView['source'];
  variant: string;
  variantSetBy: WireCode;
  /** Changed since `from`. */
  dirty: boolean;
}

const alias = (a: string) => Number(a.replace(/^C/, ''));

export function draftFrom(p: L1ProposalView, schema: L1ColumnView[]): Draft {
  const roles: Record<string, number[]> = {};
  for (const r of p.roles) roles[r.role] = r.aliases.map(alias);
  const levels: Record<string, string[]> = {};
  for (const l of p.levels) levels[l.role] = l.levels;
  // A group role without a selection includes every level of its column.
  for (const [role, cols] of Object.entries(roles)) {
    if (!levels[role] && cols.length === 1) {
      const col = schema.find((c) => alias(c.alias) === cols[0]);
      if (col && col.levels.length > 0) levels[role] = col.levels.map((l) => l.code);
    }
  }
  return {
    method: p.method,
    roles,
    levels,
    filter: null,
    variantTicked: null,
    variantPick: p.variant !== 'not_stated' && p.variant !== 'none' ? p.variant : null,
    olsTarget: null,
    exactWholeNumbers: p.wholeNumbers.filter((w) => w.answer === 'exact').map((w) => alias(w.alias)),
    wholeNumbers: p.wholeNumbers,
    from: p.proposal,
    source: p.source,
    variant: p.variant,
    variantSetBy: p.variantSetBy,
    dirty: false,
  };
}

function emptyDraft(method: string): Draft {
  return {
    method,
    roles: {},
    levels: {},
    filter: null,
    variantTicked: null,
    variantPick: null,
    olsTarget: null,
    exactWholeNumbers: [],
    wholeNumbers: [],
    from: null,
    source: 'manual',
    variant: 'not_stated',
    variantSetBy: { code: 'not_stated' },
    dirty: true,
  };
}

/** The draft as the edits Rust takes. Unticked levels of a group column
 *  become an include-filter on it: that is how a level is left out. */
export function editsOf(d: Draft, schema: L1ColumnView[], catalogue: L1MethodView[]): BindingEdits {
  const entry = catalogue.find((m) => m.method === d.method);
  const filters: BindingEdits['filters'] = [];
  const levels: BindingEdits['levels'] = [];
  for (const slot of entry?.slots ?? []) {
    if (slot.kind !== 'group') continue;
    const col = d.roles[slot.role]?.[0];
    const included = d.levels[slot.role];
    if (col === undefined || !included) continue;
    levels.push({ role: slot.role, levels: included });
    const all = schema.find((c) => alias(c.alias) === col)?.levels.map((l) => l.code) ?? [];
    if (included.length < all.length) filters.push({ alias: col, levels: included });
  }
  if (d.filter && d.filter.levels.length > 0) filters.push(d.filter);
  return {
    methodId: d.method,
    roles: (entry?.slots ?? []).filter((s) => (d.roles[s.role] ?? []).length > 0).map((s) => ({ role: s.role, aliases: d.roles[s.role] })),
    levels,
    filters,
    variantTicked: d.variantTicked,
    olsOverallF: d.olsTarget === 'overall_f' ? true : null,
    olsPredictor: typeof d.olsTarget === 'number' ? d.olsTarget : null,
    exactWholeNumbers: d.exactWholeNumbers,
  };
}

interface Confirmation {
  view: L1ConfirmView;
  proposal: L1ProposalView;
  /** The user's answer to a missing-code question, when there is one. */
  missingCodeAnswer: 'real_values' | 'missing_code' | null;
}

interface AiOpen {
  claimId: number;
  input: HelperTaskInput;
}

export interface L1FlowProps {
  bridge: StatsL1Bridge;
  onExit?: () => void;
}

const familyLine = (c: { family: string; statistic: string; df: string[]; n: string | null; p: string }) =>
  `${FAMILY_SYMBOL[c.family] ?? c.family}${c.df.length ? `(${c.df.join(', ')})` : ''} = ${c.statistic}, p ${c.p}${c.n ? `, n = ${c.n}` : ''}`;

export const L1Flow: React.FC<L1FlowProps> = ({ bridge, onExit }) => {
  const [stage, setStage] = useState<Stage>('add');
  const [manuscript, setManuscript] = useState<{ name: string; path: string } | null>(null);
  const [data, setData] = useState<{ name: string; path: string } | null>(null);
  const [view, setView] = useState<L1OpenView | null>(null);
  const [claims, setClaims] = useState<L1ClaimView[]>([]);
  const [skipped, setSkipped] = useState<Set<number>>(new Set());
  const [edited, setEdited] = useState<Set<number>>(new Set());
  const [editing, setEditing] = useState<number | 'new' | null>(null);
  const [proposals, setProposals] = useState<L1ProposalView[]>([]);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({});
  const [confirmed, setConfirmed] = useState<Record<number, Confirmation>>({});
  const [claimErrors, setClaimErrors] = useState<Record<number, string>>({});
  const [aiHints, setAiHints] = useState<Record<number, { reason: string; variantHint: string | null }>>({});
  const [ai, setAi] = useState<AiOpen | null>(null);
  const aiOpener = useRef<HTMLButtonElement | null>(null);
  const [result, setResult] = useState<ReproductionView | null>(null);
  const [settings, setSettings] = useState<AiSettings>({ aiHelp: false, webContext: false, model: '' });
  const [showSettings, setShowSettings] = useState(false);
  const [usage, setUsage] = useState<L1UsageView | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Focus between stages (evaluation fixes, part C, M-S9): each stage's
  // heading takes focus when the stage changes, and a form or panel gives
  // focus back to the control that opened it when it closes.
  const stageHeading = useRef<HTMLHeadingElement>(null);
  const shownStage = useRef<Stage>(stage);
  const editButtons = useRef<Record<string, HTMLButtonElement | null>>({});
  const addClaimButton = useRef<HTMLButtonElement>(null);
  const manuscriptInput = useRef<HTMLInputElement>(null);
  const dataInput = useRef<HTMLInputElement>(null);
  const run = view?.run ?? null;
  const runRef = useRef<number | null>(null);
  runRef.current = run;

  useEffect(() => {
    bridge.settingGet().then(setSettings).catch(() => undefined);
  }, [bridge]);

  useEffect(() => {
    if (shownStage.current === stage) return;
    shownStage.current = stage;
    stageHeading.current?.focus();
  }, [stage]);

  // The control a closed form gives focus back to. Focused after the
  // re-render: "Add a claim" is not mounted while its form is open.
  const refocus = useRef<number | 'new' | null>(null);
  const closeEditor = (claimId: number | 'new') => {
    refocus.current = claimId;
    setEditing(null);
  };
  useEffect(() => {
    if (editing !== null || refocus.current === null) return;
    const to = refocus.current;
    refocus.current = null;
    (to === 'new' ? addClaimButton.current : editButtons.current[String(to)])?.focus();
  }, [editing]);

  // Closing the screen closes the Rust run, and with it everything held.
  useEffect(
    () => () => {
      if (runRef.current !== null) void bridge.close(runRef.current).catch(() => undefined);
    },
    [bridge]
  );

  const schema = useMemo(() => view?.schema.columns ?? [], [view]);
  const catalogue = useMemo(() => view?.catalogue ?? [], [view]);
  const ctx: WordContext = useMemo(
    () => ({
      column: (a: number) => {
        const c = schema.find((x) => alias(x.alias) === a);
        return c ? `${c.alias} ${c.header}` : `C${a}`;
      },
    }),
    [schema]
  );
  const levelLabel = (col: number, code: string) => {
    const l = schema.find((x) => alias(x.alias) === col)?.levels.find((x) => x.code === code);
    return l?.label ?? code;
  };
  const included = claims.filter((c) => !skipped.has(c.claimId));

  const refreshUsage = useCallback(() => {
    if (runRef.current === null) return;
    bridge.usage(runRef.current).then(setUsage).catch(() => undefined);
  }, [bridge]);

  const resetRun = () => {
    setView(null);
    setClaims([]);
    setSkipped(new Set());
    setEdited(new Set());
    setEditing(null);
    setProposals([]);
    setDrafts({});
    setConfirmed({});
    setClaimErrors({});
    setAiHints({});
    setAi(null);
    setResult(null);
    setUsage(null);
  };

  const closeCheck = async () => {
    const r = run;
    resetRun();
    setManuscript(null);
    setData(null);
    setStage('add');
    if (r !== null) await bridge.close(r).catch(() => undefined);
  };

  /* ------------------------------- Add -------------------------------- */

  const pickManuscript = async () => {
    const p = await pickManuscriptPath(['pdf', 'docx', 'txt'], 'Manuscript');
    if (p) setManuscript({ name: basenameOf(p), path: p });
  };
  const pickData = async () => {
    const p = await pickManuscriptPath(['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'], 'Data');
    if (p) setData({ name: basenameOf(p), path: p });
  };
  const fromInput = (f: File) => ({ name: f.name, path: (f as File & { path?: string }).path ?? f.name });

  const openFiles = async () => {
    if (!manuscript || !data) return;
    setBusy(true);
    setError(null);
    try {
      if (run !== null) await bridge.close(run).catch(() => undefined);
      resetRun();
      const v = await bridge.open(manuscript.path, data.path);
      setView(v);
      setClaims(v.claims);
      setProposals(v.proposals);
      const d: Record<number, Draft> = {};
      for (const c of v.claims) {
        const p = v.proposals.find((x) => x.claimId === c.claimId);
        d[c.claimId] = p ? draftFrom(p, v.schema.columns) : emptyDraft(v.catalogue[0]?.method ?? 'independent_t');
      }
      setDrafts(d);
      setStage('claims');
    } catch (e) {
      setError(refusalSentence(e));
    } finally {
      setBusy(false);
    }
  };

  /* --------------------------- Review claims -------------------------- */

  const invalidate = (claimId: number) =>
    setConfirmed((c) => {
      if (!(claimId in c)) return c;
      const next = { ...c };
      delete next[claimId];
      return next;
    });

  const saveClaim = async (claimId: number | 'new', input: L1ClaimInput) => {
    if (run === null) return;
    setError(null);
    try {
      if (claimId === 'new') {
        const r = await bridge.addClaim(run, input);
        const added: L1ClaimView = {
          claimId: r.claimId,
          revision: r.revision,
          sectionIndex: 0,
          section: 'other',
          paragraph: 0,
          sentenceIndex: 0,
          sentence: input.sentence?.trim() || 'Added by you',
          family: input.family,
          statistic: input.statistic,
          df: input.df,
          n: input.n ?? null,
          p: input.p === null ? 'ns' : `${input.pRelation ?? '='} ${input.p}`,
          level0: null,
        };
        setClaims((cs) => [...cs, added]);
        const ps = await bridge.proposals(run);
        setProposals(ps);
        const p = ps.find((x) => x.claimId === r.claimId);
        setDrafts((d) => ({ ...d, [r.claimId]: p ? draftFrom(p, schema) : emptyDraft(catalogue[0]?.method ?? 'independent_t') }));
      } else {
        const r = await bridge.editClaim(run, claimId, input);
        setClaims((cs) =>
          cs.map((c) =>
            c.claimId === claimId
              ? {
                  ...c,
                  revision: r.revision,
                  family: input.family,
                  statistic: input.statistic,
                  df: input.df,
                  n: input.n ?? null,
                  p: input.p === null ? 'ns' : `${input.pRelation ?? '='} ${input.p}`,
                  level0: null,
                }
              : c
          )
        );
        // Rust dropped this claim's proposals and its confirmation: the form
        // stays as it was, and its binding must be confirmed again.
        setEdited((s) => new Set(s).add(claimId));
        invalidate(claimId);
        setProposals((ps) => ps.filter((p) => p.claimId !== claimId));
        setDrafts((d) => (d[claimId] ? { ...d, [claimId]: { ...d[claimId], from: null, dirty: true, source: 'manual' } } : d));
      }
      closeEditor(claimId);
    } catch (e) {
      setError(refusalSentence(e));
    }
  };

  /* -------------------------------- Bind ------------------------------ */

  const updateDraft = (claimId: number, f: (d: Draft) => Draft) => {
    setDrafts((ds) => ({ ...ds, [claimId]: { ...f(ds[claimId]), dirty: true } }));
    invalidate(claimId);
  };

  const onAiResult = async (claimId: number, r: L1HelperResultView) => {
    if (run === null) return;
    if (r.kind === 'claimMethod') {
      setAiHints((h) => ({ ...h, [claimId]: { reason: r.modelReason, variantHint: r.variantHint } }));
    }
    if (r.kind === 'claimMethod' || r.kind === 'bindingSuggestion') {
      const handle = r.proposal;
      const ps = await bridge.proposals(run);
      setProposals(ps);
      const p = handle && ps.find((x) => x.proposal.run === handle.run && x.proposal.id === handle.id);
      if (p) {
        const d = draftFrom(p, schema);
        if (r.kind === 'claimMethod' && r.variantHint) d.variantPick = r.variantHint;
        setDrafts((ds) => ({ ...ds, [claimId]: d }));
        invalidate(claimId);
      }
    }
  };

  /* ------------------------------- Confirm ---------------------------- */

  // Entering Confirm turns every changed form into a proposal (no network,
  // nothing confirmed), so the summary shows exactly what Rust will confirm.
  const prepareConfirm = async () => {
    if (run === null) return;
    setBusy(true);
    const errs: Record<number, string> = {};
    const next = { ...drafts };
    for (const c of included) {
      const d = next[c.claimId];
      if (!d || confirmed[c.claimId]) continue;
      if (d.dirty || !d.from) {
        try {
          const p = await bridge.draftProposal(run, c.claimId, d.from, editsOf(d, schema, catalogue));
          setProposals((ps) => [...ps, p]);
          next[c.claimId] = { ...draftFrom(p, schema), levels: d.levels, filter: d.filter, olsTarget: d.olsTarget, variantTicked: d.variantTicked, variantPick: d.variantPick };
        } catch (e) {
          errs[c.claimId] = refusalSentence(e, ctx);
        }
      }
    }
    setDrafts(next);
    setClaimErrors(errs);
    setStage('confirm');
    setBusy(false);
  };

  // An answer to the whole-number question is part of the binding: the
  // draft is sent to Rust again at once, and Confirm waits for the new
  // handle, so it always confirms the draft on screen.
  const answerWholeNumber = async (claimId: number, column: number, exact: boolean) => {
    const d = drafts[claimId];
    if (run === null || !d) return;
    const exactWholeNumbers = exact ? [...d.exactWholeNumbers.filter((a) => a !== column), column] : d.exactWholeNumbers.filter((a) => a !== column);
    const changed: Draft = {
      ...d,
      exactWholeNumbers,
      wholeNumbers: d.wholeNumbers.map((w) => (alias(w.alias) === column ? { ...w, answer: exact ? 'exact' : 'rounded' } : w)),
      dirty: true,
    };
    setDrafts((ds) => ({ ...ds, [claimId]: changed }));
    invalidate(claimId);
    setClaimErrors((e) => ({ ...e, [claimId]: '' }));
    try {
      const p = await bridge.draftProposal(run, claimId, d.from, editsOf(changed, schema, catalogue));
      setProposals((ps) => [...ps, p]);
      setDrafts((ds) => ({ ...ds, [claimId]: { ...draftFrom(p, schema), levels: changed.levels, filter: changed.filter, olsTarget: changed.olsTarget, variantTicked: changed.variantTicked, variantPick: changed.variantPick } }));
    } catch (e) {
      setClaimErrors((x) => ({ ...x, [claimId]: refusalSentence(e, ctx) }));
    }
  };

  const confirmOne = async (claimId: number) => {
    const d = drafts[claimId];
    // A draft Rust has not accepted is never confirmed (part C, M-S1).
    if (run === null || !d?.from || d.dirty) return;
    setClaimErrors((e) => ({ ...e, [claimId]: '' }));
    try {
      const v = await bridge.confirmBinding(run, d.from);
      const p = proposals.find((x) => x.proposal.id === d.from!.id) ?? {
        proposal: d.from,
        claimId,
        method: d.method,
        roles: [],
        levels: [],
        variant: d.variant,
        variantSetBy: d.variantSetBy,
        source: d.source,
        reason: null,
        wholeNumbers: d.wholeNumbers,
      };
      setConfirmed((c) => ({ ...c, [claimId]: { view: v, proposal: p, missingCodeAnswer: null } }));
    } catch (e) {
      setClaimErrors((x) => ({ ...x, [claimId]: refusalSentence(e, ctx) }));
    }
  };

  const allConfirmed =
    included.length > 0 &&
    included.every((c) => {
      const k = confirmed[c.claimId];
      return k && k.missingCodeAnswer !== 'missing_code';
    });

  const runCheck = async () => {
    if (run === null || !allConfirmed) return;
    setBusy(true);
    setError(null);
    try {
      setResult(await bridge.compute(run, included.map((c) => confirmed[c.claimId].view.token)));
      setStage('results');
    } catch (e) {
      setError(refusalSentence(e, ctx));
    } finally {
      setBusy(false);
    }
  };

  /* ------------------------------ Rendering --------------------------- */

  const openAi = (claimId: number, input: HelperTaskInput, opener: HTMLButtonElement | null) => {
    aiOpener.current = opener;
    setAi({ claimId, input });
  };
  const closeAi = () => {
    setAi(null);
    aiOpener.current?.focus();
  };
  const aiInput = ai?.input;

  const stageNav = (
    <nav aria-label="Steps" data-testid="l1-steps" style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
      {STAGES.map((s) => (
        <span key={s.id} aria-current={s.id === stage ? 'step' : undefined} style={{ fontWeight: s.id === stage ? 700 : 400 }}>
          {s.label}
        </span>
      ))}
    </nav>
  );

  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="l1-flow">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        {stageNav}
        <div style={{ display: 'flex', gap: 8 }}>
          <Button variant="ghost" onClick={() => setShowSettings((s) => !s)} aria-expanded={showSettings} data-testid="l1-settings-toggle">
            AI help settings
          </Button>
          {run !== null && (
            <Button variant="secondary" onClick={() => void closeCheck()} data-testid="l1-close">Close this check</Button>
          )}
          {onExit && <Button variant="ghost" onClick={onExit} data-testid="l1-exit">Back to the verifier</Button>}
        </div>
      </div>
      {showSettings && <L1Settings bridge={bridge} settings={settings} onSettings={setSettings} />}
      {run !== null && settings.aiHelp && <L1UsageLine usage={usage} />}
      {error && <p role="alert" style={{ color: 'var(--g-flagged)' }} data-testid="l1-error">{error}</p>}

      {stage === 'add' && (
        <Card>
          <h3 className="gds-card__title" ref={stageHeading} tabIndex={-1} data-testid="l1-stage-heading">Check a paper against its data</h3>
          <p className="gds-jc__disclaimer" data-testid="l1-entry-text">{ENTRY_TEXT}</p>
          <div style={{ display: 'grid', gap: 6 }}>
            <div>
              <Button variant="secondary" onClick={() => (isTauri ? void pickManuscript() : manuscriptInput.current?.click())} data-testid="l1-pick-manuscript">
                Choose manuscript
              </Button>{' '}
              <input ref={manuscriptInput} type="file" accept=".pdf,.docx,.txt" style={{ display: 'none' }} data-testid="l1-manuscript-file" aria-label="Manuscript file" onChange={(e) => e.target.files?.[0] && setManuscript(fromInput(e.target.files[0]))} />
              <span className="gds-mono" data-testid="l1-manuscript-name">{manuscript?.name ?? 'No manuscript chosen'}</span>
            </div>
            <div>
              <Button variant="secondary" onClick={() => (isTauri ? void pickData() : dataInput.current?.click())} data-testid="l1-pick-data">
                Choose data file
              </Button>{' '}
              <input ref={dataInput} type="file" accept=".csv,.tsv,.xlsx,.xls,.xlsb,.xlsm,.ods,.sav,.mat" style={{ display: 'none' }} data-testid="l1-data-file" aria-label="Data file" onChange={(e) => e.target.files?.[0] && setData(fromInput(e.target.files[0]))} />
              <span className="gds-mono" data-testid="l1-data-name">{data?.name ?? 'No data file chosen'}</span>
            </div>
            <div>
              <Button onClick={() => void openFiles()} disabled={!manuscript || !data || busy} data-testid="l1-open">
                Open both files
              </Button>
            </div>
          </div>
        </Card>
      )}

      {view && stage !== 'add' && (
        <p data-testid="l1-files-summary" style={{ margin: 0 }}>
          <span className="gds-mono">{manuscript?.name}</span> and <span className="gds-mono">{data?.name}</span>: {claims.length}{' '}
          {claims.length === 1 ? 'claim' : 'claims'} read, {view.schema.columns.length} {view.schema.columns.length === 1 ? 'column' : 'columns'}, {view.schema.rows} rows.
          {view.notLocated > 0 ? ` ${view.notLocated} more results could not be placed in a sentence and are not listed.` : ''}
        </p>
      )}
      {view && stage !== 'add' && view.schema.issues.length > 0 && (
        <ul data-testid="l1-dataset-issues" style={{ margin: 0 }}>
          {view.schema.issues.map((c, i) => (
            <li key={i}>{datasetIssueSentence(c)}</li>
          ))}
        </ul>
      )}
      {view && stage !== 'add' && schema.some((col) => col.issues.length > 0) && (
        <ul data-testid="l1-column-issues" style={{ margin: 0 }}>
          {schema.flatMap((col) => col.issues.map((c, i) => <li key={`${col.alias}-${i}`}>{columnIssueSentence(c, `${col.alias} ${col.header}`)}</li>))}
        </ul>
      )}

      {stage === 'claims' && view && (
        <Card>
          <h3 className="gds-card__title" ref={stageHeading} tabIndex={-1} data-testid="l1-stage-heading">Review the claims</h3>
          <p className="gds-jc__disclaimer">One row per result Gaply read. Edit a value that was read wrongly, skip a claim, or add one Gaply missed.</p>
          <ol style={{ display: 'grid', gap: 8, paddingLeft: 18 }} data-testid="l1-claims">
            {claims.map((c) => (
              <li key={c.claimId} data-testid={`l1-claim-${c.claimId}`} style={{ opacity: skipped.has(c.claimId) ? 0.6 : 1 }}>
                <p style={{ margin: 0 }}>{c.sentence}</p>
                <p className="gds-mono" style={{ margin: 0 }}>{familyLine(c)}</p>
                <p className="gds-jc__disclaimer" style={{ margin: 0 }} data-testid={`l1-claim-${c.claimId}-level0`}>{level0Phrase(c.level0)}</p>
                {edited.has(c.claimId) && (
                  <p style={{ margin: 0 }} data-testid={`l1-claim-${c.claimId}-reconfirm`}>Edited by you: its binding needs confirming again.</p>
                )}
                <div style={{ display: 'flex', gap: 6 }}>
                  <FocusButton variant="ghost" ref={(el) => { editButtons.current[String(c.claimId)] = el; }} onClick={() => setEditing(c.claimId)} data-testid={`l1-claim-${c.claimId}-edit`}>Edit</FocusButton>
                  <Button
                    variant="ghost"
                    aria-pressed={skipped.has(c.claimId)}
                    onClick={() =>
                      setSkipped((s) => {
                        const n = new Set(s);
                        if (n.has(c.claimId)) n.delete(c.claimId);
                        else n.add(c.claimId);
                        return n;
                      })
                    }
                    data-testid={`l1-claim-${c.claimId}-skip`}
                  >
                    {skipped.has(c.claimId) ? 'Include' : 'Skip'}
                  </Button>
                </div>
                {editing === c.claimId && <ClaimForm initial={c} onSave={(i) => void saveClaim(c.claimId, i)} onCancel={() => closeEditor(c.claimId)} />}
              </li>
            ))}
          </ol>
          {editing === 'new' ? (
            <ClaimForm initial={null} onSave={(i) => void saveClaim('new', i)} onCancel={() => closeEditor('new')} />
          ) : (
            <FocusButton variant="secondary" ref={addClaimButton} onClick={() => setEditing('new')} data-testid="l1-claim-add">Add a claim</FocusButton>
          )}
          <div style={{ marginTop: 8 }}>
            <Button onClick={() => setStage('bind')} disabled={included.length === 0} data-testid="l1-to-bind">Continue to binding</Button>
          </div>
        </Card>
      )}

      {stage === 'bind' && view && (
        <Card>
          <h3 className="gds-card__title" ref={stageHeading} tabIndex={-1} data-testid="l1-stage-heading">Bind each claim to the data</h3>
          <p className="gds-jc__disclaimer" data-testid="l1-bind-text">
            Choose the method and the columns for each claim. Column names leave this device only if you tick &lsquo;Also send
            my original column names&rsquo;. Level names never leave it.
          </p>
          {included.map((c) => {
            const d = drafts[c.claimId];
            if (!d) return null;
            return (
              <BindForm
                key={c.claimId}
                claim={c}
                draft={d}
                schema={schema}
                catalogue={catalogue}
                hint={aiHints[c.claimId]}
                edited={edited.has(c.claimId)}
                onChange={(f) => updateDraft(c.claimId, f)}
                onMethod={(m) => {
                  setDrafts((ds) => ({ ...ds, [c.claimId]: { ...emptyDraft(m), from: ds[c.claimId]?.from ?? null } }));
                  invalidate(c.claimId);
                }}
                ai={
                  settings.aiHelp
                    ? {
                        method: (el) => openAi(c.claimId, { task: 'claim_method', claimId: c.claimId, methodsPassage: null }, el),
                        columns: (el) => openAi(c.claimId, { task: 'binding_suggestion', claimId: c.claimId, methodId: d.method }, el),
                      }
                    : null
                }
              >
                {ai && ai.claimId === c.claimId && aiInput && aiInput.task !== 'result_explanation' && aiInput.task !== 'web_context' && (
                  <L1AiPanel
                    key={`${aiInput.task}-${c.claimId}`}
                    bridge={bridge}
                    run={view.run}
                    input={aiInput}
                    onResult={(r) => void onAiResult(c.claimId, r)}
                    onSettled={refreshUsage}
                    onClose={closeAi}
                  />
                )}
              </BindForm>
            );
          })}
          <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
            <Button variant="secondary" onClick={() => setStage('claims')} data-testid="l1-back-claims">Back to claims</Button>
            <Button onClick={() => void prepareConfirm()} disabled={busy} data-testid="l1-to-confirm">Continue to confirm</Button>
          </div>
        </Card>
      )}

      {stage === 'confirm' && view && (
        <Card>
          <h3 className="gds-card__title" ref={stageHeading} tabIndex={-1} data-testid="l1-stage-heading">Confirm each binding</h3>
          {included.map((c) => {
            const d = drafts[c.claimId];
            const k = confirmed[c.claimId];
            return (
              <section key={c.claimId} className="gds-card" data-testid={`l1-confirm-${c.claimId}`} aria-label={`Binding for claim ${c.claimId + 1}`} style={{ marginBottom: 8 }}>
                <p style={{ margin: 0 }}>{c.sentence}</p>
                {d && <p data-testid={`l1-summary-${c.claimId}`} style={{ margin: '4px 0' }}>{summaryOf(d, catalogue, ctx, levelLabel, k?.view)}</p>}
                {d && (
                  <p className="gds-jc__disclaimer" style={{ margin: 0 }}>
                    Variant: {variantLine(d)}. Source: {SOURCE_NAMES[d.source]}.
                  </p>
                )}
                {k?.view.suspectMissingCodes.map((s, i) => (
                  <fieldset key={i} data-testid={`l1-missing-code-${c.claimId}`} style={{ border: 'none', padding: 0 }}>
                    <legend>
                      Column {ctx.column(alias(s.alias))} has {s.cells} {s.cells === 1 ? 'cell' : 'cells'} equal to {s.code}. Is {s.code} a missing-value code in your data? Gaply has not changed these cells.
                    </legend>
                    <label>
                      <input type="radio" name={`mc-${c.claimId}-${i}`} checked={k.missingCodeAnswer === 'real_values'} onChange={() => setConfirmed((x) => ({ ...x, [c.claimId]: { ...x[c.claimId], missingCodeAnswer: 'real_values' } }))} />{' '}
                      No, they are real values
                    </label>{' '}
                    <label>
                      <input type="radio" name={`mc-${c.claimId}-${i}`} checked={k.missingCodeAnswer === 'missing_code'} onChange={() => setConfirmed((x) => ({ ...x, [c.claimId]: { ...x[c.claimId], missingCodeAnswer: 'missing_code' } }))} />{' '}
                      Yes, it marks a missing value
                    </label>
                    {k.missingCodeAnswer === 'missing_code' && (
                      <p data-testid={`l1-missing-code-${c.claimId}-advice`}>
                        Gaply does not recode data. Mark {s.code} as missing in the data file, then open it again; until then this claim cannot run.
                      </p>
                    )}
                  </fieldset>
                ))}
                {d?.wholeNumbers.map((wn) => (
                  <fieldset key={wn.alias} data-testid={`l1-whole-${c.claimId}-${wn.alias}`} style={{ border: 'none', padding: 0, margin: '4px 0' }}>
                    <legend>
                      Column {ctx.column(alias(wn.alias))}: {WHOLE_NUMBER_QUESTION}
                    </legend>
                    {WHOLE_NUMBER_ANSWERS.map((a) => (
                      <label key={a.answer} style={{ marginRight: 12 }}>
                        <input
                          type="radio"
                          name={`whole-${c.claimId}-${wn.alias}`}
                          checked={wn.answer === a.answer}
                          onChange={() => void answerWholeNumber(c.claimId, alias(wn.alias), a.answer === 'exact')}
                          data-testid={`l1-whole-${c.claimId}-${wn.alias}-${a.answer}`}
                        />{' '}
                        {a.label}
                      </label>
                    ))}
                  </fieldset>
                ))}
                {claimErrors[c.claimId] && <p role="alert" style={{ color: 'var(--g-flagged)' }}>{claimErrors[c.claimId]}</p>}
                {k ? (
                  <Badge status="certain" data-testid={`l1-confirmed-${c.claimId}`}>Confirmed</Badge>
                ) : (
                  <Button onClick={() => void confirmOne(c.claimId)} disabled={!d?.from || d.dirty} data-testid={`l1-confirm-btn-${c.claimId}`}>
                    Confirm this binding
                  </Button>
                )}
                {!k && d && (!d.from || d.dirty) && (
                  <p className="gds-jc__disclaimer" data-testid={`l1-confirm-wait-${c.claimId}`} style={{ margin: 0 }}>
                    Confirm is available once this binding is accepted as shown. Go back to binding to change it.
                  </p>
                )}
              </section>
            );
          })}
          {skipped.size > 0 && <p className="gds-jc__disclaimer">{skipped.size} skipped {skipped.size === 1 ? 'claim is' : 'claims are'} not run.</p>}
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('bind')} data-testid="l1-back-bind">Back to binding</Button>
            <Button onClick={() => void runCheck()} disabled={!allConfirmed || busy} data-testid="l1-run">Run the check</Button>
          </div>
          {!allConfirmed && <p className="gds-jc__disclaimer" data-testid="l1-run-blocked">Run is available once every included claim has a confirmed binding or is skipped.</p>}
        </Card>
      )}

      {stage === 'results' && result && view && (
        <section aria-labelledby="l1-results-h" data-testid="l1-results" style={{ display: 'grid', gap: 8 }}>
          <h3 id="l1-results-h" className="gds-card__title" ref={stageHeading} tabIndex={-1} data-testid="l1-stage-heading" style={{ margin: 0 }}>Results</h3>
          <p style={{ margin: 0 }} role="status" data-testid="l1-results-counts">
            {result.counts.reproduced} reproduced
            {partlyReproduced(result) > 0 ? ` (${partlyReproduced(result)} of them with a number that was not decided)` : ''},{' '}
            {result.counts.notReproduced} not reproduced, {result.counts.couldNotCheck} could not be checked.{' '}
            {skippedPhrase(result.skipped)}
            {result.skipped > 0 ? ' ' : ''}
            {dataReadPhrase(result.data)}
          </p>
          {result.skipped > 0 && (
            <ul data-testid="l1-results-skipped" style={{ margin: 0 }}>
              {claims
                .filter((c) => !result.claims.some((r) => r.claimId === c.claimId))
                .map((c) => (
                  <li key={c.claimId}>Skipped by you: {c.sentence}</li>
                ))}
            </ul>
          )}
          {result.claims.map((r) => (
            <L1ResultCard
              key={r.claimId}
              bridge={bridge}
              run={view.run}
              claim={claims.find((c) => c.claimId === r.claimId)}
              result={r}
              alpha={result.alpha}
              ctx={ctx}
              catalogue={catalogue}
              aiHelp={settings.aiHelp}
              webContext={settings.webContext}
              onUsage={refreshUsage}
            />
          ))}
          <div>
            <Button variant="secondary" onClick={() => setStage('bind')} data-testid="l1-results-back">Change a binding</Button>
          </div>
        </section>
      )}
    </div>
  );
};

/* ------------------------------------------------------------------------ *
 *  Pieces
 * ------------------------------------------------------------------------ */

function variantLine(d: Draft): string {
  if (d.variantTicked) return `${sentenceFor({ code: d.variantTicked })}, ${sentenceFor({ code: 'user_ticked' })}`;
  if (d.variantSetBy.code === 'stated_in_paper') return `${sentenceFor({ code: d.variant })}, ${sentenceFor(d.variantSetBy)}`;
  if (d.variant === 'none') return 'this method has none';
  return 'not stated';
}

export function summaryOf(
  d: Draft,
  catalogue: L1MethodView[],
  ctx: WordContext,
  levelLabel: (col: number, code: string) => string,
  counts?: L1ConfirmView
): string {
  const entry = catalogue.find((m) => m.method === d.method);
  const stated = d.variantTicked ?? (d.variantSetBy.code === 'stated_in_paper' ? d.variant : null);
  const parts = [`${methodPhrase(d.method, stated)}.`];
  for (const slot of entry?.slots ?? []) {
    const cols = d.roles[slot.role] ?? [];
    if (cols.length === 0) {
      parts.push(`${ROLE_NAMES[slot.role] ?? slot.role}: not chosen.`);
      continue;
    }
    let s = `${ROLE_NAMES[slot.role] ?? slot.role}: ${cols.map((a) => ctx.column(a)).join(', ')}`;
    const lv = d.levels[slot.role];
    if (slot.kind === 'group' && lv) s += `, levels ${lv.map((l) => levelLabel(cols[0], l)).join(' and ')}`;
    parts.push(`${s}.`);
  }
  if (d.filter && d.filter.levels.length > 0) {
    parts.push(`Only rows where ${ctx.column(d.filter.alias)} is ${d.filter.levels.map((l) => levelLabel(d.filter!.alias, l)).join(' or ')}.`);
  }
  if (entry?.needsTarget) {
    parts.push(d.olsTarget === 'overall_f' ? 'Target: the overall F.' : typeof d.olsTarget === 'number' ? `Target: the coefficient of ${ctx.column(d.olsTarget)}.` : 'Target: not chosen.');
  }
  if (counts) {
    if (counts.rowsExcludedMissing > 0) parts.push(`${counts.rowsExcludedMissing} ${counts.rowsExcludedMissing === 1 ? 'row' : 'rows'} excluded for missing values.`);
    if (counts.rowsFilteredOut > 0) parts.push(`${counts.rowsFilteredOut} ${counts.rowsFilteredOut === 1 ? 'row' : 'rows'} left out by the level choice or filter.`);
    parts.push(`n used: ${counts.nUsed} of ${counts.rowsRead} rows.`);
  }
  return parts.join(' ');
}

const FAMILIES: Array<L1ClaimInput['family']> = ['t', 'F', 'chi_square', 'r', 'z'];
const RELATIONS: Array<NonNullable<L1ClaimInput['pRelation']>> = ['=', '<', '<=', '>', '>='];

const ClaimForm: React.FC<{ initial: L1ClaimView | null; onSave: (i: L1ClaimInput) => void; onCancel: () => void }> = ({ initial, onSave, onCancel }) => {
  const rel0 = initial?.p.split(' ')[0];
  const [family, setFamily] = useState<L1ClaimInput['family']>(initial?.family ?? 't');
  const [statistic, setStatistic] = useState(initial?.statistic ?? '');
  const [df, setDf] = useState(initial?.df.join(', ') ?? '');
  const [n, setN] = useState(initial?.n ?? '');
  const [ns, setNs] = useState(initial?.p === 'ns');
  const [rel, setRel] = useState<NonNullable<L1ClaimInput['pRelation']>>((RELATIONS as string[]).includes(rel0 ?? '') ? (rel0 as never) : '=');
  const [p, setP] = useState(initial && initial.p !== 'ns' ? initial.p.split(' ').slice(1).join(' ') : '');
  const [sentence, setSentence] = useState('');
  const first = useRef<HTMLSelectElement>(null);
  useEffect(() => first.current?.focus(), []);
  const idp = initial ? `l1-edit-${initial.claimId}` : 'l1-add';
  return (
    <form
      data-testid={`${idp}-form`}
      onSubmit={(e) => {
        e.preventDefault();
        onSave({
          family,
          statistic: statistic.trim(),
          df: df.split(',').map((x) => x.trim()).filter(Boolean),
          n: n.trim() || null,
          p: ns ? null : p.trim(),
          pRelation: ns ? null : rel,
          sentence: initial ? null : sentence.trim() || null,
        });
      }}
      style={{ display: 'grid', gap: 4, gridTemplateColumns: 'max-content 1fr', alignItems: 'center', marginTop: 6 }}
    >
      <label htmlFor={`${idp}-family`}>Statistic</label>
      <select id={`${idp}-family`} ref={first} value={family} onChange={(e) => setFamily(e.target.value as L1ClaimInput['family'])}>
        {FAMILIES.map((f) => (
          <option key={f} value={f}>{FAMILY_SYMBOL[f]}</option>
        ))}
      </select>
      <label htmlFor={`${idp}-stat`}>Value as printed</label>
      <input id={`${idp}-stat`} data-testid={`${idp}-stat`} value={statistic} onChange={(e) => setStatistic(e.target.value)} required />
      <label htmlFor={`${idp}-df`}>Degrees of freedom (comma-separated)</label>
      <input id={`${idp}-df`} data-testid={`${idp}-df`} value={df} onChange={(e) => setDf(e.target.value)} />
      <label htmlFor={`${idp}-n`}>n (optional)</label>
      <input id={`${idp}-n`} value={n} onChange={(e) => setN(e.target.value)} />
      <span>p</span>
      <span>
        <label>
          <input type="checkbox" checked={ns} onChange={() => setNs(!ns)} /> printed only as ns
        </label>{' '}
        <select aria-label="p relation" value={rel} disabled={ns} onChange={(e) => setRel(e.target.value as never)}>
          {RELATIONS.map((r) => (
            <option key={r} value={r}>{r}</option>
          ))}
        </select>{' '}
        <input aria-label="p as printed" data-testid={`${idp}-p`} value={p} disabled={ns} onChange={(e) => setP(e.target.value)} />
      </span>
      {!initial && (
        <>
          <label htmlFor={`${idp}-sentence`}>Sentence (optional)</label>
          <input id={`${idp}-sentence`} value={sentence} onChange={(e) => setSentence(e.target.value)} />
        </>
      )}
      <span />
      <span style={{ display: 'flex', gap: 6 }}>
        <Button type="submit" data-testid={`${idp}-save`}>Save</Button>
        <Button variant="ghost" onClick={onCancel}>Cancel</Button>
      </span>
    </form>
  );
};

interface BindFormProps {
  claim: L1ClaimView;
  draft: Draft;
  schema: L1ColumnView[];
  catalogue: L1MethodView[];
  hint?: { reason: string; variantHint: string | null };
  edited: boolean;
  onChange: (f: (d: Draft) => Draft) => void;
  onMethod: (m: string) => void;
  ai: { method: (el: HTMLButtonElement | null) => void; columns: (el: HTMLButtonElement | null) => void } | null;
  children?: React.ReactNode;
}

const BindForm: React.FC<BindFormProps> = ({ claim, draft, schema, catalogue, hint, edited, onChange, onMethod, ai, children }) => {
  const entry = catalogue.find((m) => m.method === draft.method);
  const id = `l1-bind-${claim.claimId}`;
  const methodBtn = useRef<HTMLButtonElement>(null);
  const columnsBtn = useRef<HTMLButtonElement>(null);
  const colName = (c: L1ColumnView) => `${c.alias} ${c.header} (${COLUMN_TYPE_NAMES[c.columnType] ?? c.columnType}${c.hasMissing ? ', has missing values' : ''})`;
  const groupCols = schema.filter((c) => c.levels.length > 0);
  const stated = draft.variantSetBy.code === 'stated_in_paper' && !draft.variantTicked;

  return (
    <section className="gds-card" aria-labelledby={`${id}-h`} data-testid={id} style={{ marginBottom: 8 }}>
      <h4 id={`${id}-h`} style={{ margin: 0 }}>{claim.sentence}</h4>
      <p className="gds-mono" style={{ margin: 0 }}>{familyLine(claim)}</p>
      <p style={{ margin: '4px 0' }} data-testid={`${id}-source`}>
        <Badge status="neutral">Suggestion, not confirmed</Badge> Source: {SOURCE_NAMES[draft.source]}
        {draft.dirty && draft.source !== 'manual' ? ', changed by you' : ''}
        {edited ? '. The claim was edited, so this binding needs confirming again.' : ''}
      </p>
      {hint && (
        <p className="gds-jc__disclaimer" data-testid={`${id}-ai-reason`}>
          AI's reason, as written: <span>{hint.reason}</span>
        </p>
      )}

      <div style={{ display: 'grid', gap: 6, gridTemplateColumns: 'max-content 1fr', alignItems: 'start' }}>
        <label htmlFor={`${id}-method`}>Method</label>
        <select id={`${id}-method`} data-testid={`${id}-method`} value={draft.method} onChange={(e) => onMethod(e.target.value)}>
          {catalogue.map((m) => (
            <option key={m.method} value={m.method}>{METHOD_NAMES[m.method] ?? m.method}</option>
          ))}
        </select>

        {entry && entry.variants.length > 0 && (
          <>
            <span>Variant</span>
            <div data-testid={`${id}-variant`}>
              {stated ? (
                <span data-testid={`${id}-variant-status`}>{sentenceFor({ code: draft.variant })}: {sentenceFor(draft.variantSetBy)}</span>
              ) : (
                <>
                  <span data-testid={`${id}-variant-status`}>
                    {draft.variantTicked ? `${sentenceFor({ code: draft.variantTicked })}: ${sentenceFor({ code: 'user_ticked' })}` : 'not stated'}
                  </span>
                  {hint?.variantHint && !draft.variantTicked && (
                    <span className="gds-jc__disclaimer"> (AI suggests {sentenceFor({ code: hint.variantHint })}; a suggestion is not the paper)</span>
                  )}
                  <div>
                    <label htmlFor={`${id}-variant-pick`}>Variant</label>{' '}
                    <select
                      id={`${id}-variant-pick`}
                      data-testid={`${id}-variant-pick`}
                      value={draft.variantPick ?? ''}
                      onChange={(e) => onChange((d) => ({ ...d, variantPick: e.target.value || null, variantTicked: d.variantTicked ? e.target.value || null : null }))}
                    >
                      <option value="">Choose…</option>
                      {entry.variants.map((v) => (
                        <option key={v} value={v}>{sentenceFor({ code: v })}</option>
                      ))}
                    </select>{' '}
                    <label>
                      <input
                        type="checkbox"
                        data-testid={`${id}-variant-tick`}
                        checked={draft.variantTicked !== null}
                        disabled={!draft.variantPick}
                        onChange={() => onChange((d) => ({ ...d, variantTicked: d.variantTicked ? null : d.variantPick }))}
                      />{' '}
                      the paper states this
                    </label>
                  </div>
                </>
              )}
            </div>
          </>
        )}

        {entry?.slots.map((slot) => {
          const chosen = draft.roles[slot.role] ?? [];
          const eligible = schema.filter((c) => (slot.kind === 'group' ? c.levels.length > 0 : c.columnType === 'numeric' || c.columnType === 'integer'));
          const col = chosen[0];
          const colView = schema.find((c) => alias(c.alias) === col);
          return (
            <React.Fragment key={slot.role}>
              <label htmlFor={`${id}-${slot.role}`}>{ROLE_NAMES[slot.role] ?? slot.role}</label>
              <div>
                {slot.kind === 'numeric_list' ? (
                  <select
                    id={`${id}-${slot.role}`}
                    data-testid={`${id}-${slot.role}`}
                    multiple
                    value={chosen.map(String)}
                    onChange={(e) => {
                      const v = Array.from(e.target.selectedOptions).map((o) => Number(o.value));
                      onChange((d) => ({ ...d, roles: { ...d.roles, [slot.role]: v } }));
                    }}
                  >
                    {eligible.map((c) => (
                      <option key={c.alias} value={alias(c.alias)}>{colName(c)}</option>
                    ))}
                  </select>
                ) : (
                  <select
                    id={`${id}-${slot.role}`}
                    data-testid={`${id}-${slot.role}`}
                    value={col === undefined ? '' : String(col)}
                    onChange={(e) => {
                      const a = e.target.value === '' ? [] : [Number(e.target.value)];
                      const lv = slot.kind === 'group' ? schema.find((c) => alias(c.alias) === a[0])?.levels.map((l) => l.code) : undefined;
                      onChange((d) => ({ ...d, roles: { ...d.roles, [slot.role]: a }, levels: lv ? { ...d.levels, [slot.role]: lv } : d.levels }));
                    }}
                  >
                    <option value="">Choose a column…</option>
                    {eligible.map((c) => (
                      <option key={c.alias} value={alias(c.alias)}>{colName(c)}</option>
                    ))}
                  </select>
                )}
                {slot.kind === 'group' && colView && (
                  <fieldset style={{ border: 'none', padding: 0, margin: '4px 0' }} data-testid={`${id}-${slot.role}-levels`}>
                    <legend>
                      Levels to include
                      {slot.levels ? ` (${slot.levels.code === 'exactly' ? 'exactly' : 'at least'} ${slot.levels.levels})` : ''}
                    </legend>
                    {colView.levels.map((l) => {
                      const on = (draft.levels[slot.role] ?? []).includes(l.code);
                      return (
                        <label key={l.code} style={{ marginRight: 10 }}>
                          <input
                            type="checkbox"
                            checked={on}
                            onChange={() =>
                              onChange((d) => {
                                const cur = d.levels[slot.role] ?? [];
                                return { ...d, levels: { ...d.levels, [slot.role]: on ? cur.filter((x) => x !== l.code) : [...cur, l.code] } };
                              })
                            }
                          />{' '}
                          {l.label ?? l.code} ({l.count})
                        </label>
                      );
                    })}
                  </fieldset>
                )}
              </div>
            </React.Fragment>
          );
        })}

        {entry?.needsTarget && (
          <>
            <label htmlFor={`${id}-target`}>Target</label>
            <select
              id={`${id}-target`}
              data-testid={`${id}-target`}
              value={draft.olsTarget === null ? '' : String(draft.olsTarget)}
              onChange={(e) => onChange((d) => ({ ...d, olsTarget: e.target.value === '' ? null : e.target.value === 'overall_f' ? 'overall_f' : Number(e.target.value) }))}
            >
              <option value="">Choose…</option>
              <option value="overall_f">The overall F</option>
              {(draft.roles.predictors ?? []).map((a) => (
                <option key={a} value={a}>The coefficient of {schema.find((c) => alias(c.alias) === a)?.header ?? `C${a}`}</option>
              ))}
            </select>
          </>
        )}

        <label htmlFor={`${id}-filter-col`}>Filter (optional)</label>
        <div>
          <select
            id={`${id}-filter-col`}
            data-testid={`${id}-filter-col`}
            value={draft.filter ? String(draft.filter.alias) : ''}
            onChange={(e) => onChange((d) => ({ ...d, filter: e.target.value === '' ? null : { alias: Number(e.target.value), levels: [] } }))}
          >
            <option value="">No filter</option>
            {groupCols.map((c) => (
              <option key={c.alias} value={alias(c.alias)}>Only rows where {c.alias} {c.header} is…</option>
            ))}
          </select>
          {draft.filter &&
            schema
              .find((c) => alias(c.alias) === draft.filter!.alias)
              ?.levels.map((l) => {
                const on = draft.filter!.levels.includes(l.code);
                return (
                  <label key={l.code} style={{ marginLeft: 10 }}>
                    <input
                      type="checkbox"
                      checked={on}
                      onChange={() =>
                        onChange((d) => ({
                          ...d,
                          filter: d.filter && { ...d.filter, levels: on ? d.filter.levels.filter((x) => x !== l.code) : [...d.filter.levels, l.code] },
                        }))
                      }
                    />{' '}
                    {l.label ?? l.code}
                  </label>
                );
              })}
        </div>
      </div>

      {ai && (
        <div style={{ display: 'flex', gap: 8, marginTop: 6 }}>
          <FocusButton variant="secondary" ref={methodBtn} onClick={() => ai.method(methodBtn.current)} data-testid={`${id}-ai-method`}>
            Suggest method with AI
          </FocusButton>
          <FocusButton variant="secondary" ref={columnsBtn} onClick={() => ai.columns(columnsBtn.current)} data-testid={`${id}-ai-columns`}>
            Suggest columns with AI
          </FocusButton>
        </div>
      )}
      {children}
    </section>
  );
};

export default L1Flow;
