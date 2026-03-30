import React from 'react';
import { useNavigate } from 'react-router-dom';
import '../styles/variables.css';
import '../styles/animations.css';
import '../styles/free-features-new.css';

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
            Access advanced research tools powered by generative mathematical models. Completely open
            for academic exploration.
          </p>

          <div className="free2-scroll-hint" aria-hidden="true">
            <div className="free2-scroll-shell">
              <div className="free2-scroll-thumb" />
            </div>
            <span className="free2-scroll-text">SCROLL TO NAVIGATE</span>
          </div>
        </div>

        {/* Right column – holographic cards */}
        <div className="free2-right">
          {/* Card 1 – Paper Search */}
          <article className="free2-card free2-card--primary">
            <div className="free2-card-header">
              <span className="free2-chip free2-chip--cyan">Free Access</span>
              <span className="material-symbols-outlined free2-card-icon">science</span>
            </div>
            <h3 className="free2-card-title">Paper Search</h3>
            <p className="free2-card-body">
              Semantic discovery across arXiv, PubMed &amp; IEEE. Visualize citation networks in real
              time.
            </p>

            <div className="free2-search-preview">
              <div className="free2-skel-line free2-skel-line--wide" />
              <div className="free2-skel-line free2-skel-line--half" />
              <div className="free2-search-chip">
                <span className="material-symbols-outlined free2-search-icon">search</span>
              </div>
            </div>

            <div className="free2-card-footer">
              <span className="free2-module-label">MODULE_01</span>
              <button className="free2-interaction" type="button" aria-label="Open Paper Search" onClick={() => navigate('/paper-search')}>
                <span className="material-symbols-outlined free2-interaction-icon">arrow_forward</span>
              </button>
            </div>
          </article>

          {/* Card 2 – Journal Match */}
          <article className="free2-card free2-card--secondary">
            <div className="free2-card-header">
              <span className="free2-chip">Algorithm</span>
              <span className="material-symbols-outlined free2-card-icon">hub</span>
            </div>
            <h3 className="free2-card-title free2-card-title--muted">Journal Match</h3>
            <p className="free2-card-body free2-card-body--muted">
              AI-driven probability matching for 45,000+ academic journals. Optimize your submission
              strategy.
            </p>

            <div className="free2-match-grid">
              <div className="free2-match-pill free2-match-pill--primary">
                <div className="free2-match-score">98%</div>
                <div className="free2-match-label">Nature</div>
              </div>
              <div className="free2-match-pill free2-match-pill--secondary">
                <div className="free2-match-score">85%</div>
                <div className="free2-match-label">Science</div>
              </div>
            </div>

            <div className="free2-card-footer">
              <span className="free2-module-label">MODULE_02</span>
              <button className="free2-interaction" type="button" aria-label="Open Journal Match" onClick={() => navigate('/journal-matching')}>
                <span className="material-symbols-outlined free2-interaction-icon">arrow_forward</span>
              </button>
            </div>
          </article>

          {/* Card 3 – Citation generator (100% free, browser-only) */}
          <article className="free2-card free2-card--tertiary">
            <div className="free2-card-header">
              <span className="free2-chip free2-chip--violet">No sign-in</span>
              <span className="material-symbols-outlined free2-card-icon">format_quote</span>
            </div>
            <h3 className="free2-card-title">Citation Generator</h3>
            <p className="free2-card-body">
              APA, Vancouver &amp; Harvard in your browser. DOI, ISBN, PubMed lookup, BibTeX / RIS / JSON converter—saved lists stay on your device.
            </p>

            <div className="free2-cite-preview" aria-hidden="true">
              <div className="free2-cite-line">(Author et al., 2024)</div>
              <div className="free2-cite-line free2-cite-line--dim">References</div>
              <div className="free2-cite-line free2-cite-line--short" />
            </div>

            <div className="free2-card-footer">
              <span className="free2-module-label">MODULE_03</span>
              <button
                className="free2-interaction"
                type="button"
                aria-label="Open Citation Generator"
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
