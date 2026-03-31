import React from 'react';
import { useNavigate } from 'react-router-dom';
import author1 from '../assets/craf-authors/author-1.jpeg';
import author2 from '../assets/craf-authors/author-2.jpeg';
import author3 from '../assets/craf-authors/author-3.jpeg';
import author4 from '../assets/craf-authors/author-4.jpeg';
import '../styles/variables.css';
import '../styles/animations.css';
import '../styles/free-features-new.css';
import './CoResearchAuthorFinder.css';

const PHOTOS = [author1, author2, author3, author4] as const;

export default function CoResearchAuthorFinder() {
  const navigate = useNavigate();
  const goHub = () => navigate('/research-hub');

  return (
    <section className="free2-section craf2-section" aria-labelledby="craf2-heading">
      <div className="free2-index-bg" aria-hidden="true">
        <span className="free2-index-text">03</span>
      </div>

      <div className="free2-grid" aria-hidden="true" />

      <div className="free2-inner">
        <div className="free2-left">
          <div className="free2-status-row">
            <span className="free2-status-dot" />
            <span className="free2-status-label">RESEARCH NETWORK</span>
            <span className="free2-status-line" />
            <span className="free2-status-version">HUB LIVE</span>
          </div>

          <h2 id="craf2-heading" className="free2-heading">
            Co- Research <br />
            <span className="free2-heading-highlight">Author Finder</span>
          </h2>

          <p className="free2-body">
            The same holographic card system as Free Features—now tuned for researcher discovery, co-author
            matching, and secure collaboration on Gaply Research Hub.
          </p>

          <div className="free2-scroll-hint" aria-hidden="true">
            <div className="free2-scroll-shell">
              <div className="free2-scroll-thumb" />
            </div>
            <span className="free2-scroll-text">EXPLORE PROFILES</span>
          </div>
        </div>

        <div className="free2-right">
          <article className="free2-card free2-card--primary">
            <div className="free2-card-header">
              <span className="free2-chip free2-chip--cyan">Researcher</span>
              <span className="material-symbols-outlined free2-card-icon">person</span>
            </div>
            <h3 className="free2-card-title">Academic profiles</h3>
            <p className="free2-card-body">
              Domain tags, institutions, and publication context—built for science, not generic social blurbs.
            </p>
            <div className="craf2-photo">
              <img src={PHOTOS[0]} alt="" decoding="async" />
            </div>
            <div className="free2-card-footer">
              <span className="free2-module-label">RH_01</span>
              <button className="free2-interaction" type="button" aria-label="Open Research Hub" onClick={goHub}>
                <span className="material-symbols-outlined free2-interaction-icon">arrow_forward</span>
              </button>
            </div>
          </article>

          <article className="free2-card free2-card--secondary">
            <div className="free2-card-header">
              <span className="free2-chip">Discovery</span>
              <span className="material-symbols-outlined free2-card-icon">dynamic_feed</span>
            </div>
            <h3 className="free2-card-title free2-card-title--muted">Research feed</h3>
            <p className="free2-card-body free2-card-body--muted">
              Updates, questions, and &ldquo;looking for collaborator&rdquo; posts in one academic stream.
            </p>
            <div className="craf2-photo">
              <img src={PHOTOS[1]} alt="" decoding="async" />
            </div>
            <div className="free2-card-footer">
              <span className="free2-module-label">RH_02</span>
              <button className="free2-interaction" type="button" aria-label="Open Research Hub" onClick={goHub}>
                <span className="material-symbols-outlined free2-interaction-icon">arrow_forward</span>
              </button>
            </div>
          </article>

          <article className="free2-card free2-card--tertiary">
            <div className="free2-card-header">
              <span className="free2-chip free2-chip--violet">Co-author</span>
              <span className="material-symbols-outlined free2-card-icon">handshake</span>
            </div>
            <h3 className="free2-card-title">Connection requests</h3>
            <p className="free2-card-body">
              Field-matched invites with context—so introductions feel intentional, not random.
            </p>
            <div className="craf2-photo">
              <img src={PHOTOS[2]} alt="" decoding="async" />
            </div>
            <div className="free2-card-footer">
              <span className="free2-module-label">RH_03</span>
              <button className="free2-interaction" type="button" aria-label="Open Research Hub" onClick={goHub}>
                <span className="material-symbols-outlined free2-interaction-icon">arrow_forward</span>
              </button>
            </div>
          </article>

          <article className="free2-card free2-card--quaternary">
            <div className="free2-card-header">
              <span className="free2-chip free2-chip--emerald">Secure</span>
              <span className="material-symbols-outlined free2-card-icon">forum</span>
            </div>
            <h3 className="free2-card-title">Researcher messages</h3>
            <p className="free2-card-body">
              Direct messages once you connect—keep collaboration inside a research-native space.
            </p>
            <div className="craf2-photo">
              <img src={PHOTOS[3]} alt="" decoding="async" />
            </div>
            <div className="free2-card-footer">
              <span className="free2-module-label">RH_04</span>
              <button className="free2-interaction" type="button" aria-label="Open Research Hub" onClick={goHub}>
                <span className="material-symbols-outlined free2-interaction-icon">arrow_forward</span>
              </button>
            </div>
          </article>
        </div>
      </div>

      <div className="free2-particles" aria-hidden="true">
        <span className="free2-particle free2-particle--a" />
        <span className="free2-particle free2-particle--b" />
        <span className="free2-particle free2-particle--c" />
      </div>
    </section>
  );
}
