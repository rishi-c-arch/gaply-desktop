// Gaply — Research Copilot chat panel (F11). Educates & guides; structurally
// incapable of writing the paper (request + response guards). Multilingual.
// Grounds every answer in findings/RAG with provenance + the uncertainty
// disclaimer. Docks to the PublishReady report and is also a standalone route.
import React, { useRef, useState } from 'react';
import { Button } from '../../design-system';
import { ChatClient } from './chatBridge';
import { buildChatPayload, ChatContext } from './chatContext';
import {
  detectGhostwriting,
  GHOSTWRITE_BLOCK_MESSAGE,
  isGhostwritingRequest,
  refusalFor,
} from './chatGuards';
import { detectLanguage, LANGUAGE_NAME } from './detectLanguage';
import './copilot.css';

const CHIPS = [
  'Why was this flagged?',
  'How do I improve my methods section?',
  'What papers should I read to strengthen this?',
  'Will this pass Q1 review?',
];

interface Msg {
  role: 'user' | 'assistant';
  text: string;
  language?: string;
  provenance?: string[];
  aiAssessed?: boolean;
  kind?: 'answer' | 'refused' | 'blocked';
}

export interface ResearchCopilotPanelProps {
  context: ChatContext;
  client: ChatClient;
}

const ResearchCopilotPanel: React.FC<ResearchCopilotPanelProps> = ({ context, client }) => {
  const [messages, setMessages] = useState<Msg[]>([
    {
      role: 'assistant',
      text: 'Ask me anything about your analysis — in any language. I explain and teach, but I won’t write your paper for you.',
      kind: 'answer',
    },
  ]);
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);

  const send = async (raw: string) => {
    const question = raw.trim();
    if (!question || busy) return;
    const language = detectLanguage(question);
    setMessages((m) => [...m, { role: 'user', text: question, language }]);
    setInput('');

    // REQUEST guard — refuse ghostwriting up front (no cloud call).
    if (isGhostwritingRequest(question)) {
      setMessages((m) => [...m, { role: 'assistant', text: refusalFor(question), language, kind: 'refused' }]);
      return;
    }

    setBusy(true);
    try {
      const payload = buildChatPayload(context, question, language);
      const res = await client.ask(payload);
      // RESPONSE guard — block ghostwritten prose even if the model drifts.
      const gw = detectGhostwriting(res.answer);
      if (gw.blocked) {
        setMessages((m) => [...m, { role: 'assistant', text: GHOSTWRITE_BLOCK_MESSAGE, language, kind: 'blocked' }]);
      } else {
        setMessages((m) => [
          ...m,
          { role: 'assistant', text: res.answer, language, provenance: res.provenance, aiAssessed: res.aiAssessed, kind: 'answer' },
        ]);
      }
    } catch (e) {
      setMessages((m) => [...m, { role: 'assistant', text: 'The copilot is unavailable right now.', kind: 'answer' }]);
    } finally {
      setBusy(false);
      requestAnimationFrame(() => logRef.current?.scrollTo?.({ top: 1e9 }));
    }
  };

  return (
    <div className="gds-chat" data-testid="copilot">
      <div className="gds-chat__log" ref={logRef} data-testid="chat-log">
        {messages.map((m, i) => (
          <div
            key={i}
            className={`gds-chat__msg gds-chat__msg--${m.role}${m.kind === 'refused' ? ' gds-chat__msg--refused' : ''}${m.kind === 'blocked' ? ' gds-chat__msg--blocked' : ''}`}
            data-testid={`msg-${m.role}${m.kind === 'refused' ? '-refused' : m.kind === 'blocked' ? '-blocked' : ''}`}
          >
            {m.language && m.role === 'user' && (
              <span className="gds-chat__lang">{LANGUAGE_NAME[m.language] ?? m.language}</span>
            )}
            <div className="gds-chat__text">{m.text}</div>
            {m.provenance && m.provenance.length > 0 && (
              <div className="gds-chat__prov" data-testid="msg-provenance">
                {m.provenance.slice(0, 4).map((p, j) => <span key={j}>{p}</span>)}
              </div>
            )}
            {m.aiAssessed && (
              <div className="gds-chat__disclaimer" data-testid="msg-disclaimer">
                AI-assessed guidance — a statistical signal, not definitive proof.
              </div>
            )}
          </div>
        ))}
      </div>

      <div style={{ display: 'grid', gap: 8 }}>
        <div className="gds-chat__chips">
          {CHIPS.map((c) => (
            <button key={c} className="gds-chat__chip" data-testid={`chip`} onClick={() => send(c)}>{c}</button>
          ))}
        </div>
        <div className="gds-chat__input-row">
          <input
            className="gds-chat__input"
            placeholder="Ask in any language…"
            value={input}
            data-testid="chat-input"
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && send(input)}
          />
          <Button onClick={() => send(input)} disabled={busy} data-testid="chat-send">Send</Button>
        </div>
      </div>
    </div>
  );
};

export default ResearchCopilotPanel;
