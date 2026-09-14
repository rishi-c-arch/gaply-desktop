// Gaply — the checklist display (Prompt 5 item 12).
//
// Each item shows WHICH REQUIREMENT it came from (with the journal's sentence)
// and WHICH FIELD it checked. An item with no journal behind it is labelled
// STRUCTURAL and looks different, so a reader never mistakes Gaply's own
// structural check for something the journal said.
//
// # A passed item is a passed item, not a finding
//
// The teardown's "6 findings, 3 of which were agents saying they had nothing
// to say" pattern is what this display refuses. Passes and failures are
// counted separately, passes are collapsed by default, and nothing on the
// passed side is styled as an issue.
import React from 'react';
import { ChecklistItem } from '../../report/reportTypes';
import './journalFingerprint.css';
import './checklistDisplay.css';

/** A structural item is one with no journal requirement behind it. */
export function isStructural(item: ChecklistItem): boolean {
  return !item.source_span && !item.guideline_source;
}

/** NOT STATED, never "All". Mirrors `articleTypeText` on the fingerprint side. */
export function checklistArticleType(item: ChecklistItem): string {
  const t = item.article_type;
  return t && t.trim() ? t : 'NOT STATED';
}

function Item({ item }: { item: ChecklistItem }) {
  const structural = isStructural(item);
  return (
    <li
      className={`cd-item${structural ? ' cd-item--structural' : ''}`}
      data-testid={item.passed ? 'checklist-pass' : 'checklist-fail'}
    >
      <div className="cd-item__head">
        <span className={`cd-mark cd-mark--${item.passed ? 'pass' : 'fail'}`}>
          {item.passed ? 'PASS' : 'FAIL'}
        </span>
        <span className="cd-requirement">{item.requirement}</span>
        {structural ? (
          <span className="cd-structural" data-testid="structural-label">
            STRUCTURAL
          </span>
        ) : (
          <span className="jf-articletype" data-testid="checklist-article-type">
            {checklistArticleType(item)}
          </span>
        )}
      </div>
      <p className="cd-detail">{item.detail}</p>
      {item.checked_field ? (
        <p className="cd-checked" data-testid="checked-field">
          checked: <code>{item.checked_field}</code>
        </p>
      ) : null}
      {item.source_span ? (
        <figure className="jf-span">
          {/* Whole, never clipped. */}
          <blockquote cite={item.guideline_source ?? undefined}>{item.source_span}</blockquote>
          {item.guideline_source ? (
            <figcaption>
              <a href={item.guideline_source} target="_blank" rel="noreferrer noopener">
                {item.guideline_source}
              </a>
            </figcaption>
          ) : null}
        </figure>
      ) : (
        <p className="cd-nojournal" data-testid="no-journal-source">
          Gaply&rsquo;s own structural check — this journal states no requirement here.
        </p>
      )}
    </li>
  );
}

export function ChecklistDisplay({ items }: { items: ChecklistItem[] }) {
  const failed = items.filter((i) => !i.passed);
  const passed = items.filter((i) => i.passed);
  return (
    <section className="cd">
      <header className="cd-counts" data-testid="checklist-counts">
        {/* Counted apart, and the passes are NOT called findings. */}
        <span className="cd-count cd-count--fail">
          {failed.length} unmet
        </span>
        <span className="cd-count cd-count--pass">{passed.length} met</span>
      </header>

      {failed.length > 0 ? (
        <ul className="cd-list">
          {failed.map((i, n) => (
            <Item item={i} key={`f${n}`} />
          ))}
        </ul>
      ) : null}

      {passed.length > 0 ? (
        <details className="cd-passed">
          <summary>{passed.length} met</summary>
          <ul className="cd-list">
            {passed.map((i, n) => (
              <Item item={i} key={`p${n}`} />
            ))}
          </ul>
        </details>
      ) : null}
    </section>
  );
}
