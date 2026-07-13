// Gaply — the analysis-scoped chat dock. Mirrors PublishReady's CopilotDock but
// is wired to the Set 4 stats-chat (run_stats_chat), scoped to THIS analysis's
// VerificationReport. Every answer is ADVISORY (the backend turn is advisory by
// type); the firewall refuses off-topic / code-authoring in the backend and the
// refusal is shown honestly. The chat can NEVER source a verified number — those
// are the engine's, echoed only in the report above.
import React, { useRef, useState } from 'react';
import { Badge, Button } from '../../design-system';
import { StatsVerifierBridge } from './statsVerifierBridge';
import { AnalysisSpec, StatsChatTurn } from './statsVerifierTypes';
import {
  detectGhostwriting,
  GHOSTWRITE_BLOCK_MESSAGE,
  isGhostwritingRequest,
  refusalFor,
} from '../copilot/chatGuards';
import '../copilot/copilot.css';

const CHIPS = [
  'What does my result mean?',
  'Why is this a mismatch?',
  'Is my p-value borderline?',
  'What should I check next?',
];

interface Msg {
  role: 'user' | 'assistant';
  text: string;
  kind?: StatsChatTurn['kind'];
  disclaimer?: string;
}

export interface StatsChatDockProps {
  bridge: StatsVerifierBridge;
  path: string;
  spec: AnalysisSpec;
  manuscriptPath?: string;
  language?: string;
  userToken?: string;
}

const StatsChatDock: React.FC<StatsChatDockProps> = ({ bridge, path, spec, manuscriptPath, language = 'en', userToken }) => {
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<Msg[]>([
    {
      role: 'assistant',
      text:
        'Ask me about THIS analysis — in any language. I interpret your already-computed results (I can’t recompute or write your code).',
      kind: 'answered',
    },
  ]);
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);

  const send = async (raw: string) => {
    const question = raw.trim();
    if (!question || busy) return;
    setMessages((m) => [...m, { role: 'user', text: question }]);
    setInput('');

    // REQUEST guard (client backstop, mirrors ResearchCopilotPanel) — refuse a
    // manuscript-ghostwriting request up front, before any cloud call. The backend
    // run_stats_chat firewall remains the real boundary (it also catches
    // code-authoring); this is defense-in-depth so the client has the same guard.
    if (isGhostwritingRequest(question)) {
      setMessages((m) => [...m, { role: 'assistant', text: refusalFor(question), kind: 'refused_ghostwriting' }]);
      return;
    }

    setBusy(true);
    try {
      const turn = await bridge.chat({ path, spec, manuscriptPath, question, language, userToken });
      // RESPONSE guard (client backstop, mirrors ResearchCopilotPanel) — block
      // drafted manuscript prose even if the model drifts.
      if (detectGhostwriting(turn.answer).blocked) {
        setMessages((m) => [...m, { role: 'assistant', text: GHOSTWRITE_BLOCK_MESSAGE, kind: 'blocked_ghostwriting' }]);
      } else {
        setMessages((m) => [
          ...m,
          { role: 'assistant', text: turn.answer, kind: turn.kind, disclaimer: turn.advisory ? turn.disclaimer : undefined },
        ]);
      }
    } catch {
      setMessages((m) => [...m, { role: 'assistant', text: 'The copilot is unavailable right now.', kind: 'unavailable' }]);
    } finally {
      setBusy(false);
      requestAnimationFrame(() => logRef.current?.scrollTo?.({ top: 1e9 }));
    }
  };

  return (
    <div style={{ borderTop: '1px solid var(--g-border)', background: 'var(--g-bg-layer1)' }} data-testid="sv-copilot-dock">
      <button
        onClick={() => setOpen((o) => !o)}
        data-testid="sv-copilot-toggle"
        style={{ width: '100%', textAlign: 'left', padding: '10px 16px', background: 'none', border: 'none', color: 'var(--g-text-2)', cursor: 'pointer', fontWeight: 600 }}
      >
        {open ? '▾' : '▸'} Stats Copilot ★ — ask about this analysis
      </button>
      {open && (
        <div style={{ height: 360, padding: '0 16px 16px' }}>
          <div className="gds-chat" data-testid="sv-copilot">
            <div className="gds-chat__log" ref={logRef} data-testid="sv-chat-log">
              {messages.map((m, i) => (
                <div
                  key={i}
                  className={`gds-chat__msg gds-chat__msg--${m.role}${m.kind === 'refused_ghostwriting' ? ' gds-chat__msg--refused' : ''}${m.kind === 'blocked_ghostwriting' ? ' gds-chat__msg--blocked' : ''}`}
                  data-testid={`sv-msg-${m.role}${m.kind === 'refused_ghostwriting' ? '-refused' : m.kind === 'blocked_ghostwriting' ? '-blocked' : ''}`}
                >
                  <div className="gds-chat__text">{m.text}</div>
                  {m.role === 'assistant' && m.kind === 'answered' && m.disclaimer && (
                    <div className="gds-chat__disclaimer" data-testid={`sv-msg-disclaimer-${i}`}>
                      <Badge status="assessed">advisory</Badge> {m.disclaimer}
                    </div>
                  )}
                </div>
              ))}
            </div>
            <div style={{ display: 'grid', gap: 8 }}>
              <div className="gds-chat__chips">
                {CHIPS.map((c) => (
                  <button key={c} className="gds-chat__chip" data-testid="sv-chip" onClick={() => send(c)}>
                    {c}
                  </button>
                ))}
              </div>
              <div className="gds-chat__input-row">
                <input
                  className="gds-chat__input"
                  placeholder="Ask in any language…"
                  value={input}
                  data-testid="sv-chat-input"
                  onChange={(e) => setInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && send(input)}
                />
                <Button onClick={() => send(input)} disabled={busy} data-testid="sv-chat-send">
                  Send
                </Button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default StatsChatDock;
