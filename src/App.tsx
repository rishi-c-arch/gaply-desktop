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
import PremiumLanding from './pages/PremiumLanding';
import PremiumFeaturePage from './pages/PremiumFeaturePage';
import UserDashboard from './pages/UserDashboard';

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
  return (
    <header className={`header ${theme}`} style={{
      position: 'fixed',
      top: 0,
      left: 0,
      width: '100%',
      zIndex: 1000,
      background: 'transparent',
      backdropFilter: 'none',
      WebkitBackdropFilter: 'none',
      borderBottom: 'none',
      transition: 'all 0.3s ease-in-out',
      padding: '10px 20px',
      height: 'auto',
    }}>
      <div className="header-content" style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        maxWidth: 1200,
        margin: '0 auto',
      }}>
        <div className="logo" style={{
          fontSize: '1.5rem',
          fontWeight: 700,
          letterSpacing: '0.05em',
          color: 'white',
          mixBlendMode: 'difference',
          transition: 'color 0.3s ease-in-out',
        }}>
          <h1>Gaply</h1>
        </div>
        <nav className="nav-menu" style={{
          display: 'flex',
          gap: '25px',
          alignItems: 'center',
        }}>
          <a href="#features" style={{
            color: 'white',
            textDecoration: 'none',
            fontSize: '0.9rem',
            fontWeight: 500,
            transition: 'all 0.3s ease-in-out',
            mixBlendMode: 'difference',
            position: 'relative',
            padding: '5px 0',
          }}
          onMouseEnter={(e) => e.currentTarget.style.transform = 'translateY(-2px)'}
          onMouseLeave={(e) => e.currentTarget.style.transform = 'translateY(0)'}
          >Features</a>
          <a href="#premium" style={{
            color: 'white',
            textDecoration: 'none',
            fontSize: '0.9rem',
            fontWeight: 500,
            transition: 'all 0.3s ease-in-out',
            mixBlendMode: 'difference',
            position: 'relative',
            padding: '5px 0',
          }}
          onMouseEnter={(e) => e.currentTarget.style.transform = 'translateY(-2px)'}
          onMouseLeave={(e) => e.currentTarget.style.transform = 'translateY(0)'}
          >Premium</a>
          <a href="#contact" style={{
            color: 'white',
            textDecoration: 'none',
            fontSize: '0.9rem',
            fontWeight: 500,
            transition: 'all 0.3s ease-in-out',
            mixBlendMode: 'difference',
            position: 'relative',
            padding: '5px 0',
          }}
          onMouseEnter={(e) => e.currentTarget.style.transform = 'translateY(-2px)'}
          onMouseLeave={(e) => e.currentTarget.style.transform = 'translateY(0)'}
          >Contact</a>
          <a href="#dashboard" className="premium-btn" style={{
            background: 'transparent',
            border: '1px solid',
            borderColor: 'white',
            color: 'white',
            padding: '8px 18px',
            borderRadius: '20px',
            textDecoration: 'none',
            fontSize: '0.9rem',
            fontWeight: 600,
            transition: 'all 0.3s ease-in-out',
            mixBlendMode: 'difference',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)';
            e.currentTarget.style.transform = 'translateY(-2px)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.backgroundColor = 'transparent';
            e.currentTarget.style.transform = 'translateY(0)';
          }}
          >Dashboard</a>
        </nav>
      </div>
    </header>
  );
};

const HeroSection: React.FC = () => {
  const heroRef = useRef<HTMLElement>(null);
  const sphereRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (sphereRef.current) {
      gsap.to(sphereRef.current, {
        rotation: 360,
        duration: 20,
        ease: "none",
        repeat: -1
      });
    }
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
          <h3>95%</h3>
          <p>ACCURACY</p>
        </div>
      </div>
    </section>
  );
};

// Main App component with routing
const App: React.FC = () => {
  const [currentPage, setCurrentPage] = useState('home');
  const [headerTheme, setHeaderTheme] = useState<'light'|'dark'>('dark');

  // Handle routing based on URL hash
  useEffect(() => {
    const handleHashChange = () => {
      const hash = window.location.hash.slice(1);
      setCurrentPage(hash || 'home');
    };

    handleHashChange();
    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, []);

  // Header theme detection
  useEffect(() => {
    const toLuminance = (r: number, g: number, b: number) => {
      const a = [r, g, b].map(v => {
        v /= 255;
        return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
      });
      return 0.2126 * a[0] + 0.7152 * a[1] + 0.0722 * a[2];
    };

    const parseRGB = (color: string): [number, number, number] | null => {
      const match = color.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      return match ? [parseInt(match[1]), parseInt(match[2]), parseInt(match[3])] : null;
    };

    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          const bgColor = window.getComputedStyle(entry.target).backgroundColor;
          const rgb = parseRGB(bgColor);
          if (rgb) {
            const luminance = toLuminance(rgb[0], rgb[1], rgb[2]);
            setHeaderTheme(luminance > 0.5 ? 'light' : 'dark');
          }
        }
      });
    }, { root: null, rootMargin: '-40% 0px -55% 0px', threshold: [0.25, 0.5, 0.75] });

    document.querySelectorAll('section').forEach((s) => observer.observe(s));
    return () => observer.disconnect();
  }, []);

  const renderPage = () => {
    switch (currentPage) {
      case 'premium':
        return <PremiumLanding />;
      case 'premium/gap-finder':
        return <PremiumFeaturePage featureType="gap-finder" />;
      case 'premium/deep-analysis':
        return <PremiumFeaturePage featureType="deep-analysis" />;
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
    </div>
  );
};

export default App;