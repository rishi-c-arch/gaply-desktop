import React from 'react';
import { Link } from 'react-router-dom';
import '../styles/variables.css';
import '../styles/animations.css';
import '../styles/sections.css';
import '../styles/journal-quartile.css';
import './CoResearchAuthorFinder.css';

export default function CoResearchAuthorFinder() {
  return (
    <section
      className="jqa-section craf-section"
      aria-labelledby="craf-heading"
    >
      <div className="jqa-inner">
        <header className="jqa-header craf-header--compact">
          <h2 id="craf-heading" className="jqa-heading">
            Co- Research Author Finder
          </h2>
          <div className="jqa-underline" aria-hidden="true" />
        </header>

        <div className="jqa-cta">
          <Link
            to="/research-hub"
            className="jqa-btn jqa-btn-premium craf-cta-link"
          >
            Open Research Hub
          </Link>
        </div>
      </div>
    </section>
  );
}
