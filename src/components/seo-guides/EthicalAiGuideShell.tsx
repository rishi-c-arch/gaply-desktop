import React, { useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { ETHICAL_AI_PDF_PATH } from './ethicalAiGuideTopics';
import './EthicalAiGuideShell.css';

const FONT_LINK_ID = 'ethical-ai-guide-fonts';

interface EthicalAiGuideShellProps {
  headline: string;
  subhead: string;
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

const EthicalAiGuideShell: React.FC<EthicalAiGuideShellProps> = ({ headline, subhead, children }) => {
  const navigate = useNavigate();

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
          <button type="button" className="eag-topnav__cta" onClick={() => navigate('/')}>
            Back to Home
          </button>
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
          <div className="eag-hero__visual" aria-hidden="true">
            <img
              className="eag-hero__img"
              src="https://images.unsplash.com/photo-1598511728269-cb1d0cacfba4?auto=format&fit=crop&w=900&q=80"
              alt=""
              loading="lazy"
              decoding="async"
            />
            <div className="eag-hero__veil" />
          </div>
        </header>

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
