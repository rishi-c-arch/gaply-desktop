import React from 'react';
import { Link } from 'react-router-dom';
import researchBuddyCenter from '../assets/research-buddy-center.jpeg';
import '../styles/variables.css';
import './CoResearchAuthorFinder.css';

export default function CoResearchAuthorFinder() {
  return (
    <section className="craf-social" aria-labelledby="craf-social-title">
      <div className="craf-social__watermark" aria-hidden="true">
        <span className="craf-social__watermark-text">GAPLY</span>
      </div>

      <div className="craf-social__inner">
        <div className="craf-social__toolbar">
          <div className="craf-social__chevrons">
            <button type="button" className="craf-social__icon-btn" aria-label="Previous">
              <span className="material-symbols-outlined" aria-hidden>
                chevron_left
              </span>
            </button>
            <button type="button" className="craf-social__icon-btn" aria-label="Next">
              <span className="material-symbols-outlined" aria-hidden>
                chevron_right
              </span>
            </button>
          </div>
        </div>

        <div className="craf-social__center">
          <h2 id="craf-social-title" className="craf-social__title-stack">
            <span className="craf-social__title-kicker">Co- Research Author Finder</span>
            <span className="craf-social__buddy-line">Research.</span>
            <span className="craf-social__buddy-line">Buddy.</span>
          </h2>

          <div className="craf-social__portrait-center">
            <div className="craf-social__hero-frame">
              <img src={researchBuddyCenter} alt="" decoding="async" />
            </div>
          </div>
        </div>

        <div className="craf-social__foot">
          <Link to="/research-hub" className="craf-social__cta">
            Open Research Hub
            <span className="craf-social__cta-arrow" aria-hidden>
              →
            </span>
          </Link>
        </div>
      </div>
    </section>
  );
}
