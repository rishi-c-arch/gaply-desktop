import React from 'react';
import { Link } from 'react-router-dom';
import { useTheme } from '../contexts/ThemeContext';
import '../sections/ResearchHubSection.css';

const ResearchHubPage: React.FC = () => {
  const { theme } = useTheme();

  return (
    <div className="rh-section" data-theme={theme} style={{ minHeight: '100vh', paddingTop: 100 }}>
      <div className="rh-section__noise" aria-hidden />
      <div className="rh-section__glow" aria-hidden />
      <div className="rh-section__inner">
        <span className="rh-section__badge">ResearchHub</span>
        <h1 className="rh-section__title" style={{ fontSize: 'clamp(1.75rem, 4vw, 2.5rem)' }}>
          <span>Collaboration hub</span>
        </h1>
        <p className="rh-section__lead" style={{ maxWidth: 560 }}>
          We are connecting this space to Supabase-backed profiles, feeds, and messages. If you do not see live features yet,
          check back soon — schema and APIs are staged for rollout.
        </p>
        <ul style={{ margin: '0 0 28px', paddingLeft: 20, color: 'inherit', opacity: 0.85, lineHeight: 1.7 }}>
          <li>Rich researcher profiles linked to your Gaply account</li>
          <li>Feed for updates and collaborator calls</li>
          <li>Connection requests and DMs between researchers</li>
          <li>Domain-based discovery (AI, neuroscience, climate, …)</li>
        </ul>
        <div className="rh-section__cta-row">
          <Link className="rh-section__cta" to="/">
            Back to home
          </Link>
          <Link className="rh-section__cta-secondary" to="/login">
            Log in
          </Link>
        </div>
      </div>
    </div>
  );
};

export default ResearchHubPage;
