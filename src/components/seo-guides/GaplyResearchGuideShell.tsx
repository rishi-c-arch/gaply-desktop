import React, { useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import './GaplyResearchGuideShell.css';

const FONT_LINK_ID = 'gaply-research-guide-fonts';

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
          <button type="button" className="grg-topnav__cta" onClick={() => navigate('/')}>
            Back to Home
          </button>
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
            <div className="grg-hero__visual" aria-hidden="true">
              <img
                className="grg-hero__img"
                src="https://images.unsplash.com/photo-1456513080510-7bf3a84b82f8?auto=format&fit=crop&w=900&q=80"
                alt=""
                loading="lazy"
                decoding="async"
              />
              <div className="grg-hero__veil" />
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
