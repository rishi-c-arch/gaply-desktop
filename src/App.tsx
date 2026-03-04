import React, { useEffect } from 'react';
import { ThemeProvider, useTheme } from './contexts/ThemeContext';
import { Routes, Route, Link, useLocation, useMatch } from 'react-router-dom';
import './App.css';
import './styles/variables.css';
import './hero-animations.css';
import './hero-new.css';
import './responsive.css';
import ThreeJSGlobe from './components/ThreeJSGlobe';
import HeroToSecondTransition from './components/HeroToSecondTransition';
import FreeFeatures from './sections/FreeFeatures';
import PremiumFeatures from './sections/PremiumFeatures';
import JournalQuartile from './sections/JournalQuartile';
import PremiumFooter3D from './components/PremiumFooter3D';
import EnhancedPremiumPage from './components/EnhancedPremiumPage';
import Overview from './pages/Overview';
import ProjectsPage from './pages/ProjectsPage';
import UsagePage from './pages/UsagePage';
import SettingsPage from './pages/SettingsPage';
import BillingPage from './pages/BillingPage';
import DashboardLayout from './components/Layout/DashboardLayout';
import PackageSelection from './components/PackageSelection';
import LoginPage from './components/LoginPage';
import SignupPage from './components/SignupPage';
import AcademicAIRemoverPage from './components/AcademicAIRemoverPage';
import PaperSearchPage from './components/PaperSearchPage';
import JournalMatchingPage from './components/JournalMatchingPage';
import FeaturesPage from './components/FeaturesPage';
import CareerPage from './components/CareerPage';
import HireExpertPage from './components/HireExpertPage';
import ExpertSearchResultsPage from './components/ExpertSearchResultsPage';
import PricingSection from './components/PricingSection';
import PrivacyPolicyPage from './components/PrivacyPolicyPage';
import TermsOfServicePage from './components/TermsOfServicePage';
import DocumentOrchestratorPage from './components/DocumentOrchestratorPage';
import ManuscriptOrchestratorPage from './components/ManuscriptOrchestratorPage';
import StatisticalResearchOrchestratorPage from './components/StatisticalResearchOrchestratorPage';
import DataMaestroProPage from './components/DataMaestroProPage';
import JournalVerifyPage from './components/JournalVerifyPage';
import ContactPage from './components/ContactPage';
import DownloadsPage from './components/DownloadsPage';
import DownloadPopUp from './components/DownloadPopUp';
import BlogPage from './pages/BlogPage';
import BlogPostPage from './pages/BlogPostPage';
import { AuthProvider } from './contexts/AuthContext';
import { ProtectedRoute } from './components/ProtectedRoute';

// SEO Component for dynamic meta tags
const SEOHead: React.FC<{ title?: string; description?: string; keywords?: string }> = ({ 
  title = "Gaply - AI-Powered Academic Research Platform | Thesis Writing, Journal Matching, AI Detection", 
  description = "Professional AI-powered academic research platform offering thesis writing help, journal matching, AI content detection, plagiarism checking, dissertation editing, and research paper assistance for PhD students and researchers worldwide.",
  keywords = "AI content remover for research papers, free AI detector and editor for PhD thesis, AI writing detection tool for academic writing, detect AI plagiarism in research paper, AI paraphrase detector academic, plagiarism checking service for thesis, best plagiarism checker for research papers, remove plagiarism from dissertation, academic text originality checker, help with thesis writing and formatting, dissertation writing service online, PhD thesis writing help, master's thesis editing service, dissertation proofreading and formatting, thesis structure and formatting guidelines, doctoral dissertation consultation, academic thesis writing assistance, research paper writing service, research paper evaluation help, academic paper writing assistance, scientific writing support online, research methodology help, literature review writing service, best site for research guidance, how to write a research proposal, funding proposal writing help, journal finder for Scopus, Elsevier journal suggestion tool, SCI journal recommendation service, find journal for my paper, journal submission assistance service, how to submit paper to journal, conference paper preparation help, conference presentation coaching, publish paper in IEEE journal, academic proofreading and editing service, research paper editing service, thesis proofreading help, professional dissertation editor, edit academic paper online, grammar check for scholarly writing, academic copyediting service, SPSS statistical analysis help, data analysis service for researchers, statistical analysis assistance for thesis, SPSS tutorial for dissertation, data interpretation help for research, quantitative analysis support for PhD, statistics help for academic research"
}) => {
  useEffect(() => {
    // Update document title
    document.title = title;
    
    // Update meta description
    const metaDescription = document.querySelector('meta[name="description"]');
    if (metaDescription) {
      metaDescription.setAttribute('content', description);
    }
    
    // Update meta keywords
    const metaKeywords = document.querySelector('meta[name="keywords"]');
    if (metaKeywords) {
      metaKeywords.setAttribute('content', keywords);
    }
    
    // Update Open Graph title
    const ogTitle = document.querySelector('meta[property="og:title"]');
    if (ogTitle) {
      ogTitle.setAttribute('content', title);
    }
    
    // Update Open Graph description
    const ogDescription = document.querySelector('meta[property="og:description"]');
    if (ogDescription) {
      ogDescription.setAttribute('content', description);
    }
    
    // Update Twitter title
    const twitterTitle = document.querySelector('meta[property="twitter:title"]');
    if (twitterTitle) {
      twitterTitle.setAttribute('content', title);
    }
    
    // Update Twitter description
    const twitterDescription = document.querySelector('meta[property="twitter:description"]');
    if (twitterDescription) {
      twitterDescription.setAttribute('content', description);
    }
  }, [title, description, keywords]);
  
  return null;
};

// Remove floating orb function
const removeFloatingOrb = () => {
  // Remove any elements that might be the floating orb
  const selectors = [
    '.blob', '.cursor', '.mouse-follower', '.orb', '.light-orb', '.hero-orb',
    '.floating-orbs', '.particle', '[class*="cursor"]', '[class*="orb"]', 
    '[class*="blob"]', '[class*="float"]'
  ];
  
  selectors.forEach(selector => {
    const elements = document.querySelectorAll(selector);
    elements.forEach(el => {
      if (el instanceof HTMLElement) {
        el.style.display = 'none';
        el.style.opacity = '0';
        el.style.visibility = 'hidden';
        el.style.pointerEvents = 'none';
      }
    });
  });
  
  // Also remove any fixed positioned elements with high z-index that might be the orb
  const allDivs = document.querySelectorAll('div');
  allDivs.forEach(div => {
    const style = window.getComputedStyle(div);
    if (style.position === 'fixed' && parseInt(style.zIndex) > 1000) {
      // Check if it's a circular element (likely the orb)
      if (style.borderRadius === '50%' || style.borderRadius.includes('%')) {
        div.style.display = 'none';
        div.style.opacity = '0';
        div.style.visibility = 'hidden';
        div.style.pointerEvents = 'none';
      }
    }
  });
};

// Apple-style Header Component
const AppleHeader: React.FC<{ theme: 'light' | 'dark'; onToggleTheme: () => void }> = ({ theme, onToggleTheme }) => {
  const isDark = theme === 'dark';
  const textShadow = isDark ? '0 1px 2px rgba(0,0,0,0.25)' : '0 1px 2px rgba(0,0,0,0.12)';

  return (
    <header className="apple-header">
      <div className="apple-header__content">
        <div className="apple-header__logo" style={{ color: 'var(--header-text)', textShadow }}>
          Gaply
        </div>
        <nav className="apple-header__nav" aria-label="Primary">
          {[
            { label: 'Features', href: '/features' },
            { label: 'Pricing', href: '/pricing' },
            { label: 'Download', href: '/download' },
            { label: 'Careers', href: '/career' },
            { label: 'Contact', href: '/contact' },
          ].map((l) => (
            <a
              key={l.label}
              href={l.href}
              className="apple-header__link"
              style={{ color: 'var(--header-text)' }}
            >
              {l.label}
            </a>
          ))}
          <button
            className="theme-toggle"
            onClick={onToggleTheme}
            aria-label="Toggle light/dark theme"
          >
            {isDark ? 'Light Mode' : 'Dark Mode'}
          </button>
          <button
            className="apple-header__cta"
            onClick={() => {
              window.location.href = '/hire-expert';
            }}
          >
            <span>Hire an expert</span>
            <div className="apple-header__cta-shine" aria-hidden="true" />
          </button>
        </nav>
      </div>
    </header>
  );
};

// Left-side hero options (replaces header on home): figma-style vertical nav
const HERO_OPTIONS = [
  { label: 'Features', href: '/features' },
  { label: 'Pricing', href: '/pricing' },
  { label: 'Download', href: '/download' },
  { label: 'Careers', href: '/career' },
  { label: 'Hire an expert', href: '/hire-expert' },
];

// Apple-style Hero Section (Figma-inspired hero; no header on home)
const AppleHeroSection: React.FC = () => {
  const { theme, toggleTheme } = useTheme();
  const isDark = theme === 'dark';
  const location = useLocation();
  const [isMenuOpen, setIsMenuOpen] = React.useState(false);
  const activeSidebarIndex = React.useMemo(() => {
    const idx = HERO_OPTIONS.findIndex((opt) => opt.href === location.pathname);
    return idx === -1 ? 0 : idx;
  }, [location.pathname]);

  return (
    <section className="apple-hero apple-hero--no-header hero-section-stitch">
      <nav className="hero-top-nav hero-top-nav--relative" aria-label="Primary">
        <div className="hero-top-nav__left">
          <Link className="hero-top-nav__logo" to="/">
            Gaply
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
          <button
            type="button"
            className="hero-icon-btn"
            aria-label="Toggle theme"
            onClick={toggleTheme}
          >
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
              className={`hero-sidebar__link ${
                i === activeSidebarIndex ? 'hero-sidebar__link--active' : ''
              }`}
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
            Gaply is your personal Research Guide. So don&apos;t put off publication for later.
            Think about your impact factor today.
          </p>
          <div className="hero-cta-row">
            <button
              className="hero-cta-primary"
              onClick={() => {
                window.location.href = '/login';
              }}
            >
              <span>Start Analysis</span>
              <span className="hero-cta-primary__icon" aria-hidden="true">→</span>
            </button>
            <button
              className="hero-cta-secondary"
              onClick={() => {
                window.location.href = '/features';
              }}
            >
              <span className="material-symbols-outlined hero-cta-play" aria-hidden="true">play_circle</span>
              <span>Watch Demo</span>
            </button>
          </div>
        </div>

        <div className="hero-visual-col">
          <div className="hero-visual-wrap">
            <div className="hero-blur-orb" aria-hidden="true" />

            {/* Large DNA background layer */}
            <div className="hero-dna-layer" aria-hidden="true">
              <img
                className="hero-dna-overlay"
                alt="DNA spiral"
                src="https://lh3.googleusercontent.com/aida-public/AB6AXuAiPX4n1bGN32XXi6Oglrn-qVkS29j0osQxjswIn2oIqbxAmi-5Yps0mrG6mtC68m3ydc03_0fv-Y9eF9CJetfcsCGhY5zxaw6h7N06x3b12D96rnlyMXpz-dqIKrCZDn_C9AinqWkoS4KdRj11laXXbj8nf1NTPh3-pTHTHJJ5kHGA4IH0Mx1lCtv9HPKV5_j3VoRib1J5cdsai6cCZCVzsyeMprweKjkol9OKf9bT0lSPBFOenbK31ZsruKpvFZefizNVMTkADns"
              />
            </div>

            {/* Small 3D globe chip floating above DNA */}
            <div className="hero-globe-chip" aria-hidden="true">
              <ThreeJSGlobe />
            </div>
          </div>

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
              Global Citation
              <br />
              Network Analysis
            </h3>
          </div>
          <div className="hero-bottom-card__right">
            <p>
              Real-time visualization of cross-disciplinary connections. Track the evolution
              of your research impact across global institutions.
            </p>
          </div>
        </div>
      </div>
      </main>
    </section>
  );
};

// Inner App component that uses useAuth
const AppContent: React.FC = () => {
  const { theme: headerTheme, toggleTheme: handleToggleTheme } = useTheme();
  const location = useLocation();
  const isExactHome = useMatch({ path: '/', end: true });
  const isHideHeaderPath =
    isExactHome ||
    location.pathname.startsWith('/dashboard') ||
    location.pathname.startsWith('/blog') ||
    location.pathname === '/hire-expert' ||
    location.pathname === '/search-results' ||
    location.pathname === '/paper-search' ||
    location.pathname === '/journal-matching';

  // Call the function to remove the floating orb when the component mounts
  useEffect(() => {
    removeFloatingOrb();
  }, []);

  const handleAuthSuccess = (token: string, userData: any, redirect?: string) => {
    window.location.href = redirect || '/packages';
  };

  return (
    <div className="App" data-home={isExactHome ? 'true' : undefined}>
      <div className="noise-bg" aria-hidden="true" />
      {!isHideHeaderPath && <AppleHeader theme={headerTheme} onToggleTheme={handleToggleTheme} />}
      <Routes>
          <Route path="/" element={
            <>
              <SEOHead 
                title="Gaply - AI-Powered Academic Research Platform | Thesis Writing, Journal Matching, AI Detection"
                description="Professional AI-powered academic research platform offering thesis writing help, journal matching, AI content detection, plagiarism checking, dissertation editing, and research paper assistance for PhD students and researchers worldwide."
                keywords="AI content remover for research papers, free AI detector and editor for PhD thesis, AI writing detection tool for academic writing, detect AI plagiarism in research paper, AI paraphrase detector academic, plagiarism checking service for thesis, best plagiarism checker for research papers, remove plagiarism from dissertation, academic text originality checker, help with thesis writing and formatting, dissertation writing service online, PhD thesis writing help, master's thesis editing service, dissertation proofreading and formatting, thesis structure and formatting guidelines, doctoral dissertation consultation, academic thesis writing assistance, research paper writing service, research paper evaluation help, academic paper writing assistance, scientific writing support online, research methodology help, literature review writing service, best site for research guidance, how to write a research proposal, funding proposal writing help, journal finder for Scopus, Elsevier journal suggestion tool, SCI journal recommendation service, find journal for my paper, journal submission assistance service, how to submit paper to journal, conference presentation coaching, publish paper in IEEE journal, academic proofreading and editing service, research paper editing service, thesis proofreading help, professional dissertation editor, edit academic paper online, grammar check for scholarly writing, academic copyediting service, SPSS statistical analysis help, data analysis service for researchers, statistical analysis assistance for thesis, SPSS tutorial for dissertation, data interpretation help for research, quantitative analysis support for PhD, statistics help for academic research"
              />
              <AppleHeroSection />
              <HeroToSecondTransition />
              <FreeFeatures />
              <PremiumFeatures />
              <JournalQuartile />
              <PremiumFooter3D />
            </>
          } />
          <Route path="/academic-ai-remover" element={
            <>
              <SEOHead 
                title="AI Content Remover for Research Papers | Free AI Detector & Editor for PhD Thesis - Gaply"
                description="Advanced AI content detection and removal tool for academic writing. Free AI detector and editor for PhD thesis, research papers, and dissertations. Bypass Turnitin, GPTZero, Crossplag with our AI paraphrase detector."
                keywords="AI content remover for research papers, free AI detector and editor for PhD thesis, AI writing detection tool for academic writing, detect AI plagiarism in research paper, AI paraphrase detector academic, plagiarism checking service for thesis, best plagiarism checker for research papers, remove plagiarism from dissertation, academic text originality checker"
              />
              <AcademicAIRemoverPage />
            </>
          } />
          <Route path="/paper-search" element={
            <>
              <SEOHead 
                title="Research Paper Search Engine | Academic Paper Writing Assistance - Gaply"
                description="Comprehensive research paper search engine for academic writing. Find relevant papers, get research paper evaluation help, academic paper writing assistance, and scientific writing support online."
                keywords="research paper writing service, research paper evaluation help, academic paper writing assistance, scientific writing support online, research methodology help, literature review writing service, best site for research guidance, how to write a research proposal, funding proposal writing help"
              />
              <PaperSearchPage />
            </>
          } />
          <Route path="/journal-matching" element={
            <>
              <SEOHead 
                title="Journal Finder for Scopus | Elsevier Journal Suggestion Tool - Gaply"
                description="Professional journal matching service for academic publishing. Journal finder for Scopus, Elsevier journal suggestion tool, SCI journal recommendation service. Find the perfect journal for your paper."
                keywords="journal finder for Scopus, Elsevier journal suggestion tool, SCI journal recommendation service, find journal for my paper, journal submission assistance service, how to submit paper to journal, conference paper preparation help, conference presentation coaching, publish paper in IEEE journal"
              />
              <JournalMatchingPage />
            </>
          } />
          <Route path="/document-orchestrator" element={
            <>
              <SEOHead 
                title="Document Analysis Orchestrator | Gaply"
                description="Run chunk-by-chunk manuscript analysis with strict JSON output and publication chance estimation."
                keywords="document analysis orchestrator, publication chance estimator, journal guidelines compliance, academic AI analysis"
              />
              <DocumentOrchestratorPage />
            </>
          } />
          <Route path="/final-orchestrator" element={
            <>
              <SEOHead 
                title="Document Analysis Orchestrator | PublishReady - Gaply"
                description="Run chunk-level analysis with strict JSON output and publication chance estimation. Uses gaply-orchestrator for full manuscript analysis."
                keywords="document analysis orchestrator, publication chance, PublishReady, gaply-orchestrator, journal guidelines"
              />
              <DocumentOrchestratorPage />
            </>
          } />
          <Route path="/manuscript-upload" element={
            <>
              <SEOHead 
                title="Upload Manuscript | Final Analysis Suite - Gaply"
                description="Upload manuscript files, add journal links, and generate a clean report with Gaply chat support."
                keywords="final analysis suite, manuscript upload, journal submission assistant, gaply chat"
              />
              <ManuscriptOrchestratorPage />
            </>
          } />
          <Route path="/statistical-research" element={
            <>
              <SEOHead 
                title="Statistical Research Orchestrator | Gaply"
                description="Advanced statistical analysis orchestrator for research. Upload datasets, get test recommendations, interpretations, and comprehensive HTML reports."
                keywords="statistical analysis, research orchestrator, statistical tests, data analysis, research methodology"
              />
              <StatisticalResearchOrchestratorPage />
            </>
          } />
          <Route path="/datamaestro-pro" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="DataMaestro Pro - AI Statistical Analysis | Gaply"
                  description="AI-powered statistical analysis for academic research. Upload datasets, get smart test recommendations, publication-ready results with tables, charts, and downloadable reports."
                  keywords="DataMaestro Pro, statistical analysis, AI research analysis, SPSS alternative, data analysis, academic research tool"
                />
                <DataMaestroProPage />
              </>
            </ProtectedRoute>
          } />
          <Route path="/features" element={
            <>
              <SEOHead 
                title="Academic Research Features | Thesis Writing, AI Detection, Journal Matching - Gaply"
                description="Comprehensive academic research features including thesis writing help, AI content detection, journal matching, research paper assistance, proofreading services, and statistical analysis support."
                keywords="academic research features, thesis writing help, AI content detection, journal matching, research paper assistance, proofreading services, statistical analysis support, dissertation editing, academic writing tools"
              />
              <FeaturesPage />
            </>
          } />
          <Route path="/pricing" element={
            <>
              <SEOHead 
                title="Academic Research Platform Pricing | Thesis Writing Services - Gaply"
                description="Affordable pricing for professional academic research services. Choose from our thesis writing, AI detection, journal matching, and research paper assistance packages."
                keywords="academic research platform pricing, thesis writing services pricing, AI detection pricing, journal matching pricing, research paper assistance pricing, dissertation editing pricing"
              />
              <PricingSection />
            </>
          } />
          <Route path="/download" element={
            <>
              <SEOHead
                title="Download Gaply Desktop | Mac & Windows - Gaply"
                description="Download Gaply desktop app for Mac (Apple Silicon & Intel) and Windows. Same features as the web app — research paper writing, thesis help, journal matching."
                keywords="download Gaply, Gaply desktop app, Mac app, Windows app, research platform download"
              />
              <DownloadsPage />
            </>
          } />
          <Route path="/contact" element={
            <>
              <SEOHead 
                title="Contact Gaply | Academic Research Support & Expert Consultation"
                description="Get in touch with Gaply's academic research experts. Contact us for thesis writing help, journal matching, research paper editing, AI content detection, and statistical analysis support."
                keywords="contact Gaply, academic research support, thesis writing help contact, journal matching support, research paper editing contact, AI content detection support"
              />
              <ContactPage />
            </>
          } />
          <Route path="/blog" element={
            <>
              <SEOHead
                title="Blog | Research Writing & Academic Publishing Insights - Gaply"
                description="Expert insights on research paper writing, PhD thesis guidance, journal matching, and academic publishing. Tips from researchers and editors."
                keywords="research paper writing blog, thesis writing tips, journal matching, academic publishing, PhD research, research objectives, findings research paper"
              />
              <BlogPage />
            </>
          } />
          <Route path="/blog/:slug" element={<BlogPostPage />} />
          <Route path="/privacy" element={
            <>
              <SEOHead
                title="Privacy Policy | Gaply"
                description="Gaply privacy policy covering data processing, temporary storage, optional saving, third-party services, and user rights."
                keywords="Gaply privacy policy, data processing, file retention, academic research privacy"
              />
              <PrivacyPolicyPage />
            </>
          } />
          <Route path="/terms" element={
            <>
              <SEOHead
                title="Terms of Service | Gaply"
                description="Gaply terms of service covering accounts, content, billing, acceptable use, and legal policies."
                keywords="Gaply terms of service, user agreement, acceptable use, billing terms"
              />
              <TermsOfServicePage />
            </>
          } />
          <Route path="/career" element={
            <>
              <SEOHead 
                title="Join Our Academic Research Team | Career Opportunities - Gaply"
                description="Join our team of academic research experts. Career opportunities for research specialists, thesis writing experts, and academic consultants. Work with PhD students and researchers worldwide."
                keywords="academic research careers, thesis writing jobs, research specialist positions, academic consultant jobs, PhD research opportunities, academic writing careers"
              />
              <CareerPage />
            </>
          } />
                <Route path="/hire-expert" element={
                  <>
                    <SEOHead 
                      title="Hire an Expert | Professional Academic Research Services - Gaply"
                      description="Hire our team of academic research experts for personalized thesis writing, journal matching, AI detection, and research paper assistance. Get professional help for your PhD, Master's, or research projects."
                      keywords="hire academic expert, thesis writing expert, research paper expert, PhD thesis help, dissertation expert, academic consultant, research specialist, thesis writing service, journal matching expert, AI detection expert"
                    />
                    <HireExpertPage />
                  </>
                } />
                <Route path="/search-results" element={
                  <>
                    <SEOHead 
                      title="Expert Search Results | Find Academic Research Experts - Gaply"
                      description="Browse our comprehensive database of academic research experts. Find the perfect specialist for your thesis writing, journal matching, AI detection, and research paper assistance needs."
                      keywords="expert search results, academic research experts, thesis writing experts, research paper experts, PhD thesis help, dissertation experts, academic consultants, research specialists"
                    />
                    <ExpertSearchResultsPage />
                  </>
                } />
          <Route path="/login" element={
            <>
              <SEOHead 
                title="Login to Gaply | Access Your Academic Research Platform"
                description="Login to your Gaply account to access premium academic research features including thesis writing tools, AI detection, journal matching, and research paper assistance."
                keywords="Gaply login, academic research platform login, thesis writing account, AI detection login, journal matching login, research paper assistance login"
              />
              <LoginPage onLoginSuccess={handleAuthSuccess} onSwitchToSignup={() => window.location.href = '/signup'} />
            </>
          } />
          <Route path="/signup" element={
            <>
              <SEOHead 
                title="Sign Up for Gaply | Create Your Academic Research Account"
                description="Sign up for Gaply to access professional academic research tools including thesis writing assistance, AI content detection, journal matching, and research paper support."
                keywords="Gaply signup, academic research platform signup, thesis writing account creation, AI detection signup, journal matching signup, research paper assistance signup"
              />
              <SignupPage onSignupSuccess={handleAuthSuccess} onSwitchToLogin={() => window.location.href = '/login'} />
            </>
          } />
          <Route path="/premium" element={
            <>
              <SEOHead 
                title="Premium Academic Research Services | Advanced Thesis Writing & AI Detection - Gaply"
                description="Upgrade to premium academic research services with advanced thesis writing tools, enhanced AI content detection, priority journal matching, and expert research paper assistance."
                keywords="premium academic research services, advanced thesis writing, enhanced AI detection, priority journal matching, expert research assistance, premium dissertation editing"
              />
              <EnhancedPremiumPage />
            </>
          } />
          <Route path="/packages" element={
            <>
              <SEOHead 
                title="Choose Your Academic Research Package | Thesis Writing Plans - Gaply"
                description="Select the perfect academic research package for your needs. Choose from thesis writing plans, AI detection packages, journal matching services, and research paper assistance options."
                keywords="academic research packages, thesis writing plans, AI detection packages, journal matching services, research paper assistance plans, dissertation editing packages"
              />
              <PackageSelection onClose={() => window.location.href = '/'} />
            </>
          } />
          <Route path="/account" element={
            <>
              <SEOHead 
                title="My Account | Manage Your Academic Research Services - Gaply"
                description="Manage your Gaply account, view your academic research services, track your thesis writing progress, and access your AI detection and journal matching tools."
                keywords="Gaply account management, academic research account, thesis writing account, AI detection account, journal matching account, research assistance account"
              />
              <Overview />
            </>
          } />
          <Route path="/my-account" element={
            <>
              <SEOHead 
                title="My Account | Manage Your Academic Research Services - Gaply"
                description="Manage your Gaply account, view your academic research services, track your thesis writing progress, and access your AI detection and journal matching tools."
                keywords="Gaply account management, academic research account, thesis writing account, AI detection account, journal matching account, research assistance account"
              />
              <Overview />
            </>
          } />
          <Route path="/dashboard" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="Dashboard | Academic Research Tools & Progress Tracking - Gaply"
                  description="Access your Gaply dashboard to manage academic research projects, track thesis writing progress, monitor AI detection results, and view journal matching recommendations."
                  keywords="academic research dashboard, thesis writing dashboard, AI detection dashboard, journal matching dashboard, research progress tracking, academic project management"
                />
                <Overview />
              </>
            </ProtectedRoute>
          } />
          {/* Local-only dashboard preview without auth redirect */}
          <Route path="/dashboard-preview" element={
            <>
              <SEOHead 
                title="Dashboard Preview | Gaply"
                description="Local preview of the Feature Usage Overview dashboard layout."
                keywords="dashboard preview, feature usage overview, gaply"
              />
              <Overview />
            </>
          } />
          <Route path="/dashboard/publishready" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="PublishReady | Upload Manuscript & Full Analysis - Gaply"
                  description="Upload your manuscript (PDF, DOCX, TXT), add journal link, and run full analysis with referee-style review, report, and chat. Uses gaply-orchestrator."
                  keywords="PublishReady, manuscript upload, document analysis, publication chance, referee review, gaply-orchestrator"
                />
                <DashboardLayout pageTitle="PublishReady">
                  <ManuscriptOrchestratorPage />
                </DashboardLayout>
              </>
            </ProtectedRoute>
          } />
          <Route path="/dashboard/datamaestro" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="DataMaestro | AI Statistical Analysis - Gaply"
                  description="AI-powered statistical analysis for academic research. Upload datasets, get smart test recommendations, publication-ready results with tables, charts, and downloadable reports."
                  keywords="DataMaestro, statistical analysis, AI research analysis, SPSS alternative, data analysis, academic research tool"
                />
                <DashboardLayout pageTitle="DataMaestro">
                  <DataMaestroProPage />
                </DashboardLayout>
              </>
            </ProtectedRoute>
          } />
          <Route path="/dashboard/journal-verify" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="Verify Journal Authenticity | Journal Check - Gaply"
                  description="AI-powered journal verification. Enter a journal URL or DOI to detect if it's real or predatory. Get detailed authenticity reports with key parameters."
                  keywords="journal verification, predatory journal detection, journal authenticity check, DOI verification, research journal validation"
                />
                <DashboardLayout pageTitle="Journal Verification">
                  <JournalVerifyPage />
                </DashboardLayout>
              </>
            </ProtectedRoute>
          } />
          <Route path="/dashboard/projects" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="Projects | Academic Research - Gaply"
                  description="View your academic research projects and their status."
                  keywords="research projects, academic projects, project management"
                />
                <ProjectsPage />
              </>
            </ProtectedRoute>
          } />
          <Route path="/dashboard/usage" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="Feature Usage Overview | Gaply"
                  description="View your PublishReady and DataMaestro usage metrics."
                  keywords="usage, PublishReady, DataMaestro, feature usage"
                />
                <UsagePage />
              </>
            </ProtectedRoute>
          } />
          <Route path="/dashboard/settings" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="Settings | Gaply"
                  description="Manage your account, billing, profile, and get support."
                  keywords="settings, account, billing, profile, support"
                />
                <SettingsPage />
              </>
            </ProtectedRoute>
          } />
          <Route path="/dashboard/billing" element={
            <ProtectedRoute>
              <>
                <SEOHead 
                  title="Billing | Gaply"
                  description="View your billing history and recent transactions."
                  keywords="billing, transactions, subscription, payments"
                />
                <BillingPage />
              </>
            </ProtectedRoute>
          } />
        </Routes>
        <DownloadPopUp />
    </div>
  );
};

// Main App component that provides AuthProvider and ThemeProvider
const App: React.FC = () => {
  return (
    <AuthProvider>
      <ThemeProvider>
        <AppContent />
      </ThemeProvider>
    </AuthProvider>
  );
};

export default App;