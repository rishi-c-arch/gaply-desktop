// Gaply — app home hero. A faithful in-app render of the marketing
// "Academic Research Platform" hero (the same markup + hero-new.css the public
// site uses), reusing the shared ResearchHubHeroEntry, ThreeJSGlobe, DNA
// background, and Material Symbols. Only HERO_OPTIONS is copied locally so the
// marketing App.tsx is not touched. CTAs are wired to the in-app flow.
import React from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { useTheme } from '../../contexts/ThemeContext';
import ResearchHubHeroEntry from '../../sections/ResearchHubHeroEntry';
import HeroAnimation from './HeroAnimation';
import homeHeroClip from '../../assets/home-hero-clip.png';
import '../../hero-animations.css';
import '../../hero-new.css';
import './homeHero.css';

// The real app feature pages (the old dashboard tools), in the hero-sidebar
// style. Each navigates to its actual in-app route.
const HERO_OPTIONS = [
  { label: 'PublishReady ★', href: '/app/publishready' },
  { label: 'Plagiarism Check', href: '/app/check/plagiarism' },
  { label: 'AI Check', href: '/app/check/ai' },
  { label: 'Statistical Analysis Check', href: '/app/check/stats' },
  { label: 'Citation Manager', href: '/app/citations' },
  { label: 'Note Creator', href: '/app/notes' },
  { label: 'Journal Check', href: '/app/journal' },
  { label: 'Research Copilot ★', href: '/app/copilot' },
  { label: 'Research Co-Author', href: '/app/community' },
  { label: 'My Manuscripts', href: '/app/coming-soon?feature=My%20Manuscripts' },
  { label: 'Settings', href: '/app/settings' },
];

const AppHomeHero: React.FC = () => {
  const { theme, toggleTheme } = useTheme();
  const isDark = theme === 'dark';
  const navigate = useNavigate();
  const location = useLocation();
  const [isMenuOpen, setIsMenuOpen] = React.useState(false);
  const activeSidebarIndex = React.useMemo(() => {
    const idx = HERO_OPTIONS.findIndex((opt) => opt.href === location.pathname);
    return idx === -1 ? 0 : idx;
  }, [location.pathname]);

  return (
    <div data-testid="home-dashboard">
      <section className="apple-hero apple-hero--no-header hero-section-stitch apple-hero--app-home" data-testid="dash-hero">
        {/* cinematic visual on the RIGHT — cropped + feathered so it melts into
            the background. The still image is the base (and the reduced-motion /
            pre-mount fallback); the live formula animation drifts on top. */}
        <div className="home-hero-clip" aria-hidden="true">
          <div className="home-hero-clip__img" style={{ backgroundImage: `url(${homeHeroClip})` }} />
          <HeroAnimation dark={isDark} />
        </div>
        <nav className="hero-top-nav hero-top-nav--relative" aria-label="Primary">
          <div className="hero-top-nav__left">
            <Link
              className="hero-top-nav__logo"
              to="/app"
              style={{ display: 'flex', alignItems: 'center', fontSize: 18, fontWeight: 600, letterSpacing: '0.05em', color: 'inherit', textDecoration: 'none' }}
            >
              GAPLY
            </Link>
            <div className="hero-top-nav__menu-wrap">
              <button
                className="hero-top-nav__menu"
                type="button"
                aria-haspopup="true"
                aria-expanded={isMenuOpen}
                onClick={() => setIsMenuOpen((open) => !open)}
              >
                <span className="hero-top-nav__menu-icon" />
                <span>Menu</span>
              </button>
              {isMenuOpen && (
                <div className="hero-menu-dropdown" role="menu">
                  {HERO_OPTIONS.map((opt) => (
                    <Link
                      key={opt.href}
                      to={opt.href}
                      className="hero-menu-dropdown__item"
                      role="menuitem"
                      onClick={() => setIsMenuOpen(false)}
                    >
                      {opt.label}
                    </Link>
                  ))}
                </div>
              )}
            </div>
          </div>
          <div className="hero-top-nav__right">
            <button type="button" className="hero-icon-btn" aria-label="Toggle theme" onClick={toggleTheme}>
              <span className="material-symbols-outlined hero-nav-icon" aria-hidden="true">
                {isDark ? 'light_mode' : 'dark_mode'}
              </span>
            </button>
            <button type="button" className="hero-icon-btn" aria-label="Search">
              <span className="material-symbols-outlined hero-nav-icon hero-nav-icon--xl" aria-hidden="true">search</span>
            </button>
            <button type="button" className="hero-icon-btn hero-icon-btn--volume" aria-label="Sound">
              <span className="material-symbols-outlined hero-nav-icon" aria-hidden="true">volume_up</span>
            </button>
          </div>
        </nav>

        <div className="hero-radial hero-radial--dark" aria-hidden="true" />
        <div className="hero-radial hero-radial--light" aria-hidden="true" />

        <main className="hero-main">
          <div className="hero-grid">
            <nav className="hero-sidebar" aria-label="Section navigation">
              <div className="hero-sidebar__line" />
              <div
                className="hero-sidebar__dot"
                aria-hidden="true"
                style={{ transform: `translateY(${activeSidebarIndex * 56}px)` }}
              />
              {HERO_OPTIONS.map((opt, i) => (
                <Link
                  key={opt.href}
                  to={opt.href}
                  className={`hero-sidebar__link ${i === activeSidebarIndex ? 'hero-sidebar__link--active' : ''}`}
                >
                  {opt.label}
                </Link>
              ))}
            </nav>

            <div className="hero-content-col">
              <div className="hero-eyebrow">
                <span className="hero-eyebrow__label">RESEARCH INTELLIGENCE</span>
                <span className="hero-eyebrow__divider" />
              </div>
              <h1 className="hero-heading">
                <span className="hero-heading__line">Academic</span>
                <span className="hero-heading__line hero-heading__line--gradient">Research Platform</span>
              </h1>
              <p className="hero-body">
                Your research. Your machine. Your Gaply.
              </p>
              <div className="hero-cta-row">
                <button
                  className="hero-cta-primary"
                  data-testid="start-analysis"
                  onClick={() => navigate('/app/upload')}
                >
                  <span>Start Analysis</span>
                  <span className="hero-cta-primary__icon" aria-hidden="true">→</span>
                </button>
                <button className="hero-cta-secondary" onClick={() => navigate('/app/upload')}>
                  <span className="material-symbols-outlined hero-cta-play" aria-hidden="true">play_circle</span>
                  <span>Watch Demo</span>
                </button>
              </div>
              <ResearchHubHeroEntry />
            </div>

            <div className="hero-visual-col">
              {/* the cinematic clip (rendered as a section-level backdrop on
                  the right) is the visual here; this column just holds the
                  scroll hint. */}
              <div className="hero-visual-wrap" />

              <div className="hero-scroll-hint" aria-hidden="true">
                <span className="hero-scroll-hint__line" />
                <span className="hero-scroll-hint__text">SCROLL</span>
              </div>
            </div>
          </div>

          <div className="hero-bottom-card">
            <div className="hero-bottom-card__number">01</div>
            <div className="hero-bottom-card__grid">
              <div className="hero-bottom-card__left">
                <div className="hero-bottom-card__live-row">
                  <span className="hero-bottom-card__dot" />
                  <span className="hero-bottom-card__live-label">Live Data</span>
                </div>
                <h3 className="hero-bottom-card__title">
                  The Private
                  <br />
                  Research Ecosystem
                </h3>
              </div>
              <div className="hero-bottom-card__right">
                <p>
                  Built on our own AI models, running on-device — no external servers, no data
                  sharing, no compromise between power and privacy.
                </p>
              </div>
            </div>
          </div>
        </main>
      </section>
    </div>
  );
};

export default AppHomeHero;
