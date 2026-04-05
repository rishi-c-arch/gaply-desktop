import React, { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useTheme } from '../../contexts/ThemeContext';
import { ETHICAL_AI_PDF_PATH } from './ethicalAiGuideTopics';
import { researchSeriesHeroSphereUrl } from './researchSeriesHeroSphereUrl';
import './EthicalAiGuideShell.css';

const FONT_LINK_ID = 'ethical-ai-guide-fonts';
const HERO_IMG_FALLBACK =
  'https://images.unsplash.com/photo-1456513080510-7bf3a84b82f8?auto=format&fit=crop&w=900&q=80';

interface EthicalAiGuideShellProps {
  headline: string;
  subhead: string;
  /** Optional “Related searches” ribbon below the hero (SEO / discoverability). */
  seoSearchPhrases?: readonly string[];
  children: React.ReactNode;
}

function IconSlides() {
  return (
    <svg className="eag-icon" width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M4 5h10v8H4V5zm12 0h4v14h-4V5zM4 15h10v4H4v-4z"
        fill="currentColor"
        opacity="0.9"
      />
    </svg>
  );
}

function IconTopics() {
  return (
    <svg className="eag-icon" width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M4 6h16v2H4V6zm0 5h16v2H4v-2zm0 5h10v2H4v-2z"
        fill="currentColor"
        opacity="0.9"
      />
    </svg>
  );
}

function IconArticle() {
  return (
    <svg className="eag-icon" width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M6 3h9l3 3v15a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm8 2H6v16h10v-2h-4v-2h4v-2h-4v-2h4V5h-2V5z"
        fill="currentColor"
        opacity="0.9"
      />
    </svg>
  );
}

function IconDownload() {
  return (
    <svg className="eag-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M12 3v10.17l2.59-2.58L16 12l-5 5-5-5 1.41-1.41L11 13.17V3h1zm-7 14h14v2H5v-2z"
        fill="currentColor"
      />
    </svg>
  );
}

function IconHelp() {
  return (
    <svg className="eag-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 17h-2v-2h2v2zm2.07-7.75-.9.92A3.23 3.23 0 0 0 13 15h-2v-.5a4.48 4.48 0 0 1 1.17-2.83l1.24-1.26A2 2 0 1 0 10 9H8a4 4 0 0 1 7.07 2.25z"
        fill="currentColor"
        opacity="0.85"
      />
    </svg>
  );
}

const EthicalAiGuideShell: React.FC<EthicalAiGuideShellProps> = ({
  headline,
  subhead,
  seoSearchPhrases,
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

  const subParts = subhead.split('·').map((s) => s.trim());
  const sidebarMeta = subParts[1] ?? subhead;

  return (
    <div className="ethical-ai-guide-root">
      <a href="#ethical-ai-guide-main" className="ethical-ai-guide-skip">
        Skip to guide content
      </a>

      <nav className="eag-topnav" aria-label="Guide">
        <div className="eag-topnav__inner">
          <span className="eag-topnav__brand">Research Series</span>
          <div className="eag-topnav__right">
            <button
              type="button"
              className="eag-topnav__theme"
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
            <button type="button" className="eag-topnav__cta" onClick={() => navigate('/')}>
              Back to Home
            </button>
          </div>
        </div>
      </nav>

      <aside className="eag-sidebar" aria-label="On this page">
        <div className="eag-sidebar__intro">
          <div className="eag-sidebar__title">The Ethical Researcher&apos;s Guide to AI</div>
          <div className="eag-sidebar__meta">{sidebarMeta}</div>
        </div>
        <nav className="eag-sidebar__nav">
          <a className="eag-sidebar__link eag-sidebar__link--active" href="#ethical-ai-deck-anchor">
            <IconSlides />
            <span>Slides</span>
          </a>
          <a className="eag-sidebar__link" href="#ethical-ai-topics-anchor">
            <IconTopics />
            <span>Topics</span>
          </a>
          <a className="eag-sidebar__link" href="#ethical-ai-transcript-anchor">
            <IconArticle />
            <span>Text version</span>
          </a>
        </nav>
        <div className="eag-sidebar__actions">
          <a className="eag-sidebar__download" href={ETHICAL_AI_PDF_PATH} download>
            <IconDownload />
            Download PDF
          </a>
        </div>
        <div className="eag-sidebar__footer">
          <Link className="eag-sidebar__minor" to="/support">
            <IconHelp />
            Support
          </Link>
        </div>
      </aside>

      <main id="ethical-ai-guide-main" className="eag-main">
        <header className="eag-hero">
          <div className="eag-hero__text">
            <span className="eag-hero__badge">
              {subParts[0] ?? 'Research Series'}
              {subParts[1] ? ` · ${subParts[1]}` : ''}
            </span>
            <h1 className="eag-hero__title">{headline}</h1>
            <p className="eag-hero__sub">{subhead}</p>
          </div>
          <div className="eag-hero__sphere-col" aria-hidden="true">
            <div className="eag-hero__sphere-glow" />
            <div className="eag-hero__sphere-scene">
              <div className="eag-hero__sphere-orbit">
                <div className="eag-hero__sphere-ball">
                  <img
                    className="eag-hero__sphere-img"
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
        </header>

        {seoSearchPhrases && seoSearchPhrases.length > 0 ? (
          <section className="eag-seo-ribbon" aria-labelledby="eag-seo-ribbon-h">
            <h2 id="eag-seo-ribbon-h" className="eag-seo-ribbon__title">
              Related searches this guide supports
            </h2>
            <ul className="eag-seo-ribbon__list">
              {seoSearchPhrases.map((phrase) => (
                <li key={phrase} className="eag-seo-ribbon__item">
                  {phrase}
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        <div className="eag-main__inner">{children}</div>
      </main>

      <footer className="eag-foot">
        <div className="eag-foot__inner">
          <div className="eag-foot__brand">
            <span className="eag-foot__name">Gaply</span>
            <span className="eag-foot__tag">
              {subParts[0]}
              {subParts[1] ? ` · ${subParts[1]}` : ''}
            </span>
          </div>
          <nav className="eag-foot__links" aria-label="Related">
            <Link to="/features">Features</Link>
            <Link to="/citation-generator">Citation generator</Link>
            <Link to="/support">Support</Link>
          </nav>
        </div>
      </footer>
    </div>
  );
};

export default EthicalAiGuideShell;
