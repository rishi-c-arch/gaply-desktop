// Gaply — the journal picker (Prompt 5 item 9).
//
// Pick one of the ten profiled journals or paste a guidelines URL. **Every
// number on this screen is known before the run starts**, which is the item's
// requirement: a user sees what the checklist will be built from rather than
// discovering it in the report.
import React from 'react';
import { JournalProfileRow } from './fingerprintTypes';
import { fetchedAtText } from './JournalFingerprintView';
import './journalFingerprint.css';
import './journalPicker.css';

export interface JournalPickerProps {
  profiles: JournalProfileRow[];
  selectedKey: string | null;
  onSelect(key: string): void;
  pastedUrl: string;
  onPastedUrlChange(url: string): void;
  /** What ingestion found for a pasted URL, once it has been ingested.
   *  `null` before any ingestion has been attempted. */
  pastedResult: JournalProfileRow | null;
  onProceedStructuralOnly?(): void;
}

/**
 * The line that tells a user what a run will be built from.
 *
 * **"N requirements extracted, K by pattern, M by model" (item 9), and the
 * model count is shown even though it is always 0 today.** Omitting a zero
 * would hide the day it stops being zero, and the pattern/model split is what
 * §3.4 asks to be reported per journal.
 */
export function IngestionSummary({ row }: { row: JournalProfileRow }) {
  if (!row.ingested) {
    return (
      <p className="jp-summary jp-summary--empty" data-testid="ingestion-empty">
        Nothing ingested for this journal. An analysis can still run, but it will be{' '}
        <strong>structural only</strong> — no requirement in it will come from this
        journal&rsquo;s own instructions.
      </p>
    );
  }
  return (
    <p className="jp-summary" data-testid="ingestion-summary">
      <strong>{row.requirement_count}</strong> requirement
      {row.requirement_count === 1 ? '' : 's'} extracted
      {' — '}
      <span data-testid="split">
        {row.by_pattern} by pattern, {row.by_model} by model
      </span>
      {row.conflict_count > 0 ? (
        <span className="jp-conflicts" data-testid="conflict-count">
          {' · '}
          {row.conflict_count} conflicted
        </span>
      ) : null}
      {' · '}
      {row.standard_count} reporting standard{row.standard_count === 1 ? '' : 's'}
      {' · '}
      {row.convention_count} convention{row.convention_count === 1 ? '' : 's'}
      {' · '}
      {row.expectation_count} expectation{row.expectation_count === 1 ? '' : 's'}
      {row.version !== null && row.fetched_at !== null ? (
        <span className="jp-version">
          {' · '}v{row.version}, fetched {fetchedAtText(row.fetched_at)}
        </span>
      ) : null}
      {row.quarantine_reason ? (
        <strong className="jf-quarantine"> · QUARANTINED — {row.quarantine_reason}</strong>
      ) : null}
    </p>
  );
}

export function JournalPicker(props: JournalPickerProps) {
  const {
    profiles,
    selectedKey,
    onSelect,
    pastedUrl,
    onPastedUrlChange,
    pastedResult,
    onProceedStructuralOnly,
  } = props;
  const selected = profiles.find((p) => p.key === selectedKey) ?? null;

  return (
    <section className="jp">
      <h3>Target journal</h3>
      <ul className="jp-list">
        {profiles.map((p) => (
          <li key={p.key}>
            <button
              type="button"
              className={`jp-item${p.key === selectedKey ? ' jp-item--on' : ''}`}
              aria-pressed={p.key === selectedKey}
              onClick={() => onSelect(p.key)}
              data-testid={`journal-${p.key}`}
            >
              <span className="jp-item__name">{p.name}</span>
              <span
                className={`jp-badge jp-badge--${p.ingested ? 'ingested' : 'none'}`}
                data-testid={`badge-${p.key}`}
              >
                {p.ingested ? `${p.requirement_count} req` : 'not ingested'}
              </span>
            </button>
          </li>
        ))}
      </ul>

      {selected ? <IngestionSummary row={selected} /> : null}
      {selected && !selected.ingested && onProceedStructuralOnly ? (
        <button type="button" className="jp-structural" onClick={onProceedStructuralOnly}>
          Proceed structural only
        </button>
      ) : null}

      <div className="jp-paste">
        <label htmlFor="jp-url">Or paste a guidelines URL for a journal not listed</label>
        <input
          id="jp-url"
          type="url"
          value={pastedUrl}
          placeholder="https://journal.example/author-guidelines"
          onChange={(e) => onPastedUrlChange(e.target.value)}
        />
        {pastedResult ? <IngestionSummary row={pastedResult} /> : null}
      </div>
    </section>
  );
}
