import React, { useEffect, useState } from 'react';
import { Routes, Route } from 'react-router-dom';
import './App.css';
import './hero-animations.css';
import './hero-new.css';
import './responsive.css';
import ThreeJSGlobe from './components/ThreeJSGlobe';
import HeroToSecondTransition from './components/HeroToSecondTransition';
import FreeFeatures3D from './components/FreeFeatures3D';
import PremiumFeatures3D from './components/PremiumFeatures3D';
import QuartileAnalysis3D from './components/QuartileAnalysis3D';
import PremiumFooter3D from './components/PremiumFooter3D';
import PremiumPage from './components/PremiumPage';
import UserDashboard from './components/UserDashboard';
import PackageSelection from './components/PackageSelection';
import LoginPage from './components/LoginPage';
import SignupPage from './components/SignupPage';
import AcademicAIRemoverPage from './components/AcademicAIRemoverPage';
import PaperSearchPage from './components/PaperSearchPage';
import JournalMatchingPage from './components/JournalMatchingPage';
import FeaturesPage from './components/FeaturesPage';
import ContactPage from './components/ContactPage';
import CareerPage from './components/CareerPage';
import HireExpertPage from './components/HireExpertPage';
import ExpertSearchResultsPage from './components/ExpertSearchResultsPage';
import PricingSection from './components/PricingSection';
import { AuthProvider, useAuth } from './contexts/AuthContext';

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
const AppleHeader: React.FC<{ theme: 'light' | 'dark' }> = ({ theme }) => {
  const textColor = theme === 'dark' ? '#ffffff' : '#000000';
  const textShadow = theme === 'dark' ? '0 1px 2px rgba(0,0,0,0.25)' : '0 1px 2px rgba(255,255,255,0.25)';

  return (
    <header
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        zIndex: 1000,
        background: 'rgba(0, 0, 0, 0.8)',
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)',
        borderBottom: '1px solid rgba(255, 255, 255, 0.1)',
        padding: window.innerWidth <= 480 ? '8px 0' : '12px 0'
      }}
    >
             <div
               style={{
                 maxWidth: 1200,
                 margin: '0 auto',
                 padding: window.innerWidth <= 480 ? '0 16px' : '0 24px',
                 display: 'flex',
                 alignItems: 'center',
                 justifyContent: 'space-between',
                 flexDirection: window.innerWidth <= 480 ? 'column' : 'row',
                 gap: window.innerWidth <= 480 ? '12px' : '0'
               }}
             >
               <div style={{ fontWeight: 800, letterSpacing: '.02em' }}>
                 <h1 style={{
                   margin: 0,
                   fontSize: window.innerWidth <= 480 ? '16px' : '18px',
                   color: textColor,
                   textShadow,
                   fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
                 }}>
                   Gaply
                 </h1>
               </div>
        <nav style={{ 
          display: 'flex', 
          gap: window.innerWidth <= 480 ? '16px' : '32px', 
          alignItems: 'center',
          flexWrap: window.innerWidth <= 480 ? 'wrap' : 'nowrap',
          justifyContent: window.innerWidth <= 480 ? 'center' : 'flex-end'
        }}>
                 {[
                   { label: 'Features', href: '/features' },
                   { label: 'Pricing', href: '/pricing' },
                   { label: 'Career', href: '/career' },
                 ].map((l) => (
                   <a
                     key={l.label}
                     href={l.href}
                     style={{
                       color: textColor,
                       textDecoration: 'none',
                       fontWeight: 500,
                       fontSize: window.innerWidth <= 480 ? '12px' : '14px',
                       letterSpacing: '.01em',
                       opacity: 0.8,
                       padding: window.innerWidth <= 480 ? '6px 8px' : '8px 12px',
                       borderRadius: 8,
                       transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                       fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
                     }}
                     onMouseEnter={(e) => {
                       e.currentTarget.style.opacity = '1';
                       e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)';
                       e.currentTarget.style.transform = 'translateY(-1px)';
                     }}
                     onMouseLeave={(e) => {
                       e.currentTarget.style.opacity = '0.8';
                       e.currentTarget.style.backgroundColor = 'transparent';
                       e.currentTarget.style.transform = 'translateY(0)';
                     }}
                   >
                     {l.label}
                   </a>
                 ))}
                 
                 {/* Right side - Hire Expert Button */}
                 <button
                   onClick={() => window.location.href = '/hire-expert'}
                   style={{
                     background: 'linear-gradient(135deg, #ffffff 0%, #f8f9fa 25%, #e9ecef 50%, #dee2e6 75%, #ced4da 100%)',
                     border: '1px solid rgba(0, 0, 0, 0.08)',
                     borderRadius: '24px',
                     padding: window.innerWidth <= 480 ? '10px 18px' : '12px 24px',
                     fontSize: window.innerWidth <= 480 ? '13px' : '15px',
                     fontWeight: '700',
                     color: '#1a1a1a',
                     fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                     letterSpacing: '-0.02em',
                     cursor: 'pointer',
                     boxShadow: '0 8px 32px rgba(0, 0, 0, 0.12), 0 2px 8px rgba(0, 0, 0, 0.08), inset 0 1px 0 rgba(255, 255, 255, 0.8)',
                     transition: 'all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                     textTransform: 'none',
                     minWidth: window.innerWidth <= 480 ? '130px' : '150px',
                     height: window.innerWidth <= 480 ? '42px' : '48px',
                     display: 'flex',
                     alignItems: 'center',
                     justifyContent: 'center',
                     marginLeft: window.innerWidth <= 480 ? '0' : '24px',
                     position: 'relative',
                     overflow: 'hidden',
                     backdropFilter: 'blur(20px)',
                     WebkitBackdropFilter: 'blur(20px)'
                   }}
                   onMouseEnter={(e) => {
                     e.currentTarget.style.transform = 'translateY(-3px) scale(1.05)';
                     e.currentTarget.style.boxShadow = '0 16px 48px rgba(0, 0, 0, 0.18), 0 4px 16px rgba(0, 0, 0, 0.12), inset 0 1px 0 rgba(255, 255, 255, 0.9)';
                     e.currentTarget.style.background = 'linear-gradient(135deg, #ffffff 0%, #f8f9fa 20%, #e9ecef 40%, #dee2e6 60%, #ced4da 80%, #adb5bd 100%)';
                     e.currentTarget.style.borderColor = 'rgba(0, 0, 0, 0.15)';
                     e.currentTarget.style.color = '#000000';
                     
                     // Trigger shine effect
                     const shineElement = e.currentTarget.querySelector('div');
                     if (shineElement) {
                       shineElement.style.left = '100%';
                     }
                   }}
                   onMouseLeave={(e) => {
                     e.currentTarget.style.transform = 'translateY(0) scale(1)';
                     e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 0, 0, 0.12), 0 2px 8px rgba(0, 0, 0, 0.08), inset 0 1px 0 rgba(255, 255, 255, 0.8)';
                     e.currentTarget.style.background = 'linear-gradient(135deg, #ffffff 0%, #f8f9fa 25%, #e9ecef 50%, #dee2e6 75%, #ced4da 100%)';
                     e.currentTarget.style.borderColor = 'rgba(0, 0, 0, 0.08)';
                     e.currentTarget.style.color = '#1a1a1a';
                     
                     // Reset shine effect
                     const shineElement = e.currentTarget.querySelector('div');
                     if (shineElement) {
                       shineElement.style.left = '-100%';
                     }
                   }}
                   onMouseDown={(e) => {
                     e.currentTarget.style.transform = 'translateY(-1px) scale(1.02)';
                     e.currentTarget.style.boxShadow = '0 4px 16px rgba(0, 0, 0, 0.15), 0 1px 4px rgba(0, 0, 0, 0.1), inset 0 1px 0 rgba(255, 255, 255, 0.7)';
                   }}
                   onMouseUp={(e) => {
                     e.currentTarget.style.transform = 'translateY(-3px) scale(1.05)';
                     e.currentTarget.style.boxShadow = '0 16px 48px rgba(0, 0, 0, 0.18), 0 4px 16px rgba(0, 0, 0, 0.12), inset 0 1px 0 rgba(255, 255, 255, 0.9)';
                   }}
                 >
                   <span style={{
                     position: 'relative',
                     zIndex: 2,
                     fontWeight: '500',
                     fontSize: window.innerWidth <= 480 ? '12px' : '14px',
                     letterSpacing: '.01em',
                     fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
                   }}>
                     Hire an expert
                   </span>
                   
                   {/* Premium shine effect */}
                   <div style={{
                     position: 'absolute',
                     top: 0,
                     left: '-100%',
                     width: '100%',
                     height: '100%',
                     background: 'linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.4), transparent)',
                     transition: 'left 0.6s ease',
                     zIndex: 1
                   }} />
                 </button>
        </nav>
      </div>
      </header>
  );
};

// Apple-style Hero Section
const AppleHeroSection: React.FC = () => {
  return (
    <section style={{ 
      minHeight: '100vh',
      background: 'linear-gradient(135deg, #000000 0%, #0A0A0A 50%, #000000 100%)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      position: 'relative',
      overflow: 'hidden',
      paddingTop: '80px'
    }}>
      {/* Interactive Globe - Hard Right Position */}
      <div style={{
        position: 'absolute',
        top: '50%',
        right: '5%',
        transform: 'translate(0, -50%)',
        zIndex: 2,
        width: window.innerWidth <= 480 ? '300px' : window.innerWidth <= 768 ? '400px' : '500px',
        height: window.innerWidth <= 480 ? '300px' : window.innerWidth <= 768 ? '400px' : '500px',
        cursor: 'grab'
      }}>
        <ThreeJSGlobe />
      </div>

      {/* Professional Text Content - Left Side */}
      <div style={{
        position: 'absolute',
        top: '50%',
        left: '15%',
        transform: 'translate(0, -50%)',
        zIndex: 3,
        maxWidth: window.innerWidth <= 768 ? '70%' : '500px',
        color: '#ffffff'
      }}>
        {/* Main Description */}
        <div style={{
          marginBottom: '24px',
          position: 'relative',
          textAlign: 'left'
        }}>
          <h1 style={{
            fontSize: window.innerWidth <= 480 ? 'clamp(1.6rem, 6.4vw, 2.4rem)' : window.innerWidth <= 768 ? 'clamp(2rem, 4.8vw, 3.2rem)' : '4rem',
            fontWeight: '300',
            color: '#ffffff',
            margin: 0,
            lineHeight: '1.1',
            letterSpacing: '-0.02em',
            fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
          }}>
            AI-Powered Academic Research Platform
          </h1>
        </div>

        {/* Stats Section */}
        <div style={{
          marginTop: '24px',
          maxWidth: '400px'
        }}>
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '8px'
          }}>
            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px'
            }}>
              <span style={{
                fontSize: 'clamp(1.2rem, 2vw, 1.5rem)',
                fontWeight: '300',
                color: '#ffffff',
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif',
                lineHeight: '1.2',
                minWidth: '60px'
              }}>
                10K+
              </span>
              <span style={{
                fontSize: 'clamp(0.7rem, 1.5vw, 0.875rem)',
                fontWeight: '300',
                color: 'rgba(255, 255, 255, 0.6)',
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif',
                letterSpacing: '0.01em',
                lineHeight: '1.3'
              }}>
                Research Papers Analyzed
              </span>
            </div>

            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px'
            }}>
              <span style={{
                fontSize: 'clamp(1.2rem, 2vw, 1.5rem)',
                fontWeight: '300',
                color: '#ffffff',
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif',
                lineHeight: '1.2',
                minWidth: '60px'
              }}>
                500+
              </span>
              <span style={{
                fontSize: 'clamp(0.7rem, 1.5vw, 0.875rem)',
                fontWeight: '300',
                color: 'rgba(255, 255, 255, 0.6)',
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif',
                letterSpacing: '0.01em',
                lineHeight: '1.3'
              }}>
                Journals Matched
              </span>
            </div>

            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px'
            }}>
              <span style={{
                fontSize: 'clamp(1.2rem, 2vw, 1.5rem)',
                fontWeight: '300',
                color: '#ffffff',
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif',
                lineHeight: '1.2',
                minWidth: '60px'
              }}>
                95%
              </span>
              <span style={{
                fontSize: 'clamp(0.7rem, 1.5vw, 0.875rem)',
                fontWeight: '300',
                color: 'rgba(255, 255, 255, 0.6)',
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif',
                letterSpacing: '0.01em',
                lineHeight: '1.3'
              }}>
                AI Detection Accuracy
              </span>
            </div>
          </div>
        </div>

        {/* Get Premium Button */}
        <div style={{
          marginTop: '60px',
          display: 'flex',
          justifyContent: 'flex-start',
          marginLeft: '-40px'
        }}>
          <button 
            onClick={() => window.location.href = '/login'}
            style={{
              background: 'linear-gradient(135deg, #007AFF 0%, #5856D6 50%, #AF52DE 100%)',
              border: 'none',
              borderRadius: '28px',
              padding: '18px 36px',
              fontSize: '18px',
              fontWeight: '700',
              color: '#ffffff',
              fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
              letterSpacing: '-0.01em',
              textTransform: 'none',
              cursor: 'pointer',
              boxShadow: '0 8px 30px rgba(0, 122, 255, 0.3)',
              transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
              position: 'relative',
              overflow: 'hidden',
              textShadow: '0 1px 2px rgba(0, 0, 0, 0.1)',
              minWidth: '200px',
              height: '56px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.transform = 'translateY(-2px) scale(1.02)';
              e.currentTarget.style.boxShadow = '0 12px 40px rgba(0, 122, 255, 0.4)';
              e.currentTarget.style.background = 'linear-gradient(135deg, #0056CC 0%, #4A4AC7 50%, #9B4BC7 100%)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.transform = 'translateY(0) scale(1)';
              e.currentTarget.style.boxShadow = '0 8px 30px rgba(0, 122, 255, 0.3)';
              e.currentTarget.style.background = 'linear-gradient(135deg, #007AFF 0%, #5856D6 50%, #AF52DE 100%)';
            }}
          >
            <span style={{
              fontSize: '18px',
              fontWeight: '700',
              letterSpacing: '-0.01em',
              fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif'
            }}>
              Get Premium
            </span>
          </button>
        </div>
      </div>
    </section>
  );
};

// Inner App component that uses useAuth
const AppContent: React.FC = () => {
  const [headerTheme] = useState<'light'|'dark'>('dark');
  const [currentPath, setCurrentPath] = useState(window.location.pathname);

  // Call the function to remove the floating orb when the component mounts
  useEffect(() => {
    removeFloatingOrb();
    
    // Listen for route changes
    const handleRouteChange = () => {
      setCurrentPath(window.location.pathname);
    };
    
    window.addEventListener('popstate', handleRouteChange);
    return () => window.removeEventListener('popstate', handleRouteChange);
  }, []);

  const handleAuthSuccess = (token: string, userData: any) => {
    window.location.href = '/packages';
  };

  // Hide header for hire-expert and search-results pages
  const shouldShowHeader = currentPath !== '/hire-expert' && currentPath !== '/search-results';

  return (
    <div className="App" style={{
      background: '#000000',
      color: '#ffffff',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif',
      overflowX: 'hidden'
    }}>
      {shouldShowHeader && <AppleHeader theme={headerTheme} />}
      <Routes>
          <Route path="/" element={
            <>
              <SEOHead 
                title="Gaply - AI-Powered Academic Research Platform | Thesis Writing, Journal Matching, AI Detection"
                description="Professional AI-powered academic research platform offering thesis writing help, journal matching, AI content detection, plagiarism checking, dissertation editing, and research paper assistance for PhD students and researchers worldwide."
                keywords="AI content remover for research papers, free AI detector and editor for PhD thesis, AI writing detection tool for academic writing, detect AI plagiarism in research paper, AI paraphrase detector academic, plagiarism checking service for thesis, best plagiarism checker for research papers, remove plagiarism from dissertation, academic text originality checker, help with thesis writing and formatting, dissertation writing service online, PhD thesis writing help, master's thesis editing service, dissertation proofreading and formatting, thesis structure and formatting guidelines, doctoral dissertation consultation, academic thesis writing assistance, research paper writing service, research paper evaluation help, academic paper writing assistance, scientific writing support online, research methodology help, literature review writing service, best site for research guidance, how to write a research proposal, funding proposal writing help, journal finder for Scopus, Elsevier journal suggestion tool, SCI journal recommendation service, find journal for my paper, journal submission assistance service, how to submit paper to journal, conference paper preparation help, conference presentation coaching, publish paper in IEEE journal, academic proofreading and editing service, research paper editing service, thesis proofreading help, professional dissertation editor, edit academic paper online, grammar check for scholarly writing, academic copyediting service, SPSS statistical analysis help, data analysis service for researchers, statistical analysis assistance for thesis, SPSS tutorial for dissertation, data interpretation help for research, quantitative analysis support for PhD, statistics help for academic research"
              />
              <AppleHeroSection />
              <HeroToSecondTransition />
              <FreeFeatures3D />
              <PremiumFeatures3D />
              <QuartileAnalysis3D />
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
              <PremiumPage />
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
              <UserDashboard />
            </>
          } />
          <Route path="/my-account" element={
            <>
              <SEOHead 
                title="My Account | Manage Your Academic Research Services - Gaply"
                description="Manage your Gaply account, view your academic research services, track your thesis writing progress, and access your AI detection and journal matching tools."
                keywords="Gaply account management, academic research account, thesis writing account, AI detection account, journal matching account, research assistance account"
              />
              <UserDashboard />
            </>
          } />
          <Route path="/dashboard" element={
            <>
              <SEOHead 
                title="Dashboard | Academic Research Tools & Progress Tracking - Gaply"
                description="Access your Gaply dashboard to manage academic research projects, track thesis writing progress, monitor AI detection results, and view journal matching recommendations."
                keywords="academic research dashboard, thesis writing dashboard, AI detection dashboard, journal matching dashboard, research progress tracking, academic project management"
              />
              <UserDashboard />
            </>
          } />
        </Routes>
    </div>
  );
};

// Main App component that provides AuthProvider
const App: React.FC = () => {
  return (
    <AuthProvider>
      <AppContent />
    </AuthProvider>
  );
};

export default App;