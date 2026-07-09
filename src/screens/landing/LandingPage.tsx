// Gaply — desktop launch landing. Replicates the site's "Research. Buddy."
// section (Epilogue/Manrope, red GAPLY watermark) with a bare interactive 3D
// globe. Clicking the headline (or the CTA) transitions to the login page:
// the landing fades/scales out, then navigates; the login animates in.
import React, { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTheme } from '../../contexts/ThemeContext';
// Bundled fonts (offline desktop): the exact intended typefaces, not a fallback.
import '@fontsource/epilogue/700.css';
import '@fontsource/epilogue/900.css';
import '@fontsource/manrope/400.css';
import '@fontsource/manrope/600.css';
import GlobeFrame from './GlobeFrame';
import './landing.css';

const EXIT_MS = 480;

const ThemeToggle: React.FC<{ dark: boolean; onToggle: () => void }> = ({ dark, onToggle }) => (
  <button
    type="button"
    className="gpl-landing__theme"
    aria-label={dark ? 'Switch to light theme' : 'Switch to dark theme'}
    onClick={onToggle}
    data-testid="landing-theme-toggle"
  >
    {dark ? (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
        <circle cx="12" cy="12" r="5" /><path d="M12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4" />
      </svg>
    ) : (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
        <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
      </svg>
    )}
  </button>
);

const LandingPage: React.FC = () => {
  const navigate = useNavigate();
  const { theme, setTheme } = useTheme();
  const light = theme === 'light';
  const [leaving, setLeaving] = useState(false);
  const timer = useRef<number | null>(null);

  // Hide the global grain overlay while the (smooth) landing is shown.
  useEffect(() => {
    document.body.classList.add('gpl-landing-active');
    return () => {
      document.body.classList.remove('gpl-landing-active');
      if (timer.current) window.clearTimeout(timer.current);
    };
  }, []);

  // Play the exit animation, then navigate to login.
  const enterApp = () => {
    if (leaving) return;
    setLeaving(true);
    timer.current = window.setTimeout(() => navigate('/auth'), EXIT_MS);
  };

  return (
    <div className={`gpl-landing${leaving ? ' gpl-landing--leaving' : ''}`} data-testid="landing-page">
      <div className="gpl-landing__watermark" aria-hidden="true">
        <span className="gpl-landing__watermark-text">GAPLY</span>
      </div>

      <ThemeToggle dark={!light} onToggle={() => setTheme(light ? 'dark' : 'light')} />


      <div className="gpl-landing__inner">
        <div className="gpl-landing__center">
          {/* clicking the headline enters the app (→ login) */}
          <h2
            className="gpl-landing__title-stack"
            role="button"
            tabIndex={0}
            data-testid="landing-headline"
            onClick={enterApp}
            onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && enterApp()}
          >
            <span className="gpl-landing__buddy-line">Research.</span>
            <span className="gpl-landing__buddy-line">Buddy.</span>
          </h2>

          {/* bare interactive 3D globe (no frame) — soft red glow behind it */}
          <div className="gpl-landing__globe" data-testid="landing-globe-wrap">
            <GlobeFrame light={light} />
          </div>
        </div>

        <div className="gpl-landing__foot">
          <button type="button" className="gpl-landing__cta" onClick={enterApp} data-testid="landing-cta">
            Open Research Hub
            <span className="gpl-landing__cta-arrow" aria-hidden>→</span>
          </button>
        </div>
      </div>
    </div>
  );
};

export default LandingPage;
