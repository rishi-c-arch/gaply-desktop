import React, { useRef } from 'react';
import { Link } from 'react-router-dom';
import { useIntersectionObserver } from '../hooks/useIntersectionObserver';
import '../styles/variables.css';
import '../styles/animations.css';
import '../styles/sections.css';
import '../styles/journal-quartile.css';
import './CoResearchAuthorFinder.css';

type QuartileSegment = {
  dasharray: string;
  dashoffset?: number;
  opacity?: number;
};

type PillarConfig = {
  id: 'p1' | 'p2' | 'p3' | 'p4';
  label: string;
  percent: string;
  subtitle: string;
  segments: QuartileSegment[];
};

const PILLARS: PillarConfig[] = [
  {
    id: 'p1',
    label: 'Match',
    percent: '92%',
    subtitle: 'Domain-aligned researcher discovery',
    segments: [
      { dasharray: '70 251.2', dashoffset: 0, opacity: 1 },
      { dasharray: '100 251.2', dashoffset: -80, opacity: 0.8 },
    ],
  },
  {
    id: 'p2',
    label: 'Profile',
    percent: '78%',
    subtitle: 'Paper-aware bios & ORCID-ready',
    segments: [
      { dasharray: '140 251.2', dashoffset: 0, opacity: 1 },
      { dasharray: '40 251.2', dashoffset: -145, opacity: 0.8 },
    ],
  },
  {
    id: 'p3',
    label: 'Connect',
    percent: '64%',
    subtitle: 'Co-author requests with context',
    segments: [
      { dasharray: '180 251.2', dashoffset: 0, opacity: 1 },
      { dasharray: '20 251.2', dashoffset: -190, opacity: 0.8 },
    ],
  },
  {
    id: 'p4',
    label: 'Message',
    percent: '51%',
    subtitle: 'Secure DMs once you connect',
    segments: [
      { dasharray: '90 251.2', dashoffset: 0, opacity: 1 },
      { dasharray: '120 251.2', dashoffset: -100, opacity: 0.8 },
    ],
  },
];

function RingChart({ segments }: { segments: QuartileSegment[] }) {
  return (
    <svg className="jqa-chart-svg" viewBox="0 0 100 100" aria-hidden>
      <circle
        className="jqa-chart-track"
        cx="50"
        cy="50"
        r="40"
        fill="transparent"
        strokeWidth="12"
      />
      {segments.map((segment, index) => (
        <circle
          key={index}
          className="jqa-chart-segment"
          cx="50"
          cy="50"
          r="40"
          fill="transparent"
          strokeWidth="12"
          strokeDasharray={segment.dasharray}
          strokeDashoffset={segment.dashoffset ?? 0}
          style={segment.opacity != null ? { opacity: segment.opacity } : undefined}
        />
      ))}
    </svg>
  );
}

export default function CoResearchAuthorFinder() {
  const sectionRef = useRef<HTMLElement>(null);
  const inView = useIntersectionObserver(sectionRef, { threshold: 0.1 });

  return (
    <section
      ref={sectionRef}
      className="jqa-section craf-section"
      aria-labelledby="craf-heading"
    >
      <div className="jqa-inner">
        <header className="jqa-header">
          <h2 id="craf-heading" className="jqa-heading">
            Co- Research Author Finder
          </h2>
          <p className="jqa-subtitle">
            Discover collaborators who fit your field, see real research context—not generic profiles—and move
            from introduction to manuscript with Research Hub on Gaply.
          </p>
          <div className="jqa-underline" aria-hidden="true" />
        </header>

        <div className="jqa-grid">
          {PILLARS.map((item, index) => (
            <Link
              key={item.id}
              to="/research-hub"
              className={`jqa-card craf-card-link jqa-card--${item.id.replace('p', 'q')} ${
                inView ? 'jqa-card--visible' : ''
              }`}
              style={{ transitionDelay: `${index * 80}ms` }}
              aria-label={`${item.label}: ${item.subtitle}. Open Research Hub.`}
            >
              <div className="jqa-card-label">{item.label}</div>

              <div className="jqa-chart-wrapper">
                <div className="jqa-chart">
                  <RingChart segments={item.segments} />
                  <div className="jqa-chart-center">
                    <span className="jqa-chart-percent">{item.percent}</span>
                  </div>
                </div>
              </div>

              <div className="jqa-card-footer">
                <p className="jqa-card-subtitle">{item.subtitle}</p>
              </div>
            </Link>
          ))}
        </div>

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
