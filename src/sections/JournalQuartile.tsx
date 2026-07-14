import React, { useRef } from 'react';
import { useIntersectionObserver } from '../hooks/useIntersectionObserver';
import '../styles/variables.css';
import '../styles/animations.css';
import '../styles/sections.css';
import '../styles/journal-quartile.css';

type QuartileSegment = {
  dasharray: string;
  dashoffset?: number;
  opacity?: number;
};

type QuartileConfig = {
  id: 'q1' | 'q2' | 'q3' | 'q4';
  label: string;
  percent: string;
  subtitle: string;
  segments: QuartileSegment[];
};

const QUARTILE_GUIDE_URLS: Record<string, string> = {
  q1: '/quartile-guides/q1-guide.html',
  q2: '/quartile-guides/q2-q3-guide.html',
  q3: '/quartile-guides/q2-q3-guide.html',
  q4: '/quartile-guides/q4-guide.html',
};

const QUARTILES: QuartileConfig[] = [
  {
    id: 'q1',
    label: 'Q1',
    percent: '72%',
    subtitle: 'High Impact Journals',
    segments: [
      { dasharray: '70 251.2', dashoffset: 0, opacity: 1 },
      { dasharray: '100 251.2', dashoffset: -80, opacity: 0.8 },
    ],
  },
  {
    id: 'q2',
    label: 'Q2',
    percent: '45%',
    subtitle: 'Standard Growth',
    segments: [
      { dasharray: '140 251.2', dashoffset: 0, opacity: 1 },
      { dasharray: '40 251.2', dashoffset: -145, opacity: 0.8 },
    ],
  },
  {
    id: 'q3',
    label: 'Q3',
    percent: '28%',
    subtitle: 'Emerging Sources',
    segments: [
      { dasharray: '180 251.2', dashoffset: 0, opacity: 1 },
      { dasharray: '20 251.2', dashoffset: -190, opacity: 0.8 },
    ],
  },
  {
    id: 'q4',
    label: 'Q4',
    percent: '15%',
    subtitle: 'Niche Publications',
    segments: [
      { dasharray: '90 251.2', dashoffset: 0, opacity: 1 },
      { dasharray: '120 251.2', dashoffset: -100, opacity: 0.8 },
    ],
  },
];

function QuartileChart({ segments }: { segments: QuartileSegment[] }) {
  return (
    <svg
      className="jqa-chart-svg"
      viewBox="0 0 100 100"
      aria-hidden
    >
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

export default function JournalQuartile() {
  const sectionRef = useRef<HTMLElement>(null);
  const inView = useIntersectionObserver(sectionRef, { threshold: 0.1 });

  return (
    <section ref={sectionRef} className="jqa-section" aria-labelledby="jqa-heading">
      <div className="jqa-inner">
        <header className="jqa-header">
          <h2 id="jqa-heading" className="jqa-heading">
            Journal Quartile Analysis
          </h2>
          <p className="jqa-subtitle">
            Visualizing impact factors across four distinct quartiles with real-time data processing.
          </p>
          <div className="jqa-underline" aria-hidden="true" />
        </header>

        <div className="jqa-grid">
          {QUARTILES.map((item, index) => (
            <article
              key={item.id}
              className={`jqa-card jqa-card--${item.id} ${inView ? 'jqa-card--visible' : ''}`}
              style={{ transitionDelay: `${index * 80}ms` }}
              tabIndex={0}
              role="button"
              aria-label={`${item.label} – ${item.subtitle}. Click to open guide.`}
              onClick={() => {
                const url = QUARTILE_GUIDE_URLS[item.id];
                if (url) window.open(url, '_blank', 'noopener,noreferrer');
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  const url = QUARTILE_GUIDE_URLS[item.id];
                  if (url) window.open(url, '_blank', 'noopener,noreferrer');
                }
              }}
            >
              <div className="jqa-card-label">{item.label}</div>

              <div className="jqa-chart-wrapper">
                <div className="jqa-chart">
                  <QuartileChart segments={item.segments} />
                  <div className="jqa-chart-center">
                    <span className="jqa-chart-percent">{item.percent}</span>
                  </div>
                </div>
              </div>

              <div className="jqa-card-footer">
                <p className="jqa-card-subtitle">{item.subtitle}</p>
              </div>
            </article>
          ))}
        </div>

        <div className="jqa-cta">
          <button
            type="button"
            onClick={() => { window.location.href = '/download'; }}
            className="jqa-btn jqa-btn-premium"
            aria-label="Verify journal authenticity"
          >
            Verify Your Journal is Real
          </button>
        </div>
      </div>
    </section>
  );
}
