// Gaply — the fingerprint screen (Prompt 5 item 10) and the checklist display
// (item 12).
//
// §7's three kinds of knowledge must never be mixed, so they are three
// sections that LOOK different, not three filters over one table:
//
//   Requirements  rules      — a value, its article type, its source sentence
//   Conventions   distributions — a median with an IQR and n, and ITS UNIT
//   Expectations  cited claims  — a quoted claim with the page it came from
//
// Nothing here is a model's opinion. Every line renders a row the backend
// stored, and `journal_display_is_llm_free.rs` asserts the read path cannot
// reach one.
import React from 'react';
import {
  Conflict,
  ConventionView,
  ExpectationView,
  FactStatus,
  JournalFingerprint,
  RequirementView,
  StandardBindingView,
  articleTypeText,
} from './fingerprintTypes';
import './journalFingerprint.css';

function StatusChip({ status }: { status: FactStatus | 'inferred' | 'unavailable' }) {
  return (
    <span className={`jf-status jf-status--${status}`} data-testid={`status-${status}`}>
      {status.toUpperCase()}
    </span>
  );
}

/** The source sentence, WHOLE. A truncated span is not a span — its purpose is
 *  to be checkable against the live page, and a clipped quote cannot be. */
function Span({ text, url, heading }: { text: string; url: string; heading?: string }) {
  return (
    <figure className="jf-span">
      <blockquote cite={url}>{text}</blockquote>
      <figcaption>
        {/* Links to the source DOCUMENT and heading, not the journal homepage. */}
        <a href={url} target="_blank" rel="noreferrer noopener">
          {url}
        </a>
        {heading ? <span className="jf-heading"> · {heading}</span> : null}
      </figcaption>
    </figure>
  );
}

function RequirementLine({ r }: { r: RequirementView }) {
  return (
    <li className="jf-line jf-line--requirement">
      <div className="jf-line__head">
        <span className="jf-kind">{r.kind.replace(/_/g, ' ')}</span>
        <span className="jf-value">{r.value}</span>
        <span className="jf-articletype" data-testid="article-type">
          {articleTypeText(r.article_type)}
        </span>
        <StatusChip status={r.status} />
      </div>
      <Span text={r.source_span} url={r.source_url} heading={r.source_heading} />
    </li>
  );
}

/**
 * **A CONFLICTED fact shows both values and chooses neither.**
 *
 * There is no "preferred" or "most recent" styling and no ordering that
 * implies one is right: both sides render identically, each with its own
 * span, so the reader sees two pages disagreeing and decides.
 */
function ConflictLine({ c }: { c: Conflict }) {
  return (
    <li className="jf-line jf-line--conflict" data-testid="conflict">
      <div className="jf-line__head">
        <span className="jf-kind">{c.kind.replace(/_/g, ' ')}</span>
        <span className="jf-articletype">{articleTypeText(c.article_type)}</span>
        <StatusChip status="conflicted" />
      </div>
      <p className="jf-conflict__note">
        Two of this journal&rsquo;s pages disagree. Both are shown; Gaply does not choose.
      </p>
      <div className="jf-conflict__sides">
        {c.values.map((v, i) => (
          <div className="jf-conflict__side" key={`${v.source_url}-${i}`} data-testid="conflict-side">
            <span className="jf-value">{v.value}</span>
            <Span text={v.source_span} url={v.source_url} heading={v.source_heading} />
          </div>
        ))}
      </div>
    </li>
  );
}

/** A convention is a DISTRIBUTION, and it renders with its unit and its n. */
function ConventionLine({ c }: { c: ConventionView }) {
  if (c.status === 'unavailable') {
    return (
      <li className="jf-line jf-line--convention" data-testid="convention">
        <div className="jf-line__head">
          <span className="jf-kind">{c.metric.replace(/_/g, ' ')}</span>
          <StatusChip status="unavailable" />
        </div>
        <p className="jf-unavailable">{c.detail || 'Searched; the source does not supply this.'}</p>
      </li>
    );
  }
  return (
    <li className="jf-line jf-line--convention" data-testid="convention">
      <div className="jf-line__head">
        <span className="jf-kind">{c.metric.replace(/_/g, ' ')}</span>
        <StatusChip status="inferred" />
      </div>
      <div className="jf-distribution">
        <span className="jf-median">
          median {c.median ?? '—'}
          {c.iqr_low !== null && c.iqr_high !== null ? ` (IQR ${c.iqr_low}–${c.iqr_high})` : ''}
        </span>
        <span className="jf-n">n = {c.n}</span>
      </div>
      {/* The unit and any scope caveat live in `detail` and are never dropped. */}
      {c.detail ? <p className="jf-detail">{c.detail}</p> : null}
      <p className="jf-notarule">What recent papers did — not a rule.</p>
    </li>
  );
}

function ExpectationLine({ e }: { e: ExpectationView }) {
  return (
    <li className="jf-line jf-line--expectation" data-testid="expectation">
      <div className="jf-line__head">
        <span className="jf-claim">{e.claim}</span>
        <StatusChip status={e.status} />
      </div>
      {e.frequency_k !== null && e.frequency_n !== null ? (
        <span className="jf-frequency">
          {e.frequency_k} of {e.frequency_n}
        </span>
      ) : null}
      <Span text={e.source_span} url={e.source_url} />
    </li>
  );
}

function StandardLine({ s }: { s: StandardBindingView }) {
  return (
    <li className="jf-line jf-line--requirement" data-testid="standard">
      <div className="jf-line__head">
        <span className="jf-kind">{s.standard}</span>
        <span className="jf-value">{s.design}</span>
      </div>
      <Span text={s.source_span} url={s.source_url} />
    </li>
  );
}

export function fetchedAtText(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toISOString().slice(0, 10);
}

export function JournalFingerprintView({ fp }: { fp: JournalFingerprint }) {
  if (!fp.provenance) {
    return (
      <section className="jf" data-testid="fingerprint-empty">
        <p className="jf-empty">
          No guidelines have been ingested for this journal. An analysis can still run, but it
          will be <strong>structural only</strong> — nothing in it will come from this
          journal&rsquo;s own instructions.
        </p>
      </section>
    );
  }
  const p = fp.provenance;
  return (
    <section className="jf">
      {/* Item 11: which fingerprint, fetched when — on the screen and in the report. */}
      <header className="jf-provenance" data-testid="provenance">
        Fingerprint v{p.version} · fetched {fetchedAtText(p.fetched_at)} · {p.source_count} source
        {p.source_count === 1 ? '' : 's'}
        {p.quarantine_reason ? (
          <strong className="jf-quarantine" data-testid="quarantine">
            {' '}
            QUARANTINED — {p.quarantine_reason}
          </strong>
        ) : null}
      </header>

      <div className="jf-section jf-section--requirements" data-testid="section-requirements">
        <h3>Requirements</h3>
        <p className="jf-section__kind">Rules the journal states. Violation is blocking.</p>
        <ul>
          {fp.conflicts.map((c, i) => (
            <ConflictLine c={c} key={`c${i}`} />
          ))}
          {fp.requirements.map((r, i) => (
            <RequirementLine r={r} key={`r${i}`} />
          ))}
          {fp.standards.map((s, i) => (
            <StandardLine s={s} key={`s${i}`} />
          ))}
        </ul>
      </div>

      <div className="jf-section jf-section--conventions" data-testid="section-conventions">
        <h3>Conventions</h3>
        <p className="jf-section__kind">
          What this journal&rsquo;s recently published papers did. A comparison, with its count
          and its unit — never a rule.
        </p>
        <ul>
          {fp.conventions.map((c, i) => (
            <ConventionLine c={c} key={`v${i}`} />
          ))}
        </ul>
      </div>

      <div className="jf-section jf-section--expectations" data-testid="section-expectations">
        <h3>Expectations</h3>
        <p className="jf-section__kind">
          What reviewers are asked to look for, quoted from a public page. A frequency with its
          evidence — never a rule.
        </p>
        <ul>
          {fp.expectations.map((e, i) => (
            <ExpectationLine e={e} key={`e${i}`} />
          ))}
        </ul>
      </div>
    </section>
  );
}
