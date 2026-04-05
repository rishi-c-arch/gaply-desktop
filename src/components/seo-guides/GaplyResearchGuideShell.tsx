import React, { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useTheme } from '../../contexts/ThemeContext';
import './GaplyResearchGuideShell.css';
import { researchSeriesHeroSphereUrl } from './researchSeriesHeroSphereUrl';

const FONT_LINK_ID = 'gaply-research-guide-fonts';
const HERO_IMG_FALLBACK =
  'https://images.unsplash.com/photo-1456513080510-7bf3a84b82f8?auto=format&fit=crop&w=900&q=80';

export type ResearchGuideNavItem = { href: string; label: string };

export interface GaplyResearchGuideShellProps {
  headline: string;
  subhead: string;
  /** Small pill above the title, e.g. "Academic resource" */
  heroBadge?: string;
  sidebarTitle: string;
  sidebarMeta: string;
  sidebarNav: ResearchGuideNavItem[];
  /** Shown after hero; exact phrases for discoverability (unchanged copy elsewhere). */
  seoSearchPhrases?: readonly string[];
  siblingGuide?: { to: string; label: string };
  children: React.ReactNode;
}

const GaplyResearchGuideShell: React.FC<GaplyResearchGuideShellProps> = ({
  headline,
  subhead,
  heroBadge = 'Academic resource',
  sidebarTitle,
  sidebarMeta,
  sidebarNav,
  seoSearchPhrases,
  siblingGuide,
  children,
}) => {
  const navigate = useNavigate();
  const { theme, toggleTheme } = useTheme();
  const [heroSrc, setHeroSrc] = useState(() => researchSeriesHeroSphereUrl());

  useEffect(() => {
    if (!document.getElementById(FONT_LINK_ID)) {
      const link = document.createElement('link');
      link.id = FONT_LINK_ID;
      link.rel = 'stylesheet';
      link.href =
        'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Newsreader:ital,opsz,wght@0,6..72,400;0,6..72,600;0,6..72,700;1,6..72,400&display=swap';
      document.head.appendChild(link);
    }
  }, []);

  return (
    <div className="gaply-research-guide-root">
      <a href="#gaply-rg-main" className="grg-skip">
        Skip to guide content
      </a>

      <nav className="grg-topnav" aria-label="Guide">
        <div className="grg-topnav__inner">
          <span className="grg-topnav__brand">Research Series</span>
          <div className="grg-topnav__right">
            <button
              type="button"
              className="grg-topnav__theme"
              onClick={() => toggleTheme()}
              aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
              title={theme === 'dark' ? 'Light mode' : 'Dark mode'}
            >
              {theme === 'dark' ? (
                <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden>
                  <path d="M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10zm0-5h2v3h-2V2zm0 19h2v3h-2v-3zM2 11h3v2H2v-2zm17 0h3v2h-3v-2zM4.2 4.9l2.1 2.1-1.4 1.4L2.8 6.3 4.2 4.9zm12.5 12.5 2.1 2.1-1.4 1.4-2.1-2.1 1.4-1.4zm0-14L19.2 5l-1.4 1.4-2.1-2.1L16.7 3.3zm-12.5 12.5L5.6 19l-1.4-1.4 2.1-2.1 1.4 1.4z" />
                </svg>
              ) : (
                <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden>
                  <path d="M21 14.5A7.5 7.5 0 0 1 9.5 3 7.5 7.5 0 1 0 21 14.5z" />
                </svg>
              )}
            </button>
            <button type="button" className="grg-topnav__cta" onClick={() => navigate('/')}>
              Back to Home
            </button>
          </div>
        </div>
      </nav>

      <aside className="grg-sidebar" aria-label="On this page">
        <div className="grg-sidebar__intro">
          <div className="grg-sidebar__title">{sidebarTitle}</div>
          <div className="grg-sidebar__meta">{sidebarMeta}</div>
        </div>
        <nav className="grg-sidebar__nav">
          {sidebarNav.map((item) => (
            <a key={item.href} className="grg-sidebar__link" href={item.href}>
              {item.label}
            </a>
          ))}
        </nav>
        {siblingGuide ? (
          <div className="grg-sidebar__sibling">
            <Link to={siblingGuide.to}>{siblingGuide.label}</Link>
          </div>
        ) : null}
        <div className="grg-sidebar__footer">
          <Link className="grg-sidebar__minor" to="/support">
            Support
          </Link>
        </div>
      </aside>

      <nav className="grg-jump" aria-label="On this page">
        {sidebarNav.map((item) => (
          <a key={item.href} href={item.href}>
            {item.label}
          </a>
        ))}
      </nav>

      <main id="gaply-rg-main" className="grg-main">
        <header className="grg-hero">
          <div className="grg-hero__grid">
            <div className="grg-hero__text">
              <span className="grg-hero__badge">{heroBadge}</span>
              <h1 className="grg-hero__title">{headline}</h1>
              <p className="grg-hero__sub">{subhead}</p>
            </div>
            <div className="grg-hero__sphere-col" aria-hidden="true">
              <div className="grg-hero__sphere-glow" />
              <div className="grg-hero__sphere-scene">
                <div className="grg-hero__sphere-orbit">
                  <div className="grg-hero__sphere-ball">
                    <img
                      className="grg-hero__sphere-img"
                      src={heroSrc}
                      alt=""
                      width={640}
                      height={640}
                      loading="eager"
                      decoding="async"
                      onError={() => setHeroSrc(HERO_IMG_FALLBACK)}
                    />
                  </div>
                </div>
              </div>
            </div>
          </div>
        </header>

        {seoSearchPhrases && seoSearchPhrases.length > 0 ? (
          <section className="grg-seo-ribbon" aria-labelledby="grg-seo-ribbon-h">
            <h2 id="grg-seo-ribbon-h" className="grg-seo-ribbon__title">
              Related searches this guide supports
            </h2>
            <ul className="grg-seo-ribbon__list">
              {seoSearchPhrases.map((phrase) => (
                <li key={phrase} className="grg-seo-ribbon__item">
                  {phrase}
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        <div className="grg-main__inner">{children}</div>
      </main>

      <footer className="grg-foot">
        <div className="grg-foot__inner">
          <div className="grg-foot__brand">
            <span className="grg-foot__name">Gaply</span>
            <span className="grg-foot__tag">Research Series · Journal guides</span>
          </div>
          <nav className="grg-foot__links" aria-label="Related">
            <Link to="/features">Features</Link>
            <Link to="/journal-matching">Journal matching</Link>
            <Link to="/citation-generator">Citation generator</Link>
            <Link to="/support">Support</Link>
          </nav>
        </div>
      </footer>
    </div>
  );
};

export default GaplyResearchGuideShell;
