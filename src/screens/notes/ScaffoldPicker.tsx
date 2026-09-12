// Gaply — Research Paper Writer, Set C: the venue-scaffold picker.
// Honesty is the whole point: every scaffold is labelled "…-style (unofficial)",
// carries a "Verify current requirements ↗" link to the publisher's real author
// page, and the picker shows the persistent notice that these are Gaply's own
// structures, not official templates. NO logos, NO copied template text, NO
// compliance claims.
import React from 'react';
import { Scaffold } from './manuscriptModel';

interface ScaffoldPickerProps {
  scaffolds: Scaffold[];
  notice: string;
  currentId?: string;      // highlighted when switching an existing manuscript
  title?: string;
  confirmLabel?: string;   // e.g. "Start writing" (new) or "Switch structure"
  onPick: (scaffold: Scaffold) => void;
  onCancel?: () => void;
}

const ScaffoldPicker: React.FC<ScaffoldPickerProps> = ({ scaffolds, notice, currentId, title, confirmLabel, onPick, onCancel }) => (
  <div className="an-scaffold-picker" data-testid="scaffold-picker">
    <div className="an-sp-head">
      <h2>{title ?? 'Choose a structure'}</h2>
      {onCancel && <button className="an-ghostbtn" data-testid="sp-cancel" onClick={onCancel}>Cancel</button>}
    </div>

    {/* The persistent honesty notice (rule 3) — never dismissed. */}
    <p className="an-sp-notice" data-testid="sp-notice">{notice}</p>

    <div className="an-sp-grid" data-testid="sp-grid">
      {scaffolds.map((s) => (
        <div key={s.id} className={`an-sp-card${s.id === currentId ? ' an-sp-card--current' : ''}`} data-testid={`sp-card-${s.id}`}>
          <div className="an-sp-card-head">
            <span className="an-sp-label">{s.label}</span>
            {s.unofficial && <span className="an-sp-badge" data-testid={`sp-unofficial-${s.id}`}>unofficial</span>}
          </div>
          <p className="an-sp-notice-text">{s.noticeText}</p>
          <div className="an-sp-meta">
            <span className="an-sp-style" title="Suggested reference style">Refs: {s.cslDefaultStyle}</span>
            {/* NO onClick HERE, and never add one that stops propagation.
                External links work because tauri-plugin-opener injects a click
                listener on WINDOW (init-iife.js) that catches <a target="_blank">
                and invokes plugin:opener|open_url. React dispatches from its root
                container, and a synthetic stopPropagation() also stops the NATIVE
                event there — so the shim never sees the click and the link does
                nothing at all, silently. This card has no click handler of its
                own, so the guard that used to sit here protected nothing and cost
                every scaffold its "verify" link. Measured 12 Sep 2026. */}
            <a
              className="an-sp-verify"
              href={s.publisherAuthorUrl}
              target="_blank"
              rel="noreferrer noopener"
              data-testid={`sp-verify-${s.id}`}
            >
              Verify current requirements ↗
            </a>
          </div>
          <button className="an-sp-pick" data-testid={`sp-pick-${s.id}`} onClick={() => onPick(s)}>
            {s.id === currentId ? 'Keep this structure' : (confirmLabel ?? 'Use this structure')}
          </button>
        </div>
      ))}
    </div>
  </div>
);

export default ScaffoldPicker;
