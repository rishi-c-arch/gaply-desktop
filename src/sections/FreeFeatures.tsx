import React from 'react';
import { useNavigate } from 'react-router-dom';
import '../styles/variables.css';
import '../styles/animations.css';
import '../styles/free-features-new.css';

/** The genuinely-FREE desktop features (no entitlement gate — confirmed against
 *  tiers.ts + the useEntitlement gates). Copy is grounded in what each feature
 *  actually does: signals/indicators, NEVER automated verdicts; exact-match vs
 *  the user's OWN corpus (not the internet); "validation" ≠ the paid Statistical
 *  Analysis Verifier; "Journal Check" (evidence signals) ≠ the paid Journal
 *  Verification (LLM site-summary). No dormant/coming-soon lanes advertised.
 *  All ship in the desktop app → each CTA goes to /download. */
const FREE_CARDS = [
  {
    chipClass: 'free2-chip--cyan',
    chip: 'On-device',
    icon: 'smart_toy',
    title: 'AI Detection',
    module: 'MODULE_01',
    body:
      'Analyzes AI-writing signals using perplexity and burstiness indicators. Results are evidence for human review—not proof that text was or wasn\'t AI-generated. Runs entirely on your device.',
  },
  {
    chipClass: 'free2-chip--emerald',
    chip: 'On-device',
    icon: 'plagiarism',
    title: 'Plagiarism Check',
    module: 'MODULE_02',
    body:
      'Exact-match, verbatim overlap against your own paper library, plus self-plagiarism within a draft. An indicator, not Turnitin — it checks your corpus, never the whole internet. Fully on-device.',
  },
  {
    chipClass: 'free2-chip--violet',
    chip: 'LLM-free',
    icon: 'rule',
    title: 'Statistical Validation',
    module: 'MODULE_03',
    body:
      'A deterministic, LLM-free sanity-check that flags statistical issues in your manuscript — test/group mismatches, missing effect sizes, and more. Fully on-device.',
  },
  {
    chipClass: 'free2-chip--cyan',
    chip: 'Local + lookup',
    icon: 'library_books',
    title: 'Citation Manager',
    module: 'MODULE_04',
    body:
      'In the desktop app: a local-first reference library with CSL citation styles, verified metadata lookup (DOI / CrossRef / OpenAlex), a citation-hallucination check, and one-click exports. Your library stays on your device; lookups use the web.',
  },
  {
    chipClass: 'free2-chip--emerald',
    chip: 'Offline',
    icon: 'edit_note',
    title: 'Note Creator',
    module: 'MODULE_05',
    body:
      "Own-words, per-paper note templates plus quick project notes and unified search — with an optional side-by-side of your paper's full text. Fully offline; your notes never leave your device.",
  },
  {
    chipClass: 'free2-chip--violet',
    chip: 'Evidence',
    icon: 'travel_explore',
    title: 'Journal Check',
    module: 'MODULE_06',
    body:
      "Evidence signals on a journal — indexing and 'well-indexed' markers vs. warning-signs to investigate. Signals to weigh, not a predatory verdict.",
  },
];

export default function FreeFeatures() {
  const navigate = useNavigate();
  return (
    <section className="free2-section" aria-labelledby="free2-heading">
      {/* Giant 02 index background */}
      <div className="free2-index-bg" aria-hidden="true">
        <span className="free2-index-text">02</span>
      </div>

      {/* Subtle grid overlay */}
      <div className="free2-grid" aria-hidden="true" />

      <div className="free2-inner">
        {/* Left column – copy */}
        <div className="free2-left">
          <div className="free2-status-row">
            <span className="free2-status-dot" />
            <span className="free2-status-label">SYSTEM ONLINE</span>
            <span className="free2-status-line" />
            <span className="free2-status-version">V.2.0.4</span>
          </div>

          <h2 id="free2-heading" className="free2-heading">
            Discover <br />
            <span className="free2-heading-highlight">Free Features</span>
          </h2>

          <p className="free2-body">
            Free, unlimited research tools — most run entirely on your device. They surface signals and
            indicators for your judgement, never automated verdicts.
          </p>

          <div className="free2-scroll-hint" aria-hidden="true">
            <div className="free2-scroll-shell">
              <div className="free2-scroll-thumb" />
            </div>
            <span className="free2-scroll-text">SCROLL TO NAVIGATE</span>
          </div>
        </div>

        {/* Right column — the genuinely-free features (M6 Phase 3a repopulation).
            The old 4-slot absolute "holographic" layout can't hold this many cards
            without overlap, so the column is a responsive CSS grid
            (.free2-right--grid, defined in free-features-new.css); cards use the
            base .free2-card (no absolute --primary/--secondary modifier). */}
        <div className="free2-right free2-right--grid">
          {FREE_CARDS.map((card) => (
            <article className="free2-card" key={card.title}>
              <div className="free2-card-header">
                <span className={`free2-chip ${card.chipClass}`}>{card.chip}</span>
                <span className="material-symbols-outlined free2-card-icon">{card.icon}</span>
              </div>
              <h3 className="free2-card-title">{card.title}</h3>
              <p className="free2-card-body">{card.body}</p>
              <div className="free2-card-footer">
                <span className="free2-module-label">{card.module}</span>
                <button
                  className="free2-interaction"
                  type="button"
                  aria-label={`Get ${card.title} in the Gaply desktop app`}
                  onClick={() => navigate('/download')}
                >
                  <span className="material-symbols-outlined free2-interaction-icon">arrow_forward</span>
                </button>
              </div>
            </article>
          ))}

          {/* Citation Generator — the client-side WEB tool, distinct from the
              desktop Citation Manager above: runs in the browser, no sign-in. */}
          <article className="free2-card">
            <div className="free2-card-header">
              <span className="free2-chip free2-chip--violet">No sign-in</span>
              <span className="material-symbols-outlined free2-card-icon">format_quote</span>
            </div>
            <h3 className="free2-card-title">Citation Generator</h3>
            <p className="free2-card-body">
              Format APA, Vancouver &amp; Harvard citations in your browser — DOI, ISBN &amp; PubMed lookup, BibTeX / RIS / JSON export. Saved lists stay on your device.
            </p>
            <div className="free2-card-footer">
              <span className="free2-module-label">MODULE_07</span>
              <button
                className="free2-interaction"
                type="button"
                aria-label="Open the browser Citation Generator"
                onClick={() => navigate('/citation-generator')}
              >
                <span className="material-symbols-outlined free2-interaction-icon">arrow_forward</span>
              </button>
            </div>
          </article>
        </div>
      </div>

      {/* Foreground spark particles */}
      <div className="free2-particles" aria-hidden="true">
        <span className="free2-particle free2-particle--a" />
        <span className="free2-particle free2-particle--b" />
        <span className="free2-particle free2-particle--c" />
      </div>
    </section>
  );
}
