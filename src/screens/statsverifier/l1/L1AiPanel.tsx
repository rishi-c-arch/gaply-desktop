// Gaply — Level 1 AI help panel (Phase 2, Step 7). One panel per helper
// request: the exact request text, what it carries in plain words, the
// optional opt-ins, who receives it and what they keep, then Cancel or Send.
//
// Nothing is sent until Send is clicked. Cancel sends nothing. Each Send is
// the consent for this task and these categories (Rust makes a fresh grant
// from the previewed request on every Send). Withdraw revokes the task's
// grant and cancels every call or retry of this check in flight; the next
// Send gives consent again. The panel says exactly that (part C, M-S2).
//
// Every string from the model or the web is rendered as a React text child:
// no dangerouslySetInnerHTML, no markdown or HTML renderer, no link.
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '../../../design-system';
import type { StatsL1Bridge } from '../statsL1Bridge';
import type { HelperTaskInput, L1HelperResultView, L1PreviewView, OptInCategory } from '../statsL1Types';
import { refusalSentence, TASK_NAMES } from './l1Wording';

export const OPT_IN_LABELS: Record<OptInCategory, string> = {
  column_headers: 'Also send my original column names (each cut to 64 characters)',
  claim_sentence: "Also send the result's sentence from my manuscript",
  methods_passage: 'Also send a Methods passage I select (up to 1,000 characters)',
  paper_doi: "Also send the paper's DOI I type (a DOI only, such as 10.1234/abcd)",
};

/** The opt-ins each task offers, in the order Rust lists them. */
export const TASK_OPT_INS: Record<HelperTaskInput['task'], OptInCategory[]> = {
  claim_method: ['claim_sentence', 'methods_passage'],
  binding_suggestion: ['column_headers', 'claim_sentence'],
  result_explanation: [],
  web_context: ['paper_doi'],
};

/** What the request does NOT carry, given what is ticked (evaluation
 *  fixes, part C, M-S7). "Any row or cell", not "your data": the
 *  explanation's request carries values computed from the data. */
export function notSent(task: HelperTaskInput['task'], ticked: OptInCategory[]): string[] {
  const has = (c: OptInCategory) => ticked.includes(c);
  const out = ['any row or cell of your data', 'file names', 'level names'];
  // Ticking the headers sends every column's name (each cut to 64
  // characters), so then no column name is "not sent".
  if (task !== 'binding_suggestion' || !has('column_headers')) out.push('column names (unless ticked)');
  if (!has('claim_sentence') && !has('methods_passage')) out.push('any manuscript sentence or passage (unless ticked)');
  else out.push('any other manuscript text');
  if (task === 'web_context' && !has('paper_doi')) out.push("the paper's DOI");
  return out;
}

export const RECIPIENT_TEXT =
  'Sent to OpenAI over HTTPS, with your own API key. Gaply runs no server in between and sees nothing.';

export const RETENTION_TEXT =
  "The request sets store to false, which asks OpenAI not to keep the response for later retrieval. That does not mean nothing is kept: OpenAI keeps abuse-monitoring logs of API requests for up to 30 days, and its prompt cache may hold data for up to 24 hours. OpenAI says API data is not used to train its models unless you opt in.";

export const WEB_RETENTION_TEXT =
  "Web search sends the query on to the sites searched, whose own retention policies apply.";

/** What Withdraw does (evaluation fixes, part C, M-S2): every Send makes a
 *  new consent, and the cancel stops every request of this check. */
export const WITHDRAW_TEXT =
  'Withdrawing stops any request still in flight in this check. Nothing more is sent unless you click Send again, which gives consent again.';

export const WITHDRAWN_TEXT = 'Withdrawn: any request in flight was stopped. Nothing more is sent unless you click Send again.';

/** The search query inside a web-context preview: read from the request
 *  itself, so the field shows what will be sent. */
export function queryFromPreview(body: string): string | null {
  try {
    const outer = JSON.parse(body);
    const inner = typeof outer.input === 'string' ? JSON.parse(outer.input) : outer.input;
    return typeof inner?.query === 'string' ? inner.query : null;
  } catch {
    return null;
  }
}

export interface L1AiPanelProps {
  bridge: StatsL1Bridge;
  run: number;
  /** The request's input without the opt-in text; the panel adds the passage
   *  or the DOI the user types. */
  input: HelperTaskInput;
  title?: string;
  /** Rendered above the request, for a panel with its own controls (the
   *  method context's pickers and read-only query). */
  children?: React.ReactNode;
  onResult: (r: L1HelperResultView) => void;
  /** Each preview built, so a panel's own controls can show what it holds. */
  onPreview?: (p: L1PreviewView) => void;
  /** Every request attempt, sent or refused, so the usage line can refresh. */
  onSettled?: () => void;
  onClose: () => void;
}

type Phase = 'preview' | 'sending' | 'sent';

export const L1AiPanel: React.FC<L1AiPanelProps> = ({ bridge, run, input, title, children, onResult, onPreview, onSettled, onClose }) => {
  const [ticked, setTicked] = useState<OptInCategory[]>([]);
  const [passage, setPassage] = useState('');
  const [doi, setDoi] = useState('');
  const [preview, setPreview] = useState<L1PreviewView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>('preview');
  const [withdrawn, setWithdrawn] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const task = input.task;

  useEffect(() => {
    headingRef.current?.focus();
  }, []);

  // The request with the user's opt-in text in it: the passage or the DOI
  // goes in only when its box is ticked.
  const fullInput = useCallback((): HelperTaskInput => {
    if (input.task === 'claim_method') {
      return { ...input, methodsPassage: ticked.includes('methods_passage') ? passage : null };
    }
    if (input.task === 'web_context') {
      return { ...input, doi: ticked.includes('paper_doi') ? doi : null };
    }
    return input;
  }, [input, ticked, passage, doi]);

  const rebuild = useCallback(async () => {
    setError(null);
    setPreview(null);
    // A ticked text box that is still empty has nothing to preview yet.
    if ((ticked.includes('methods_passage') && !passage.trim()) || (ticked.includes('paper_doi') && !doi.trim())) {
      setError('Type the text for the ticked item, then update the preview.');
      return;
    }
    try {
      const p = await bridge.helperPreview(run, fullInput(), ticked);
      setPreview(p);
      onPreview?.(p);
    } catch (e) {
      setError(refusalSentence(e));
    }
    // fullInput captures ticked, passage and doi.
  }, [bridge, run, fullInput, ticked, passage, doi, onPreview]);

  // The preview is rebuilt whenever a box is ticked or the input changes.
  // Text in a box is applied with its own button, so typing does not call
  // Rust on every key.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { void rebuild(); }, [ticked, input]);

  const toggle = (c: OptInCategory) => setTicked((t) => (t.includes(c) ? t.filter((x) => x !== c) : [...t, c]));

  const send = async () => {
    if (!preview) return;
    setPhase('sending');
    setError(null);
    try {
      const r = await bridge.helperSend(run, preview.preview);
      setPhase('sent');
      onResult(r);
    } catch (e) {
      setPhase('preview');
      setError(refusalSentence(e));
    } finally {
      onSettled?.();
    }
  };

  const withdraw = async () => {
    try {
      await bridge.helperRevoke(run, task);
      await bridge.helperCancel(run);
      setWithdrawn(true);
    } catch (e) {
      setError(refusalSentence(e));
    } finally {
      onSettled?.();
    }
  };

  const id = `l1-ai-${task}`;
  return (
    <section className="gds-card" aria-labelledby={`${id}-h`} data-testid={`l1-ai-panel-${task}`} style={{ marginTop: 8 }}>
      <h4 id={`${id}-h`} ref={headingRef} tabIndex={-1} style={{ marginTop: 0 }}>
        {title ?? TASK_NAMES[task]}
      </h4>
      {children}

      {TASK_OPT_INS[task].length > 0 && (
        <fieldset style={{ border: 'none', padding: 0, margin: '8px 0' }}>
          <legend style={{ fontWeight: 600 }}>Optional, off unless you tick them</legend>
          {TASK_OPT_INS[task].map((c) => (
            <div key={c}>
              <label>
                <input
                  type="checkbox"
                  checked={ticked.includes(c)}
                  onChange={() => toggle(c)}
                  disabled={phase !== 'preview'}
                  data-testid={`l1-optin-${c}`}
                />{' '}
                {OPT_IN_LABELS[c]}
              </label>
              {c === 'methods_passage' && ticked.includes(c) && (
                <div style={{ marginTop: 4 }}>
                  <label htmlFor={`${id}-passage`}>Methods passage (paste it from your manuscript)</label>
                  <textarea
                    id={`${id}-passage`}
                    data-testid="l1-methods-passage"
                    maxLength={1000}
                    rows={4}
                    style={{ width: '100%' }}
                    value={passage}
                    onChange={(e) => setPassage(e.target.value)}
                  />
                  <span className="gds-jc__disclaimer">{passage.length} of 1,000 characters</span>{' '}
                  <Button variant="secondary" onClick={() => void rebuild()} data-testid="l1-passage-apply">
                    Update the preview
                  </Button>
                </div>
              )}
              {c === 'paper_doi' && ticked.includes(c) && (
                <div style={{ marginTop: 4 }}>
                  <label htmlFor={`${id}-doi`}>DOI</label>{' '}
                  <input id={`${id}-doi`} data-testid="l1-doi" value={doi} maxLength={200} onChange={(e) => setDoi(e.target.value)} />{' '}
                  <Button variant="secondary" onClick={() => void rebuild()} data-testid="l1-doi-apply">
                    Update the preview
                  </Button>
                </div>
              )}
            </div>
          ))}
        </fieldset>
      )}

      {preview && (
        <>
          <p data-testid="l1-ai-sent">
            <strong>Sent:</strong> {preview.categories.join('; ')}.
          </p>
          <p data-testid="l1-ai-not-sent">
            <strong>Not sent:</strong> {notSent(task, ticked).join(', ')}.
          </p>
          <p style={{ fontWeight: 600, marginBottom: 2 }}>The exact request</p>
          <pre
            data-testid="l1-ai-request"
            tabIndex={0}
            aria-label="The exact request"
            style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word', maxHeight: 240, overflow: 'auto', fontSize: 12 }}
          >
            {preview.body}
          </pre>
        </>
      )}
      <p className="gds-jc__disclaimer" data-testid="l1-ai-recipient">{RECIPIENT_TEXT}</p>
      <p className="gds-jc__disclaimer" data-testid="l1-ai-retention">
        {RETENTION_TEXT}
        {task === 'web_context' ? ` ${WEB_RETENTION_TEXT}` : ''}
      </p>
      {error && <p role="alert" style={{ color: 'var(--g-flagged)' }} data-testid="l1-ai-error">{error}</p>}

      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        {phase === 'preview' && (
          <>
            <Button variant="secondary" onClick={onClose} data-testid="l1-ai-cancel">Cancel</Button>
            <Button onClick={() => void send()} disabled={!preview} data-testid="l1-ai-send">Send</Button>
          </>
        )}
        {phase === 'sending' && <span aria-live="polite">Sending…</span>}
        {phase !== 'preview' && (
          <>
            <Button variant="secondary" onClick={() => void withdraw()} disabled={withdrawn} data-testid="l1-ai-withdraw">
              Withdraw consent
            </Button>
            <Button variant="ghost" onClick={onClose} data-testid="l1-ai-close">Close</Button>
          </>
        )}
      </div>
      {phase !== 'preview' && (
        <p className="gds-jc__disclaimer" data-testid="l1-ai-withdraw-text">
          {withdrawn ? WITHDRAWN_TEXT : WITHDRAW_TEXT}
        </p>
      )}
    </section>
  );
};

export default L1AiPanel;
