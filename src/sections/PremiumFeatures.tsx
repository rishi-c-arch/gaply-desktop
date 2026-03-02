import React from 'react';
import '../styles/variables.css';
import '../styles/animations.css';
import '../styles/premium-features-new.css';

export default function PremiumFeatures() {
  return (
    <section className="premium2-section" aria-labelledby="premium2-heading">
      {/* Background grid + fractal glow */}
      <div className="premium2-grid" aria-hidden="true" />
      <div className="premium2-fractal" aria-hidden="true" />
      <div className="premium2-orb premium2-orb--top" aria-hidden="true" />
      <div className="premium2-orb premium2-orb--bottom" aria-hidden="true" />
      <div className="premium2-orb premium2-orb--left" aria-hidden="true" />

      <div className="premium2-inner">
        {/* Left copy column */}
        <div className="premium2-left">
          <div className="premium2-left-line" aria-hidden="true">
            <span className="premium2-left-dot" />
          </div>

          <div className="premium2-left-content">
            <div className="premium2-eyebrow">
              <span className="premium2-eyebrow-dot" />
              <span className="premium2-eyebrow-label">Gaply Interface</span>
            </div>

            <h2 id="premium2-heading" className="premium2-heading">
              Premium <br />
              <span className="premium2-heading-strong">Features</span>
            </h2>

            <p className="premium2-body">
              Elevate your academic workflow. Bridge the gap between complex data and world-class
              publication with Gaply Premium.
            </p>

            <div className="premium2-scroll" aria-hidden="true">
              <div className="premium2-scroll-shell">
                <div className="premium2-scroll-dot" />
              </div>
              <span className="premium2-scroll-text">Scroll to Navigate</span>
            </div>
          </div>
        </div>

        {/* Right HUD cards – side-by-side so both visible */}
        <div className="premium2-right">
          <div className="premium2-glow" aria-hidden="true" />
          <div className="premium2-cards-row">
          {/* DataMaestro – left card, fully visible */}
          <article className="premium2-card premium2-card--side">
            <div className="premium2-card-header">
              <span className="premium2-chip premium2-chip--light">Module_02</span>
              <span className="material-symbols-outlined premium2-card-icon">hub</span>
            </div>
            <h3 className="premium2-card-title">DataMaestro</h3>
            <p className="premium2-card-body">
              Master the complexity. Advanced data orchestration for the modern researcher.
            </p>
            <div className="premium2-card-footer">
              <div className="premium2-avatar-row">
                <span className="premium2-avatar premium2-avatar--1" />
                <span className="premium2-avatar premium2-avatar--2" />
              </div>
              <button type="button" className="premium2-circle-btn" aria-label="Open DataMaestro">
                <span className="material-symbols-outlined">arrow_forward</span>
              </button>
            </div>
          </article>

          {/* Main card – PublishReady */}
          <article className="premium2-card premium2-card--main">
            <div className="premium2-card-header">
              <span className="premium2-chip premium2-chip--dark">Algorithm</span>
              <span className="material-symbols-outlined premium2-card-icon premium2-card-icon--spin">
                science
              </span>
            </div>

            <div className="premium2-chart group" aria-hidden="true">
              <div className="premium2-chart-bars">
                <span className="premium2-bar" />
                <span className="premium2-bar premium2-bar--mid" />
                <span className="premium2-bar" />
                <span className="premium2-bar premium2-bar--blue" />
                <span className="premium2-bar premium2-bar--peak">
                  <span className="premium2-bar-label">NOW</span>
                </span>
                <span className="premium2-bar" />
              </div>
              <div className="premium2-chart-pill">+12.5% Efficiency</div>
            </div>

            <h3 className="premium2-main-title">PublishReady</h3>
            <p className="premium2-card-body">
              Refine. Optimize. Succeed. The definitive algorithmic companion for manuscript perfection.
            </p>

            <div className="premium2-metrics">
              <div className="premium2-metric">
                <span className="premium2-metric-value">99.8%</span>
                <span className="premium2-metric-label">Accuracy</span>
              </div>
              <div className="premium2-metric">
                <span className="premium2-metric-value">45k+</span>
                <span className="premium2-metric-label">Journals</span>
              </div>
            </div>

            <div className="premium2-card-footer premium2-card-footer--main">
              <span className="premium2-module-label">Module_01</span>
              <button type="button" className="premium2-cta-btn" aria-label="Open PublishReady">
                <span className="material-symbols-outlined premium2-cta-icon">arrow_forward</span>
              </button>
            </div>
          </article>
          </div>

          {/* Floating specks */}
          <span className="premium2-speck premium2-speck--a" aria-hidden="true" />
          <span className="premium2-speck premium2-speck--b" aria-hidden="true" />
          <span className="premium2-speck premium2-speck--c" aria-hidden="true" />
        </div>
      </div>

      {/* Bottom status markers */}
      <div className="premium2-bullets" aria-hidden="true">
        <span className="premium2-bullet" />
        <span className="premium2-bullet premium2-bullet--dim" />
        <span className="premium2-bullet premium2-bullet--dim" />
      </div>
      <div className="premium2-system-ready" aria-hidden="true">
        // SYSTEM_READY
      </div>
    </section>
  );
}
