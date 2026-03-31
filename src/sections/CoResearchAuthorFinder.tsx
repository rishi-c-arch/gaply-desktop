import React from 'react';
import { Link } from 'react-router-dom';
import { useTheme } from '../contexts/ThemeContext';
import author1 from '../assets/craf-authors/author-1.jpeg';
import author2 from '../assets/craf-authors/author-2.jpeg';
import author3 from '../assets/craf-authors/author-3.jpeg';
import author4 from '../assets/craf-authors/author-4.jpeg';
import '../styles/variables.css';
import '../styles/animations.css';
import '../styles/sections.css';
import '../styles/journal-quartile.css';
import './CoResearchAuthorFinder.css';

const PORTRAITS = [
  { id: 1, src: author1 },
  { id: 2, src: author2 },
  { id: 3, src: author3 },
  { id: 4, src: author4 },
] as const;

export default function CoResearchAuthorFinder() {
  const { theme } = useTheme();

  return (
    <section
      className="jqa-section craf-section"
      data-theme={theme}
      aria-labelledby="craf-heading"
    >
      <div className="craf-noise" aria-hidden />
      <div className="craf-mesh" aria-hidden />

      <div className="jqa-inner craf-inner">
        <header className="jqa-header craf-header--compact">
          <p className="craf-eyebrow">Research Hub</p>
          <h2 id="craf-heading" className="jqa-heading craf-title">
            Co- Research Author Finder
          </h2>
          <div className="jqa-underline" aria-hidden="true" />
        </header>

        <div
          className="craf-stage"
          aria-label="Researcher portraits in a 3D gallery preview"
        >
          <div className="craf-pivot">
            <div className="craf-orbit">
              {PORTRAITS.map((p, i) => (
                <figure
                  key={p.id}
                  className="craf-portrait"
                  style={{ '--craf-i': i } as React.CSSProperties}
                >
                  <div className="craf-portrait__frame">
                    <img
                      src={p.src}
                      alt=""
                      className="craf-portrait__img"
                      decoding="async"
                    />
                  </div>
                  <span className="craf-portrait__sheen" aria-hidden />
                </figure>
              ))}
            </div>
            <div className="craf-pedestal" aria-hidden />
          </div>
        </div>

        <div className="jqa-cta craf-cta-wrap">
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
