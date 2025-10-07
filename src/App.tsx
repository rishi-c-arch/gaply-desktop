import React, { useEffect, useRef, useState } from 'react';
import gsap from 'gsap';
import './App.css';
import './hero-animations.css';
import './hero-new.css';
import ThreeJSGlobe from './components/ThreeJSGlobe';
import PremiumHeroText from './components/PremiumHeroText';
import HeroToSecondTransition from './components/HeroToSecondTransition';
import FreeFeatures3D from './components/FreeFeatures3D';
import QuartileAnalysis3D from './components/QuartileAnalysis3D';
import PremiumFooter3D from './components/PremiumFooter3D';
import PremiumPage from './components/PremiumPage';
import UserDashboard from './components/UserDashboard';
import PackageSelection from './components/PackageSelection';
import PremiumFeatureModal from './components/PremiumFeatureModal';
import LoginPage from './components/LoginPage';
import SignupPage from './components/SignupPage';
import { AuthProvider, useAuth } from './contexts/AuthContext';

// TypeScript declaration for window function
declare global {
  interface Window {
    showThirdSection?: () => void;
    openPopup?: (section: string) => void;
  }
}

// Header Component
type HeaderProps = { theme: 'light' | 'dark' };

const Header: React.FC<HeaderProps> = ({ theme }) => {
  const textColor = theme === 'dark' ? '#e5e7eb' : '#111827';
  const textShadow = theme === 'dark' ? '0 1px 2px rgba(0,0,0,0.25)' : '0 1px 2px rgba(255,255,255,0.25)';
  
  const handleGetPremium = () => {
    // Always redirect to login first
    window.location.href = '/login';
  };

  return (
    <header 
      className="header"
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        zIndex: 1000,
        background: 'transparent',
        color: textColor,
        backdropFilter: 'none',
        WebkitBackdropFilter: 'none',
        borderBottom: 'none'
      }}
    >
      <div 
        className="header-content"
        style={{
          maxWidth: 1200,
          margin: '0 auto',
          padding: '10px 16px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between'
        }}
      >
        <div className="logo" style={{ fontWeight: 800, letterSpacing: '.02em', transformStyle: 'preserve-3d' }}>
          <h1 style={{ margin: 0, fontSize: 16, color: '#ffffff', transform: 'translateZ(6px)', textShadow, mixBlendMode: 'difference' as any }}>Gaply</h1>
        </div>
        <nav className="nav-menu" style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
          {[
            { label: 'Features', href: '#features' },
            { label: 'Pricing', href: '#pricing' },
            { label: 'Contact', href: '#contact' },
          ].map((l) => (
            <a
              key={l.label}
              href={l.href}
              style={{
                color: '#ffffff',
                mixBlendMode: 'difference',
                textDecoration: 'none',
                fontWeight: 600,
                letterSpacing: '.02em',
                opacity: 0.9,
                padding: '4px 8px',
                borderRadius: 8,
                transition: 'transform .15s ease, opacity .15s ease, background-color .15s ease'
              }}
              onMouseEnter={(e) => { e.currentTarget.style.opacity = '1'; e.currentTarget.style.transform = 'translateZ(6px)'; e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.06)'; }}
              onMouseLeave={(e) => { e.currentTarget.style.opacity = '0.9'; e.currentTarget.style.transform = 'none'; e.currentTarget.style.backgroundColor = 'transparent'; }}
            >
              {l.label}
            </a>
          ))}
          <button 
            onClick={handleGetPremium}
            className="premium-btn"
            style={{
              color: '#ffffff',
              mixBlendMode: 'difference',
              background: 'transparent',
              padding: '6px 12px',
              borderRadius: 10,
              textDecoration: 'none',
              fontWeight: 700,
              fontSize: 12,
              border: '1px solid currentColor',
              cursor: 'pointer'
            }}
          >
            Get Premium
          </button>
        </nav>
      </div>
    </header>
  );
};

// Hero Section Component
const HeroSection: React.FC = () => {
  const heroRef = useRef<HTMLElement>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);
  const statsRef = useRef<HTMLDivElement>(null);
  const sphereRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // 3D motion for sphere background
    if (sphereRef.current) {
      gsap.to(sphereRef.current, {
        rotation: 360,
        duration: 20,
        ease: "none",
        repeat: -1
      });
    }

    // Initial animations with proper timing
    gsap.fromTo(titleRef.current, 
      { y: 50, opacity: 0 },
      { y: 0, opacity: 1, duration: 1, ease: "power3.out", delay: 0.2 }
    );

    gsap.fromTo(statsRef.current?.children || [], 
      { y: 30, opacity: 0 },
      { y: 0, opacity: 1, duration: 0.6, ease: "power3.out", stagger: 0.08, delay: 0.4 }
    );
  }, []);

  return (
    <section ref={heroRef} className="hero-section" style={{ paddingTop: 72 }}>
      {/* Sphere Background */}
      <div ref={sphereRef} className="sphere-background"></div>
      
      {/* Three.js Globe */}
      <ThreeJSGlobe />
      
      {/* Premium Hero Text Animation */}
      <PremiumHeroText />
      
      {/* Stats Below Globe */}
      <div className="stats-below-globe">
        <div className="stat-item">
          <h3>10K+</h3>
          <p>RESEARCH PAPERS</p>
        </div>
        <div className="stat-item">
          <h3>500+</h3>
          <p>JOURNALS</p>
        </div>
        <div className="stat-item">
          <h3 style={{ color: '#bdbdbd' }}>95%</h3>
          <p style={{ color: '#bdbdbd' }}>ACCURACY</p>
        </div>
      </div>
    </section>
  );
};

// (legacy SecondSection removed)


// Main App component
const App: React.FC = () => {
  const { isLoading } = useAuth();
  const [headerTheme, setHeaderTheme] = useState<'light'|'dark'>('dark');
  const [currentPage, setCurrentPage] = useState('home');
  const [premiumModal, setPremiumModal] = useState<{ type: 'gapFinder' | 'deepEvaluation' | null }>({ type: null });
  const [showAuth, setShowAuth] = useState<'login' | 'signup' | null>(null);

  // Simple routing based on URL
  useEffect(() => {
    const path = window.location.pathname;
    if (path === '/premium') {
      setCurrentPage('premium');
      setShowAuth(null);
    } else if (path === '/packages') {
      setCurrentPage('packages');
      setShowAuth(null);
    } else if (path === '/dashboard') {
      setCurrentPage('dashboard');
      setShowAuth(null);
    } else if (path === '/login') {
      setCurrentPage('home');
      setShowAuth('login');
    } else if (path === '/signup') {
      setCurrentPage('home');
      setShowAuth('signup');
    } else {
      setCurrentPage('home');
      setShowAuth(null);
    }
  }, []);

  useEffect(() => {
    const toLuminance = (r: number, g: number, b: number) => {
      const a = [r, g, b].map(v => {
        v /= 255;
        return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
      });
      return 0.2126 * a[0] + 0.7152 * a[1] + 0.0722 * a[2];
    };

    const parseRGB = (color: string): [number, number, number] | null => {
      const m = color.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/i);
      if (!m) return null;
      return [parseInt(m[1], 10), parseInt(m[2], 10), parseInt(m[3], 10)];
    };

    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        // section in view; sample its background
        const el = entry.target as HTMLElement;
        const styles = getComputedStyle(el);
        const bg = styles.backgroundColor || styles.background || 'rgb(255,255,255)';
        const rgb = parseRGB(bg);
        if (rgb) {
          const lum = toLuminance(rgb[0], rgb[1], rgb[2]);
          // threshold ~ 0.5 for light bg
          setHeaderTheme(lum > 0.5 ? 'light' : 'dark');
        } else {
          setHeaderTheme('dark');
        }
        break;
      }
    }, { root: null, rootMargin: '-40% 0px -55% 0px', threshold: [0.25, 0.5, 0.75] });

    document.querySelectorAll('section').forEach((s) => observer.observe(s));
    return () => observer.disconnect();
  }, []);

  const handleAuthSuccess = (token: string, userData: any) => {
    // Redirect to package selection after successful login/signup
    window.location.href = '/packages';
  };

  const handleSwitchAuth = (type: 'login' | 'signup') => {
    setShowAuth(type);
    window.history.pushState({}, '', `/${type}`);
  };

  const renderPage = () => {
    // Show loading spinner while checking authentication
    if (isLoading) {
      return (
        <div style={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          height: '100vh',
          background: '#0A0A0A',
          color: '#ffffff'
        }}>
          <div>Loading...</div>
        </div>
      );
    }

    // Show authentication pages
    if (showAuth === 'login') {
      return (
        <LoginPage 
          onLoginSuccess={handleAuthSuccess}
          onSwitchToSignup={() => handleSwitchAuth('signup')}
        />
      );
    }

    if (showAuth === 'signup') {
      return (
        <SignupPage 
          onSignupSuccess={handleAuthSuccess}
          onSwitchToLogin={() => handleSwitchAuth('login')}
        />
      );
    }

    // Show main pages
    switch (currentPage) {
      case 'premium':
        return <PremiumPage />;
      case 'packages':
        return <PackageSelection onClose={() => window.location.href = '/'} />;
      case 'dashboard':
        return <UserDashboard />;
      default:
        return (
          <>
            <Header theme={headerTheme} />
            <HeroSection />
            <FreeFeatures3D />
            <HeroToSecondTransition />
            <QuartileAnalysis3D />
            <PremiumFooter3D />
          </>
        );
    }
  };

  return (
    <div className="App">
      {renderPage()}
      {premiumModal.type && (
        <PremiumFeatureModal
          featureType={premiumModal.type}
          onClose={() => setPremiumModal({ type: null })}
        />
      )}
    </div>
  );
};

// Wrap App with AuthProvider
const AppWithAuth: React.FC = () => {
  return (
    <AuthProvider>
      <App />
    </AuthProvider>
  );
};

export default AppWithAuth;