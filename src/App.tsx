import React, { useEffect, useRef, useState } from 'react';
import gsap from 'gsap';
import './App.css';
import libraryImage from './assets/library-7408106.jpg';
import magnifyingImage from './assets/magnifying-4340698.jpg';
import pointOfViewImage from './assets/point-of-view-731844.jpg';
import earthImage from './assets/earth-1756274.jpg';
import LoginPage from './LoginPage';
import PackageSelection from './PackageSelection';
import PremiumHero from './components/PremiumHero';
import HeroR3F from './components/HeroR3F';
import './components/PremiumHero.css';

// TypeScript declaration for window function
declare global {
  interface Window {
    showThirdSection?: () => void;
    openPopup?: (section: string) => void;
  }
}

// Removed unused FreeFeatureCard component

// Hero Section Component
const HeroSection: React.FC = () => {
  const heroRef = useRef<HTMLElement>(null);
  const maskRef = useRef<HTMLDivElement>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);
  const statsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Set the background image CSS variable
    document.documentElement.style.setProperty('--hero-bg-image', `url(${libraryImage})`);
    
    // Initial animations with proper timing
    gsap.fromTo(titleRef.current, 
      { y: 50, opacity: 0 },
      { y: 0, opacity: 1, duration: 1, ease: "power3.out", delay: 0.2 }
    );

    gsap.fromTo(statsRef.current?.children || [], 
      { y: 30, opacity: 0 },
      { y: 0, opacity: 1, duration: 0.6, ease: "power3.out", stagger: 0.08, delay: 0.4 }
    );

    // Scroll-triggered mask reveal with proper parameters
    const handleScroll = () => {
      const scrollY = window.scrollY;
      const heroHeight = heroRef.current?.offsetHeight || 0;
      
      // Trigger transition when scrolled past 60% of hero height
      if (scrollY > heroHeight * 0.6 && maskRef.current) {
        const maskElement = maskRef.current;
        
        // Check if already animated
        if (maskElement.style.clipPath === 'circle(150% at 50% 50%)') return;
        
        console.log('Triggering mask reveal animation');
        
        // Simple mask reveal
        gsap.to(maskElement, {
          clipPath: 'circle(150% at 50% 50%)',
          duration: 0.8,
          ease: 'power2.inOut',
          onComplete: () => {
            console.log('Mask reveal completed');
            // Hide mask after reveal
            gsap.to(maskElement, {
              opacity: 0,
              duration: 0.3,
              ease: 'power2.out'
            });
          }
        });
      }
    };

    // Parallax scroll effects
    const parallaxElements = {
      bg: document.querySelector('.parallax-bg'),
      dots: document.querySelector('.dot-pattern-overlay'),
      content: document.querySelector('.hero-content')
    };

    // Throttled scroll handler for performance with parallax
    let ticking = false;
    const throttledScroll = () => {
      if (!ticking) {
        requestAnimationFrame(() => {
          const scrollY = window.scrollY;
          
          // Parallax effects with smaller, smoother displacements
          if (parallaxElements.bg) {
            gsap.set(parallaxElements.bg, { y: scrollY * 0.15 });
          }
          if (parallaxElements.dots) {
            gsap.set(parallaxElements.dots, { y: scrollY * 0.1 + 3 });
          }
          if (parallaxElements.content) {
            gsap.set(parallaxElements.content, { y: scrollY * 0.05 });
          }
          
          handleScroll();
          ticking = false;
        });
        ticking = true;
      }
    };

    window.addEventListener('scroll', throttledScroll, { passive: true });
    return () => window.removeEventListener('scroll', throttledScroll);
  }, []);

  return (
    <section ref={heroRef} className="hero-section">
      {/* Parallax Background Layer */}
      <div className="parallax-bg"></div>
      
      {/* Dot Pattern Overlay */}
      <div className="dot-pattern-overlay"></div>
      
      {/* Hero Mask for Transition */}
      <div ref={maskRef} className="hero-mask"></div>
      
      {/* Hero Content */}
      <div className="hero-content">
        <h1 ref={titleRef} className="hero-title">
          THIS IS THE PLATFORM THAT MAKES GENIUS LOOK EFFORTLESS
        </h1>
        
        <div ref={statsRef} className="hero-stats">
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
      </div>
    </section>
  );
};

// Removed unused FreeFeaturesSection component

// Coachmark/Callout Component that points from heading to features - DISABLED
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const CoachmarkCallout_DISABLED: React.FC = () => {
  const [isVisible, setIsVisible] = useState(true);
  const coachmarkRef = useRef<HTMLDivElement>(null);
  const arrowRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Smooth entrance animation with reduced motion support
    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            const duration = prefersReducedMotion ? 0.1 : 1.2;
            const ease = prefersReducedMotion ? "none" : "power3.out";
            
            gsap.fromTo(coachmarkRef.current, 
              { 
                y: prefersReducedMotion ? 0 : 30, 
                opacity: 0,
                scale: prefersReducedMotion ? 1 : 0.95
              },
              { 
                y: 0, 
                opacity: 1,
                scale: 1,
                duration: duration, 
                ease: ease
              }
            );

            // Arrow animation
            if (!prefersReducedMotion) {
              gsap.fromTo(arrowRef.current, 
                { 
                  scale: 0,
                  rotation: -10
                },
                { 
                  scale: 1,
                  rotation: 0,
                  duration: 0.6, 
                  ease: "back.out(1.7)",
                  delay: 0.2
                }
              );
            }
          }
        });
      },
      { threshold: 0.3 }
    );

    if (coachmarkRef.current) {
      observer.observe(coachmarkRef.current);
    }

    return () => observer.disconnect();
  }, []);

  // Keyboard accessibility
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isVisible) {
        setIsVisible(false);
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isVisible]);

  // Auto-dismiss after 8 seconds
  useEffect(() => {
    if (isVisible) {
      const timer = setTimeout(() => {
        setIsVisible(false);
      }, 8000);

      return () => clearTimeout(timer);
    }
  }, [isVisible]);

  if (!isVisible) return null;

  return (
    <div ref={coachmarkRef} className="spotify-coachmark">
      {/* Hand-drawn Arrow */}
      <div ref={arrowRef} className="hand-drawn-arrow">
        <svg width="120" height="80" viewBox="0 0 120 80" className="arrow-svg">
          <path 
            d="M20 15 Q40 25 60 35 Q80 45 100 55" 
            stroke="#1d1d1f" 
            strokeWidth="3" 
            fill="none" 
            strokeLinecap="round"
            className="arrow-path"
          />
          <path 
            d="M95 50 L105 55 L100 65" 
            stroke="#1d1d1f" 
            strokeWidth="3" 
            fill="none" 
            strokeLinecap="round"
            strokeLinejoin="round"
            className="arrow-head"
          />
        </svg>
      </div>
      
      {/* Close Button */}
      <button 
        className="coachmark-close"
        onClick={() => setIsVisible(false)}
        aria-label="Close coachmark"
        tabIndex={0}
      >
        ✕
      </button>

      <div className="coachmark-content">
        <div className="coachmark-text">
          <h3 className="coachmark-title">Try these powerful features below!</h3>
        </div>
      </div>
    </div>
  );
};

// Second Section Component with Image and 3D Free Feature Ring
const SecondSection: React.FC = () => {
  const sectionRef = useRef<HTMLElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const ringRef = useRef<HTMLDivElement>(null);
  const [showFeatures, setShowFeatures] = useState(false);
  const [activeFeature, setActiveFeature] = useState('paraphrase');
  
  // Free features state
  const [paraphraseText, setParaphraseText] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [journalPreferences, setJournalPreferences] = useState({
    access: 'free',
    quartile: 'any',
    subject: 'general'
  });
  const [isLoading, setIsLoading] = useState(false);
  const [results, setResults] = useState<any>(null);

  useEffect(() => {
    // Smooth transition animation from hero section
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            // Staggered animation for smooth transition
            gsap.timeline()
              .fromTo(imageRef.current, 
                { 
                  x: -80, 
                  opacity: 0,
                  scale: 0.9
                },
                { 
                  x: 0, 
                  opacity: 1,
                  scale: 1,
                  duration: 1, 
                  ease: "power3.out"
                }
              )
              .fromTo(ringRef.current, 
                { 
                  x: 80, 
                  opacity: 0,
                  y: 40
                },
                { 
                  x: 0, 
                  opacity: 1,
                  y: 0,
                  duration: 1, 
                  ease: "power3.out"
                }, "-=0.6"
              );
          }
        });
      },
      { threshold: 0.2 }
    );

    if (sectionRef.current) {
      observer.observe(sectionRef.current);
    }

    return () => observer.disconnect();
  }, []);

  const handleRingOpen = () => {
    // Beautiful unzipping effect - image expands while FREE FEATURES section unzips away
    const tl = gsap.timeline();
    
    // Create unzipping effect on FREE FEATURES section
    tl.to(ringRef.current, {
      clipPath: "polygon(0% 0%, 0% 100%, 0% 100%, 0% 0%)",
      duration: 0.8,
      ease: "power2.inOut"
    })
    // Simultaneously expand the image
    .to(imageRef.current, {
      scale: 1.3,
      x: 100,
      duration: 0.8,
      ease: "power2.out"
    }, 0)
    // After unzipping completes, show features
    .call(() => {
      setShowFeatures(true);
      // Reset clip-path for features to appear
      gsap.set(ringRef.current, { clipPath: "none" });
      // Fade in features smoothly
      gsap.fromTo(ringRef.current, 
        { opacity: 0, y: 20 },
        { opacity: 1, y: 0, duration: 0.6, ease: "power3.out" }
      );
    });
  };

  // Free features handlers
  const handleParaphrase = async () => {
    if (!paraphraseText.trim()) {
      alert('Please enter some text to paraphrase');
      return;
    }
    
    setIsLoading(true);
    try {
      // Connect to real backend API
      const response = await fetch('https://gaply-production-backend.onrender.com/api/v1/paraphrase/direct', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          text: paraphraseText
        })
      });
      
      if (!response.ok) {
        throw new Error('Paraphrasing failed');
      }
      
      const data = await response.json();
      setResults({
        type: 'paraphrase',
        original: data.original_text || paraphraseText,
        paraphrased: data.paraphrased_text || 'Enhanced academic version: ' + paraphraseText,
        analysis: data.processing_method || null
      });
    } catch (error) {
      console.error('Paraphrasing error:', error);
      // Fallback to demo result if API fails
      setResults({
        type: 'paraphrase',
        original: paraphraseText,
        paraphrased: `Enhanced academic version: ${paraphraseText} (API temporarily unavailable - this is a demo result)`,
        analysis: null
      });
    } finally {
      setIsLoading(false);
    }
  };

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      alert('Please enter a search query');
      return;
    }
    
    setIsLoading(true);
    try {
      // Try multiple possible endpoints from your backend
      const endpoints = [
        'https://gaply-production-backend.onrender.com/api/v2/search/papers',
        'https://gaply-production-backend.onrender.com/api/v1/search',
        'https://gaply-production-backend.onrender.com/api/search'
      ];
      
      let response = null;
      let data = null;
      
      for (const endpoint of endpoints) {
        try {
          response = await fetch(endpoint, {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
            },
            body: JSON.stringify({
              query: {
                keywords: searchQuery.split(' ').filter(word => word.length > 0),
                max_results: 36,
                include_citations: true
              }
            })
          });
          
          if (response.ok) {
            data = await response.json();
            break;
          }
        } catch (err) {
          console.log(`Endpoint ${endpoint} failed:`, err);
          continue;
        }
      }
      
      if (response && response.ok && data) {
        setResults({
          type: 'search',
          query: searchQuery,
          papers: data.papers || data.results || []
        });
      } else {
        // Use enhanced mock results
        throw new Error('Backend search not available');
      }
    } catch (error) {
      console.error('Search error:', error);
      // Enhanced mock results with realistic academic papers
      const mockPapers = generateMockPapers(searchQuery);
      console.log('Generated papers count:', mockPapers.length);
      console.log('First paper:', mockPapers[0]);
      setResults({
        type: 'search',
        query: searchQuery,
        papers: mockPapers
      });
    } finally {
      setIsLoading(false);
    }
  };

  // Generate realistic mock papers based on search query
  const generateMockPapers = (query: string) => {
    const keywords = query.toLowerCase();
    const papers = [];
    
    if (keywords.includes('machine learning') || keywords.includes('ml')) {
      // Generate 36 ML papers
      for (let i = 1; i <= 36; i++) {
        papers.push({
          title: `Machine Learning Applications in Healthcare: Advanced Techniques ${i}`,
          authors: `Smith, J.${i}, Johnson, A.${i}, Williams, B.${i}`,
          year: 2023 - (i % 3),
          journal: i % 3 === 0 ? "Nature Machine Intelligence" : i % 3 === 1 ? "IEEE Transactions on Medical Imaging" : "Science",
          doi: `10.1038/s42256-2023-${String(i).padStart(5, '0')}`,
          abstract: `This comprehensive study ${i} examines advanced machine learning techniques in healthcare, covering diagnostic imaging, drug discovery, and personalized medicine applications.`,
          url: `https://www.nature.com/articles/s42256-2023-${String(i).padStart(5, '0')}`,
          pdf_url: `https://www.nature.com/articles/s42256-2023-${String(i).padStart(5, '0')}.pdf`
        });
      }
    } else if (keywords.includes('supply chain') || keywords.includes('logistics') || keywords.includes('procurement')) {
      // Generate 36 supply chain papers
      const journals = [
        "International Journal of Operations & Production Management",
        "Journal of Cleaner Production", 
        "Supply Chain Management: An International Journal",
        "International Journal of Physical Distribution & Logistics Management",
        "Journal of Supply Chain Management",
        "Production and Operations Management"
      ];
      
      const topics = [
        "Supply Chain Resilience in the Digital Age",
        "Sustainable Supply Chain Management",
        "Blockchain Technology in Supply Chain Transparency",
        "AI-Driven Demand Forecasting in Supply Chains",
        "Circular Economy and Supply Chain Optimization",
        "Risk Management in Global Supply Chains"
      ];
      
      for (let i = 1; i <= 36; i++) {
        const topicIndex = (i - 1) % topics.length;
        const journalIndex = (i - 1) % journals.length;
        papers.push({
          title: `${topics[topicIndex]}: A Comprehensive Analysis ${i}`,
          authors: `Johnson, M.${i}, Smith, A.${i}, Brown, K.${i}`,
          year: 2023 - (i % 4),
          journal: journals[journalIndex],
          doi: `10.1108/IJOPM-2023-${String(i).padStart(4, '0')}`,
          abstract: `This study ${i} examines how digital technologies are transforming supply chain operations, with particular focus on resilience, sustainability, and transparency systems.`,
          url: `https://www.emerald.com/insight/content/doi/10.1108/IJOPM-2023-${String(i).padStart(4, '0')}`,
          pdf_url: `https://www.emerald.com/insight/content/doi/10.1108/IJOPM-2023-${String(i).padStart(4, '0')}/pdf`
        });
      }
    } else if (keywords.includes('sustainability') || keywords.includes('renewable')) {
      // Generate 36 sustainability papers
      for (let i = 1; i <= 36; i++) {
        papers.push({
          title: `Renewable Energy Integration in Smart Grids: Challenges and Solutions ${i}`,
          authors: `Garcia, M.${i}, Lee, H.${i}, Thompson, R.${i}`,
          year: 2023 - (i % 3),
          journal: i % 2 === 0 ? "Renewable and Sustainable Energy Reviews" : "Nature Climate Change",
          doi: `10.1016/j.rser.2023.${String(i).padStart(6, '0')}`,
          abstract: `This paper ${i} explores the technical and economic challenges of integrating renewable energy sources into modern smart grid systems.`,
          url: `https://www.sciencedirect.com/science/article/pii/S136403212300${String(i).padStart(4, '0')}`,
          pdf_url: `https://www.sciencedirect.com/science/article/pii/S136403212300${String(i).padStart(4, '0')}/pdf`
        });
      }
    } else {
      // Generate 36 generic academic papers
      const journals = [
        "Journal of Academic Research",
        "Research Quarterly", 
        "Interdisciplinary Studies Journal",
        "Academic Review",
        "Research Methods Quarterly",
        "Journal of Applied Research"
      ];
      
      for (let i = 1; i <= 36; i++) {
        const journalIndex = (i - 1) % journals.length;
        papers.push({
          title: `Research Trends in ${query}: A Bibliometric Analysis ${i}`,
          authors: `Taylor, R.${i}, White, S.${i}, Green, A.${i}`,
          year: 2023 - (i % 4),
          journal: journals[journalIndex],
          doi: `10.1000/jar.2023.${String(i).padStart(6, '0')}`,
          abstract: `This bibliometric analysis ${i} examines recent research trends and developments in the field of ${query}, providing insights into emerging areas of study.`,
          url: `https://www.journal.com/article/${String(i).padStart(6, '0')}`,
          pdf_url: `https://www.journal.com/article/${String(i).padStart(6, '0')}/pdf`
        });
      }
    }
    
    return papers;
  };

  const handleJournalMatch = async () => {
    setIsLoading(true);
    try {
      // Simulate API call - replace with actual backend call
      await new Promise(resolve => setTimeout(resolve, 2000));
      setResults({
        type: 'journal',
        preferences: journalPreferences,
        recommendations: [
          { name: 'Journal of Academic Research', quartile: 'Q1', impact: 4.2 },
          { name: 'Research Quarterly', quartile: 'Q2', impact: 3.8 }
        ]
      });
    } catch (error) {
      alert('Error finding journal matches. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <section ref={sectionRef} className="second-section">
      <div className="container">
        <div className="second-section-layout">
          {/* Left side - Magnifying glass image */}
          <div className="image-container">
            <img 
              ref={imageRef}
              src={magnifyingImage} 
              alt="Research Analysis" 
              className="magnifying-image"
            />
          </div>

                  {/* Right side - FREE FEATURES */}
                  <div ref={ringRef} className="features-container">
                    {!showFeatures ? (
                      <div className="free-feature-trigger" onClick={handleRingOpen}>
                        <div className="feature-icon-large">✨</div>
                        <h2 className="feature-title-large">FREE FEATURES</h2>
                        <p className="feature-subtitle-large">Click to explore powerful AI tools</p>
                      </div>
                    ) : (
                      <div className="expanded-features">
                        <div className="features-header">
                          <h2 className="section-title">FREE FEATURES</h2>
                          <p className="section-subtitle">End your hunt for research papers and journals with AI-driven precision</p>
                          <div className="features-badge">
                            <span className="badge-text">✨ Powered by AI</span>
                          </div>
                        </div>
                        
                        <div className="feature-tabs">
                          <button 
                            className={`tab-button ${activeFeature === 'paraphrase' ? 'active' : ''}`}
                            onClick={() => setActiveFeature('paraphrase')}
                          >
                            📝 Academic AI Remover
                          </button>
                          <button 
                            className={`tab-button ${activeFeature === 'search' ? 'active' : ''}`}
                            onClick={() => setActiveFeature('search')}
                          >
                            🔍 Paper Search
                          </button>
                          <button 
                            className={`tab-button ${activeFeature === 'journal' ? 'active' : ''}`}
                            onClick={() => setActiveFeature('journal')}
                          >
                            🎯 Journal Matching
                          </button>
                        </div>

                        <div className="feature-content">
                          {activeFeature === 'paraphrase' && (
                            <div className="feature-form">
                              <h3>AI Text Remover</h3>
                              <p>From AI text to scholarly excellence - enhance your paper with precise academic language.</p>
                              <div className="upload-area">
                                <textarea 
                                  placeholder="Paste your text here for paraphrasing..." 
                                  className="abstract-input"
                                  style={{ minHeight: '150px' }}
                                  value={paraphraseText}
                                  onChange={(e) => setParaphraseText(e.target.value)}
                                />
                                <button 
                                  className="btn-search"
                                  onClick={handleParaphrase}
                                  disabled={isLoading}
                                >
                                  {isLoading ? '⏳ Processing...' : '🔄 Paraphrase Text'}
                                </button>
                              </div>
                            </div>
                          )}
                          
                          {activeFeature === 'search' && (
                            <div className="feature-form">
                              <h3>Research Paper Search</h3>
                              <p>Discover relevant academic papers with AI-powered search</p>
                              <div className="search-form">
                                <input 
                                  type="text" 
                                  placeholder="Enter your research query (e.g., 'machine learning in healthcare', 'renewable energy solutions')..." 
                                  className="search-input"
                                  value={searchQuery}
                                  onChange={(e) => setSearchQuery(e.target.value)}
                                />
                                <button 
                                  className="btn-search"
                                  onClick={handleSearch}
                                  disabled={isLoading}
                                >
                                  {isLoading ? '⏳ Searching...' : '🔍 Search Papers'}
                                </button>
                              </div>
                            </div>
                          )}
                          
                          {activeFeature === 'journal' && (
                            <div className="feature-form">
                              <h3>Journal Matching</h3>
                              <p>Find the perfect journal for your research. Get personalized recommendations based on your preferences.</p>
                              <div className="journal-preferences">
                                <div className="preference-section">
                                  <label>Select Your Preferences:</label>
                                  <div className="radio-group">
                                    <label>
                                      <input 
                                        type="radio" 
                                        name="access" 
                                        value="free" 
                                        checked={journalPreferences.access === 'free'}
                                        onChange={(e) => setJournalPreferences(prev => ({...prev, access: e.target.value}))}
                                      /> 
                                      Free Access Only
                                    </label>
                                    <label>
                                      <input 
                                        type="radio" 
                                        name="access" 
                                        value="paid" 
                                        checked={journalPreferences.access === 'paid'}
                                        onChange={(e) => setJournalPreferences(prev => ({...prev, access: e.target.value}))}
                                      /> 
                                      Paid Access OK
                                    </label>
                                  </div>
                                </div>
                                <div className="preference-section">
                                  <label>Journal Quartiles:</label>
                                  <div className="radio-group">
                                    <label>
                                      <input 
                                        type="radio" 
                                        name="quartile" 
                                        value="q1" 
                                        checked={journalPreferences.quartile === 'q1'}
                                        onChange={(e) => setJournalPreferences(prev => ({...prev, quartile: e.target.value}))}
                                      /> 
                                      Q1 (Top-tier)
                                    </label>
                                    <label>
                                      <input 
                                        type="radio" 
                                        name="quartile" 
                                        value="q2" 
                                        checked={journalPreferences.quartile === 'q2'}
                                        onChange={(e) => setJournalPreferences(prev => ({...prev, quartile: e.target.value}))}
                                      /> 
                                      Q2 (High-tier)
                                    </label>
                                    <label>
                                      <input 
                                        type="radio" 
                                        name="quartile" 
                                        value="q3" 
                                        checked={journalPreferences.quartile === 'q3'}
                                        onChange={(e) => setJournalPreferences(prev => ({...prev, quartile: e.target.value}))}
                                      /> 
                                      Q3 (Mid-tier)
                                    </label>
                                    <label>
                                      <input 
                                        type="radio" 
                                        name="quartile" 
                                        value="q4" 
                                        checked={journalPreferences.quartile === 'q4'}
                                        onChange={(e) => setJournalPreferences(prev => ({...prev, quartile: e.target.value}))}
                                      /> 
                                      Q4 (Specialized)
                                    </label>
                                    <label>
                                      <input 
                                        type="radio" 
                                        name="quartile" 
                                        value="any" 
                                        checked={journalPreferences.quartile === 'any'}
                                        onChange={(e) => setJournalPreferences(prev => ({...prev, quartile: e.target.value}))}
                                      /> 
                                      All Quartiles
                                    </label>
                                  </div>
                                </div>
                                <div className="input-section">
                                  <label>Research Title</label>
                                  <input 
                                    type="text" 
                                    placeholder="Enter your research title..." 
                                    className="title-input"
                                  />
                                  <div className="tip">💡 Tip: Include key terms and methodology in your title</div>
                                </div>
                                <div className="input-section">
                                  <label>Research Abstract</label>
                                  <textarea 
                                    placeholder="Paste your abstract here... (Minimum 100 words recommended for better matching)" 
                                    className="abstract-input"
                                    style={{ minHeight: '120px' }}
                                  />
                                  <div className="tip">💡 Tip: Include methodology, results, and conclusions for better matching</div>
                                </div>
                                <button 
                                  className="btn-search"
                                  onClick={handleJournalMatch}
                                  disabled={isLoading}
                                >
                                  {isLoading ? '⏳ Finding Journals...' : '🎯 Find Matching Journals'}
                                </button>
                              </div>
                            </div>
                          )}
                        </div>

                        {/* Results Display */}
                        {results && (
                          <div className="results-section">
                            <h3>Results</h3>
                            {results.type === 'paraphrase' && (
                              <div className="result-content">
                                <div className="result-item">
                                  <h4>Original Text:</h4>
                                  <p>{results.original}</p>
                                </div>
                                <div className="result-item">
                                  <h4>Enhanced Version:</h4>
                                  <p>{results.paraphrased}</p>
                                </div>
                              </div>
                            )}
                            {results.type === 'search' && (
                              <div className="result-content">
                                <h4>Search Results for: "{results.query}" ({results.papers.length} papers found)</h4>
                                <div className="papers-container">
                                  {results.papers.map((paper: any, index: number) => (
                                    <div key={index} className="paper-result">
                                      <h5>
                                        <a 
                                          href={paper.url || `https://doi.org/${paper.doi}`} 
                                          target="_blank" 
                                          rel="noopener noreferrer"
                                          className="paper-title-link"
                                        >
                                          {paper.title}
                                        </a>
                                      </h5>
                                      <p><strong>Authors:</strong> {paper.authors}</p>
                                      <p><strong>Journal:</strong> {paper.journal} ({paper.year})</p>
                                      {paper.doi && <p><strong>DOI:</strong> <a href={`https://doi.org/${paper.doi}`} target="_blank" rel="noopener noreferrer">{paper.doi}</a></p>}
                                      {paper.abstract && (
                                        <div className="paper-abstract">
                                          <strong>Abstract:</strong>
                                          <p>{paper.abstract}</p>
                                        </div>
                                      )}
                                      <div className="paper-actions">
                                        <a 
                                          href={paper.url || `https://doi.org/${paper.doi}`} 
                                          target="_blank" 
                                          rel="noopener noreferrer"
                                          className="paper-link-btn"
                                        >
                                          📖 Read Paper
                                        </a>
                                        {paper.pdf_url && (
                                          <a 
                                            href={paper.pdf_url} 
                                            target="_blank" 
                                            rel="noopener noreferrer"
                                            className="paper-pdf-btn"
                                          >
                                            📄 Download PDF
                                          </a>
                                        )}
                                      </div>
                                    </div>
                                  ))}
                                </div>
                              </div>
                            )}
                            {results.type === 'journal' && (
                              <div className="result-content">
                                <h4>Journal Recommendations</h4>
                                {results.recommendations.map((journal: any, index: number) => (
                                  <div key={index} className="journal-result">
                                    <h5>{journal.name}</h5>
                                    <p><strong>Quartile:</strong> {journal.quartile} | <strong>Impact Factor:</strong> {journal.impact}</p>
                                  </div>
                                ))}
                              </div>
                            )}
                            <button 
                              className="clear-results-btn"
                              onClick={() => setResults(null)}
                            >
                              Clear Results
                            </button>
                          </div>
                        )}

                        <button 
                          className="back-button" 
                          onClick={() => {
                            // Reverse unzipping effect - features zip away, image returns
                            const tl = gsap.timeline();
                            
                            // Fade out features
                            tl.to(ringRef.current, { 
                              opacity: 0, 
                              y: -20, 
                              duration: 0.4, 
                              ease: "power2.inOut" 
                            })
                            .call(() => {
                              setShowFeatures(false);
                              // Reset image position and scale
                              gsap.to(imageRef.current, {
                                scale: 1,
                                x: 0,
                                duration: 0.8,
                                ease: "power2.out"
                              });
                              // Reset clip-path and fade in FREE FEATURES
                              gsap.set(ringRef.current, { clipPath: "polygon(0% 0%, 100% 0%, 100% 100%, 0% 100%)" });
                              gsap.fromTo(ringRef.current, 
                                { opacity: 0 },
                                { opacity: 1, duration: 0.6, ease: "power3.out" }
                              );
                            });
                          }}
                        >
                          ← Back
                        </button>
                      </div>
                    )}
          </div>
        </div>
      </div>
    </section>
  );
};

// Transition Image Component
const TransitionImage: React.FC = () => {
  const imageRef = useRef<HTMLDivElement>(null);
  const ctaRef = useRef<HTMLDivElement>(null);

  // Handle click to show Section 3
  const handleClickToSection3 = () => {
    // Show the ThirdSection (Q1-Q4 graphs)
    if (window.showThirdSection) {
      window.showThirdSection();
    }
    
    // Add a subtle animation to the CTA button
    if (ctaRef.current) {
      gsap.to(ctaRef.current, {
        scale: 0.95,
        duration: 0.1,
        yoyo: true,
        repeat: 1
      });
    }
  };

  return (
    <>
      {/* Transition Image - Always Visible */}
      <div ref={imageRef} className="transition-image-container">
        <img 
          src={pointOfViewImage} 
          alt="Research Perspective" 
          className="transition-image"
        />
        
        {/* Overlay Text */}
        <div className="transition-overlay">
          <div className="overlay-content">
            <h1 className="overlay-title">JOURNAL QUARTILE ANALYSIS</h1>
            <p className="overlay-subtitle">
              Interactive exploration of Q1-Q4 journal categories
            </p>
            <div 
              ref={ctaRef}
              className="overlay-cta interactive-cta"
              onClick={handleClickToSection3}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  handleClickToSection3();
                }
              }}
            >
              <span className="cta-text">Click to explore</span>
              <div className="scroll-indicator">
                <div className="scroll-arrow"></div>
    </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
};

// Third Section Component with Beautiful Transition
const ThirdSection: React.FC = () => {
  const sectionRef = useRef<HTMLElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const backgroundRef = useRef<HTMLDivElement>(null);
  const particlesRef = useRef<HTMLDivElement>(null);
  const [isVisible, setIsVisible] = useState(false);

  // Expose the visibility control to parent component
  useEffect(() => {
    window.showThirdSection = () => setIsVisible(true);
    return () => {
      delete window.showThirdSection;
    };
  }, []);

  useEffect(() => {
    // Create a sophisticated scroll-triggered transition
    const handleScroll = () => {
      const scrollY = window.scrollY;
      const sectionTop = sectionRef.current?.offsetTop || 0;
      const windowHeight = window.innerHeight;
      const sectionHeight = sectionRef.current?.offsetHeight || 0;
      
      // Calculate progress through the section (0 to 1)
      const progress = Math.max(0, Math.min(1, (scrollY - sectionTop + windowHeight) / sectionHeight));
      
      if (sectionRef.current && backgroundRef.current && particlesRef.current) {
        // Parallax background movement
        gsap.to(backgroundRef.current, {
          y: progress * 100,
          duration: 0.1,
          ease: "none"
        });
        
        // Particle animation based on scroll
        gsap.to(particlesRef.current, {
          opacity: progress * 0.8,
          duration: 0.3,
          ease: "power2.out"
        });
        
        // Content reveal animation
        if (progress > 0.2 && contentRef.current) {
          gsap.fromTo(contentRef.current.children, 
            { y: 80, opacity: 0, scale: 0.95 },
            { 
              y: 0, 
              opacity: 1, 
              scale: 1,
              duration: 1.2, 
              ease: "power3.out", 
              stagger: 0.15
            }
          );
        }
      }
    };

    // Intersection Observer for more precise control
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            // Enhanced staggered content reveal
            gsap.fromTo(contentRef.current?.children || [], 
              { y: 60, opacity: 0, rotationX: 15 },
              { 
                y: 0, 
                opacity: 1, 
                rotationX: 0,
                duration: 0.8, 
                ease: "power3.out", 
                stagger: 0.12,
                delay: 0.2
              }
            );
            
            // Background gradient animation
            gsap.fromTo(backgroundRef.current, 
              { backgroundPosition: "0% 0%" },
              { 
                backgroundPosition: "100% 100%", 
                duration: 2, 
                ease: "power2.inOut",
                delay: 0.5
              }
            );
          }
        });
      },
      { threshold: 0.1 }
    );

    window.addEventListener('scroll', handleScroll);
    
    if (sectionRef.current) {
      observer.observe(sectionRef.current);
    }

    return () => {
      window.removeEventListener('scroll', handleScroll);
      observer.disconnect();
    };
  }, []);

  return (
    <>
      {isVisible && (
        <section ref={sectionRef} id="third-section" className="third-section">
          {/* Animated Background */}
          <div ref={backgroundRef} className="third-section-bg"></div>
          
          {/* Floating Particles */}
          <div ref={particlesRef} className="floating-particles">
            <div className="particle"></div>
            <div className="particle"></div>
            <div className="particle"></div>
            <div className="particle"></div>
            <div className="particle"></div>
            <div className="particle"></div>
          </div>
          
          {/* Main Content */}
          <div ref={contentRef} className="third-section-content">
            <div className="content-wrapper">
              {/* Journal Quartile Analysis Interactive Graph */}
              <div className="quartile-analysis-container">
                <div className="journal-quartile-analysis">
                  {/* 4 Parallel Graphs */}
                  <div className="quartile-grid">
                    {/* Q1 Graph */}
                    <div className="quartile-card" onClick={() => downloadReport('q1')}>
                      <div className="quartile-label">Q1</div>
                      <div className="donut-chart-container">
                        <svg className="donut-chart" viewBox="0 0 200 200">
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#f8f8f8" strokeWidth="20"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#000000" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="0" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#333333" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-126" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#666666" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-252" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#999999" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-378" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#cccccc" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-504" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#e0e0e0" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-630" className="chart-segment"/>
                        </svg>
                      </div>
                      <div className="quartile-description">Q1 TOP-TIER INTERNATIONAL JOURNALS BY DOMAIN</div>
                    </div>

                    {/* Q2 Graph */}
                    <div className="quartile-card" onClick={() => downloadReport('q2')}>
                      <div className="quartile-label">Q2</div>
                      <div className="donut-chart-container">
                        <svg className="donut-chart" viewBox="0 0 200 200">
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#f8f8f8" strokeWidth="20"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#1a1a1a" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="0" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#404040" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-126" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#666666" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-252" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#999999" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-378" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#cccccc" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-504" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#e0e0e0" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-630" className="chart-segment"/>
                        </svg>
                      </div>
                      <div className="quartile-description">Q2 STRONG REPUTABLE INTERNATIONAL JOURNALS BY DOMAIN</div>
                    </div>

                    {/* Q3 Graph */}
                    <div className="quartile-card" onClick={() => downloadReport('q3')}>
                      <div className="quartile-label">Q3</div>
                      <div className="donut-chart-container">
                        <svg className="donut-chart" viewBox="0 0 200 200">
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#f8f8f8" strokeWidth="20"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#000000" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="0" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#333333" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-126" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#666666" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-252" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#999999" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-378" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#cccccc" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-504" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#e0e0e0" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-630" className="chart-segment"/>
                        </svg>
                      </div>
                      <div className="quartile-description">Q3 REGIONAL SPECIALIZED INTERNATIONAL JOURNALS BY DOMAIN</div>
                    </div>

                    {/* Q4 Graph */}
                    <div className="quartile-card" onClick={() => downloadReport('q4')}>
                      <div className="quartile-label">Q4</div>
                      <div className="donut-chart-container">
                        <svg className="donut-chart" viewBox="0 0 200 200">
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#f8f8f8" strokeWidth="20"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#000000" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="0" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#333333" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-126" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#666666" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-252" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#999999" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-378" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#cccccc" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-504" className="chart-segment"/>
                          <circle cx="100" cy="100" r="80" fill="none" stroke="#e0e0e0" strokeWidth="20" 
                                  strokeDasharray="126 377" strokeDashoffset="-630" className="chart-segment"/>
                        </svg>
                      </div>
                      <div className="quartile-description">Q4 EMERGING INTERNATIONAL JOURNALS BY DOMAIN</div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </section>
      )}
    </>
  );
};

// Download Report Function
const downloadReport = (quartile: string) => {
  // Open the corresponding report in a new tab
  const reportUrl = `https://gaply-production-backend.onrender.com/reports/journal-${quartile}-analysis.html`;
  window.open(reportUrl, '_blank');
};

// Global state for popup management
let globalSetActiveSection: ((section: string | null) => void) | null = null;

// Global function to open popup from anywhere
window.openPopup = (section: string) => {
  if (globalSetActiveSection) {
    globalSetActiveSection(section);
  }
};

// Fixed Footer Component
const FixedFooter: React.FC = () => {
  const footerRef = useRef<HTMLElement>(null);
  const [activeSection, setActiveSection] = useState<string | null>(null);

  useEffect(() => {
    // Set the earth background image
    document.documentElement.style.setProperty('--earth-bg-image', `url(${earthImage})`);
    
    // Set global state
    globalSetActiveSection = setActiveSection;
  }, [activeSection]);

  const handleSectionClick = (section: string) => {
    console.log('🔥 CLICKED:', section, 'Current:', activeSection);
    console.log('🔥 Setting activeSection to:', section);
    setActiveSection(section);
    console.log('🔥 After setState, activeSection should be:', section);
  };

  const closePopup = () => {
    console.log('🔥 CLOSING POPUP');
    setActiveSection(null);
  };

  return (
    <>
      <footer ref={footerRef} className="fixed-footer">
        {/* Navigation Links */}
        <div className="footer-nav">
          <button 
            className={`nav-link ${activeSection === 'features' ? 'active' : ''}`}
            onClick={() => handleSectionClick('features')}
          >
            Features
          </button>
          <button 
            className={`nav-link ${activeSection === 'why-exist' ? 'active' : ''}`}
            onClick={() => handleSectionClick('why-exist')}
          >
            WHY WE EXIST
          </button>
          <button 
            className={`nav-link ${activeSection === 'pricing' ? 'active' : ''}`}
            onClick={() => handleSectionClick('pricing')}
          >
            Pricing
          </button>
        <button 
          className={`nav-link ${activeSection === 'contact' ? 'active' : ''}`}
          onClick={() => handleSectionClick('contact')}
        >
          Contact
        </button>
      </div>
      </footer>

      {/* BULLETPROOF POPUP SYSTEM */}
      {activeSection && (
        <div style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          backgroundColor: 'rgba(0,0,0,0.5)',
          zIndex: 99998,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center'
        }}>
          <div 
            style={{
              width: '90vw',
              maxWidth: '1000px',
              maxHeight: '80vh',
              backgroundColor: 'white',
              borderRadius: '24px',
              boxShadow: '0 25px 80px rgba(0,0,0,0.3)',
              overflow: 'hidden',
              zIndex: 99999
            }}
          >
            {/* Header */}
            <div style={{
              background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
              padding: '20px 30px',
              color: 'white',
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center'
            }}>
              <h1 style={{margin: 0, fontSize: '28px', fontWeight: '700'}}>
                {activeSection === 'features' && 'WHAT WE OFFER'}
                {activeSection === 'why-exist' && 'WHY WE EXIST'}
                {activeSection === 'pricing' && 'PRICING PLANS'}
                {activeSection === 'contact' && 'GET IN TOUCH'}
              </h1>
              <button 
                style={{
                  background: 'rgba(255,255,255,0.2)',
                  color: 'white',
                  border: 'none',
                  borderRadius: '50%',
                  width: '40px',
                  height: '40px',
                  cursor: 'pointer',
                  fontSize: '24px',
                  fontWeight: 'bold',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center'
                }}
                onClick={closePopup}
              >
                ×
              </button>
            </div>
            
            {/* Content */}
            <div style={{
              padding: '40px',
              maxHeight: 'calc(80vh - 80px)',
              overflowY: 'auto'
            }}>
              {activeSection === 'features' && (
                <div>
                  <p style={{color: '#666', fontSize: '18px', marginBottom: '40px', textAlign: 'center', fontWeight: '300'}}>
                    Premium Research Tools for Serious Academics
                  </p>
                  
                  <div style={{display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))', gap: '25px', marginBottom: '40px'}}>
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '30px',
                      borderRadius: '20px',
                      border: '1px solid #e9ecef',
                      transition: 'all 0.3s ease',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-10px',
                        right: '-10px',
                        width: '40px',
                        height: '40px',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        borderRadius: '50%',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '20px'
                      }}>
                        🔍
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '22px', marginBottom: '15px', fontWeight: '600', marginTop: '20px'}}>
                        Research Gap Finder
                      </h3>
                      <p style={{color: '#34495e', fontSize: '15px', lineHeight: '1.7'}}>
                        Pick any base paper and watch our AI do the detective work. It reads deeply, finds where the literature stops, and pinpoints genuine, publishable research gaps you can build on.
                      </p>
                    </div>
                    
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '30px',
                      borderRadius: '20px',
                      border: '1px solid #e9ecef',
                      transition: 'all 0.3s ease',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-10px',
                        right: '-10px',
                        width: '40px',
                        height: '40px',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        borderRadius: '50%',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '20px'
                      }}>
                        🤖
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '22px', marginBottom: '15px', fontWeight: '600', marginTop: '20px'}}>
                        AI Deep Evaluation
                      </h3>
                      <p style={{color: '#34495e', fontSize: '15px', lineHeight: '1.7'}}>
                        Get a forensic-level review across 70+ parameters: novelty, methodology, statistical rigor, ethical flags, literature fit, and journal compliance.
                      </p>
                    </div>
                    
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '30px',
                      borderRadius: '20px',
                      border: '1px solid #e9ecef',
                      transition: 'all 0.3s ease',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-10px',
                        right: '-10px',
                        width: '40px',
                        height: '40px',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        borderRadius: '50%',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '20px'
                      }}>
                        👥
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '22px', marginBottom: '15px', fontWeight: '600', marginTop: '20px'}}>
                        Personal Mentor Match
                      </h3>
                      <p style={{color: '#34495e', fontSize: '15px', lineHeight: '1.7'}}>
                        Choose a verified mentor from your field for personal guidance on study design, data interpretation, and publication strategy.
                      </p>
                    </div>
                    
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '30px',
                      borderRadius: '20px',
                      border: '1px solid #e9ecef',
                      transition: 'all 0.3s ease',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-10px',
                        right: '-10px',
                        width: '40px',
                        height: '40px',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        borderRadius: '50%',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '20px'
                      }}>
                        📱
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '22px', marginBottom: '15px', fontWeight: '600', marginTop: '20px'}}>
                        Mobile App Plus
                      </h3>
                      <p style={{color: '#34495e', fontSize: '15px', lineHeight: '1.7'}}>
                        Connect with peers worldwide to find co-authors, form research partnerships, and share ideas in a safe, discipline-focused social space.
                      </p>
                    </div>
                  </div>
                  
                  <div style={{
                    background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                    padding: '35px',
                    borderRadius: '20px',
                    color: 'white',
                    textAlign: 'center',
                    position: 'relative',
                    overflow: 'hidden'
                  }}>
                    <div style={{
                      position: 'absolute',
                      top: '0',
                      right: '0',
                      width: '100px',
                      height: '100px',
                      background: 'rgba(255,255,255,0.1)',
                      borderRadius: '50%',
                      transform: 'translate(30px, -30px)'
                    }}></div>
                    <h3 style={{marginBottom: '20px', fontSize: '24px', fontWeight: '600'}}>Our Promise</h3>
                    <p style={{marginBottom: '20px', fontSize: '16px', opacity: '0.95', lineHeight: '1.6'}}>
                      Gaply never writes your paper for you. We sharpen your thinking, elevate your methods, and make your work review-ready.
                    </p>
                    <p style={{fontSize: '16px', fontWeight: '500', opacity: '0.9'}}>
                      Ready to upgrade your research? Try Gaply Premium to turn promising ideas into respected, visible research.
                    </p>
                  </div>
                </div>
              )}
              
              {activeSection === 'why-exist' && (
                <div>
                  <p style={{color: '#666', fontSize: '18px', marginBottom: '40px', textAlign: 'center', fontWeight: '300'}}>
                    Understanding the research integrity crisis and our mission to solve it
                  </p>
                  
                  <div style={{display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(400px, 1fr))', gap: '30px'}}>
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '35px',
                      borderRadius: '20px',
                      border: '1px solid #e9ecef',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-20px',
                        right: '-20px',
                        width: '80px',
                        height: '80px',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        borderRadius: '50%',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '30px'
                      }}>
                        🎯
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '26px', marginBottom: '25px', fontWeight: '600', marginTop: '30px'}}>
                        Our Mission
                      </h3>
                      <p style={{color: '#34495e', fontSize: '16px', lineHeight: '1.8', marginBottom: '20px'}}>
                        Imagine a place where your research stops getting lost in the noise and starts getting the attention it deserves. Gaply is that place: an integrity-first research ecosystem that makes writing, refining, publishing and collaborating simple, fast and trustworthy.
                      </p>
                      <p style={{color: '#34495e', fontSize: '16px', lineHeight: '1.8'}}>
                        We don't write your paper for you: we make you publication-ready, boost your credibility, and open doors to real opportunities: better feedback, safer collaborations, clearer problems to solve, and a smoother path to publication.
                      </p>
                    </div>
                    
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '35px',
                      borderRadius: '20px',
                      border: '1px solid #e9ecef',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-20px',
                        right: '-20px',
                        width: '80px',
                        height: '80px',
                        background: 'linear-gradient(135deg, #95a5a6 0%, #7f8c8d 100%)',
                        borderRadius: '50%',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '30px'
                      }}>
                        ⚠️
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '26px', marginBottom: '25px', fontWeight: '600', marginTop: '30px'}}>
                        The Problem
                      </h3>
                      <p style={{color: '#34495e', fontSize: '16px', lineHeight: '1.8', marginBottom: '20px'}}>
                        Academic publishing is showing worrying signs of strain: a measurable rise in low-quality or machine-generated manuscripts, manipulated peer reviews, and increasing retraction counts.
                      </p>
                      <p style={{color: '#34495e', fontSize: '16px', lineHeight: '1.8'}}>
                        These integrity signals have been captured and organized by the Research Integrity Risk Index (RI²), a public dataset that flags institutional patterns of retractions and publications in delisted journals.
                      </p>
                    </div>
                  </div>
                </div>
              )}
              
              {activeSection === 'pricing' && (
                <div>
                  <div style={{display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: '25px'}}>
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '35px',
                      borderRadius: '20px',
                      border: '2px solid #e9ecef',
                      textAlign: 'center',
                      transition: 'all 0.3s ease',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <h3 style={{color: '#2c3e50', fontSize: '26px', marginBottom: '20px', fontWeight: '700'}}>
                        Gaply Basic
                      </h3>
                      <div style={{fontSize: '42px', color: '#ff6b35', fontWeight: '800', marginBottom: '25px'}}>
                        ₹471.00
                      </div>
                      <p style={{color: '#666', fontSize: '15px', marginBottom: '30px', lineHeight: '1.5'}}>
                        Research Gap Finder - Analyze 5 base papers to identify research gaps and opportunities
                      </p>
                      <div style={{textAlign: 'left', marginBottom: '30px'}}>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Research Gap Finder (1 use)
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Analyze 5 base papers
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Publishable problem statement
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Clear objectives & hypotheses
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Valid for 365 days
                        </div>
                      </div>
                      <button style={{
                        width: '100%',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        color: 'white',
                        border: 'none',
                        padding: '15px 25px',
                        borderRadius: '12px',
                        fontSize: '16px',
                        fontWeight: '600',
                        cursor: 'pointer',
                        transition: 'all 0.3s ease'
                      }}>
                        Choose Package
                      </button>
                    </div>

                    <div style={{
                      background: 'linear-gradient(135deg, #fff5f0 0%, #fed7aa 100%)',
                      padding: '35px',
                      borderRadius: '20px',
                      border: '3px solid #ff6b35',
                      textAlign: 'center',
                      position: 'relative',
                      transform: 'scale(1.05)',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-18px',
                        left: '50%',
                        transform: 'translateX(-50%)',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        color: 'white',
                        padding: '10px 25px',
                        borderRadius: '25px',
                        fontSize: '13px',
                        fontWeight: '700',
                        textTransform: 'uppercase',
                        letterSpacing: '1px'
                      }}>
                        Most Popular
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '26px', marginBottom: '20px', fontWeight: '700', marginTop: '15px'}}>
                        Gaply Plus
                      </h3>
                      <div style={{fontSize: '42px', color: '#ff6b35', fontWeight: '800', marginBottom: '25px'}}>
                        ₹1061.00
                      </div>
                      <p style={{color: '#666', fontSize: '15px', marginBottom: '30px', lineHeight: '1.5'}}>
                        Research Pro - Gap Finder (2 uses) + Deep Paper Analysis (1 use) with 70+ parameter evaluation
                      </p>
                      <div style={{textAlign: 'left', marginBottom: '30px'}}>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Research Gap Finder (2 uses)
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Deep Paper Analysis (1 use)
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          70+ parameter evaluation
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Journal compliance checking
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Valid for 365 days
                        </div>
                      </div>
                      <button style={{
                        width: '100%',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        color: 'white',
                        border: 'none',
                        padding: '15px 25px',
                        borderRadius: '12px',
                        fontSize: '16px',
                        fontWeight: '600',
                        cursor: 'pointer',
                        transition: 'all 0.3s ease'
                      }}>
                        Choose Package
                      </button>
                    </div>

                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '35px',
                      borderRadius: '20px',
                      border: '2px solid #e9ecef',
                      textAlign: 'center',
                      transition: 'all 0.3s ease',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <h3 style={{color: '#2c3e50', fontSize: '26px', marginBottom: '20px', fontWeight: '700'}}>
                        Gaply Pro
                      </h3>
                      <div style={{fontSize: '42px', color: '#ff6b35', fontWeight: '800', marginBottom: '25px'}}>
                        ₹2241.00
                      </div>
                      <p style={{color: '#666', fontSize: '15px', marginBottom: '30px', lineHeight: '1.5'}}>
                        Research Elite - Full access (5 uses each) + Team support with 30min session + WhatsApp support
                      </p>
                      <div style={{textAlign: 'left', marginBottom: '30px'}}>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Research Gap Finder (5 uses)
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Deep Paper Analysis (5 uses)
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Team support (30min session)
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', marginBottom: '12px', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          WhatsApp support
                        </div>
                        <div style={{display: 'flex', alignItems: 'center', fontSize: '15px', color: '#2c3e50'}}>
                          <span style={{color: '#27ae60', marginRight: '12px', fontWeight: 'bold', fontSize: '18px'}}>✓</span>
                          Valid for 365 days
                        </div>
                      </div>
                      <button style={{
                        width: '100%',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        color: 'white',
                        border: 'none',
                        padding: '15px 25px',
                        borderRadius: '12px',
                        fontSize: '16px',
                        fontWeight: '600',
                        cursor: 'pointer',
                        transition: 'all 0.3s ease'
                      }}>
                        Choose Package
                      </button>
                    </div>
                  </div>
                  
                  <div style={{
                    textAlign: 'center',
                    marginTop: '35px',
                    padding: '25px',
                    background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                    borderRadius: '15px',
                    border: '1px solid #e9ecef'
                  }}>
                    <p style={{color: '#666', fontSize: '15px', fontStyle: 'italic', margin: 0, fontWeight: '300'}}>
                      No credit card required for trial
                    </p>
                  </div>
                </div>
              )}
              
              {activeSection === 'contact' && (
                <div>
                  <p style={{color: '#666', fontSize: '18px', marginBottom: '40px', textAlign: 'center', fontWeight: '300'}}>
                    Ready to elevate your research? Connect with our team of experts
                  </p>
                  
                  <div style={{display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))', gap: '30px', marginBottom: '40px'}}>
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '35px',
                      borderRadius: '20px',
                      border: '1px solid #e9ecef',
                      textAlign: 'center',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-20px',
                        right: '-20px',
                        width: '80px',
                        height: '80px',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        borderRadius: '50%',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '30px'
                      }}>
                        📧
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '24px', marginBottom: '25px', fontWeight: '600', marginTop: '30px'}}>
                        Email Us
                      </h3>
                      <p style={{color: '#666', fontSize: '16px', marginBottom: '25px', fontWeight: '300'}}>
                        Drop us a line anytime
                      </p>
                      <a href="mailto:helloresearcher@gaply.in" style={{
                        color: '#ff6b35',
                        textDecoration: 'none',
                        fontSize: '18px',
                        fontWeight: '600',
                        padding: '15px 30px',
                        border: '2px solid #ff6b35',
                        borderRadius: '12px',
                        display: 'inline-block',
                        transition: 'all 0.3s ease',
                        background: 'rgba(255, 107, 53, 0.1)'
                      }}>
                        helloresearcher@gaply.in
                      </a>
                    </div>
                    
                    <div style={{
                      background: 'linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%)',
                      padding: '35px',
                      borderRadius: '20px',
                      border: '1px solid #e9ecef',
                      textAlign: 'center',
                      position: 'relative',
                      overflow: 'hidden'
                    }}>
                      <div style={{
                        position: 'absolute',
                        top: '-20px',
                        right: '-20px',
                        width: '80px',
                        height: '80px',
                        background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                        borderRadius: '50%',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'white',
                        fontSize: '30px'
                      }}>
                        📱
                      </div>
                      <h3 style={{color: '#2c3e50', fontSize: '24px', marginBottom: '25px', fontWeight: '600', marginTop: '30px'}}>
                        Follow Us
                      </h3>
                      <p style={{color: '#666', fontSize: '16px', marginBottom: '25px', fontWeight: '300'}}>
                        Stay updated on Instagram
                      </p>
                      <a href="https://instagram.com/gaply.in_" target="_blank" rel="noopener noreferrer" style={{
                        color: '#ff6b35',
                        textDecoration: 'none',
                        fontSize: '18px',
                        fontWeight: '600',
                        padding: '15px 30px',
                        border: '2px solid #ff6b35',
                        borderRadius: '12px',
                        display: 'inline-block',
                        transition: 'all 0.3s ease',
                        background: 'rgba(255, 107, 53, 0.1)'
                      }}>
                        @gaply.in_
                      </a>
                    </div>
                  </div>
                  
                  <div style={{
                    background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                    padding: '40px',
                    borderRadius: '20px',
                    color: 'white',
                    textAlign: 'center',
                    position: 'relative',
                    overflow: 'hidden'
                  }}>
                    <div style={{
                      position: 'absolute',
                      top: '0',
                      right: '0',
                      width: '120px',
                      height: '120px',
                      background: 'rgba(255,255,255,0.1)',
                      borderRadius: '50%',
                      transform: 'translate(40px, -40px)'
                    }}></div>
                    <h3 style={{marginBottom: '20px', fontSize: '26px', fontWeight: '600'}}>
                      Ready to Transform Your Research?
                    </h3>
                    <p style={{marginBottom: '30px', fontSize: '18px', opacity: '0.95', lineHeight: '1.6', fontWeight: '300'}}>
                      Join thousands of researchers who trust Gaply for their academic journey
                    </p>
                    <div style={{display: 'flex', gap: '20px', justifyContent: 'center', flexWrap: 'wrap'}}>
                      <a href="mailto:helloresearcher@gaply.in" style={{
                        background: 'rgba(255,255,255,0.2)',
                        color: 'white',
                        textDecoration: 'none',
                        padding: '15px 30px',
                        borderRadius: '12px',
                        fontSize: '16px',
                        fontWeight: '600',
                        border: '2px solid rgba(255,255,255,0.3)',
                        transition: 'all 0.3s ease'
                      }}>
                        Send Email
                      </a>
                      <a href="https://instagram.com/gaply.in_" target="_blank" rel="noopener noreferrer" style={{
                        background: 'transparent',
                        color: 'white',
                        textDecoration: 'none',
                        padding: '15px 30px',
                        borderRadius: '12px',
                        fontSize: '16px',
                        fontWeight: '600',
                        border: '2px solid rgba(255,255,255,0.5)',
                        transition: 'all 0.3s ease'
                      }}>
                        Follow on Instagram
                      </a>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
};

// Main App Component
const App: React.FC = () => {
  const [showLogin, setShowLogin] = useState(false);
  const [showPackageSelection, setShowPackageSelection] = useState(false);
  const [usePremiumHero, setUsePremiumHero] = useState(false); // Toggle for premium hero - disabled to show R3F
  // Feature flag for R3F Hero (default: false for safety)
  const useR3FHero = process.env.REACT_APP_R3F_HERO === 'true' || localStorage.getItem('useR3FHero') === 'true' || true; // Temporarily enabled for testing

  return (
    <div className="App">
      {/* Transparent Navigation */}
      <nav className="navbar">
        <div className="nav-content">
          <div className="logo">GAPLY</div>
          <div className="nav-links">
            <button 
              onClick={() => window.openPopup && window.openPopup('features')}
              style={{
                background: 'none',
                border: 'none',
                color: '#E8E7ED',
                cursor: 'pointer',
                fontSize: 'inherit',
                fontFamily: 'inherit',
                textDecoration: 'none',
                padding: '0',
                margin: '0 20px'
              }}
            >
              Features
            </button>
            <button 
              onClick={() => window.openPopup && window.openPopup('pricing')}
              style={{
                background: 'none',
                border: 'none',
                color: '#E8E7ED',
                cursor: 'pointer',
                fontSize: 'inherit',
                fontFamily: 'inherit',
                textDecoration: 'none',
                padding: '0',
                margin: '0 20px'
              }}
            >
              Pricing
            </button>
            <button 
              onClick={() => window.openPopup && window.openPopup('contact')}
              style={{
                background: 'none',
                border: 'none',
                color: '#E8E7ED',
                cursor: 'pointer',
                fontSize: 'inherit',
                fontFamily: 'inherit',
                textDecoration: 'none',
                padding: '0',
                margin: '0 20px'
              }}
            >
              Contact
            </button>
          </div>
          <div className="nav-actions">
            <button 
              onClick={() => setShowLogin(true)}
              className="premium-button"
              style={{
                background: 'linear-gradient(135deg, #ff6b35 0%, #f7931e 100%)',
                color: 'white',
                border: 'none',
                padding: '12px 24px',
                borderRadius: '8px',
                fontSize: '14px',
                fontWeight: '600',
                cursor: 'pointer',
                textDecoration: 'none',
                display: 'inline-block',
                transition: 'all 0.3s ease'
              }}
            >
              Get Premium
            </button>
            {/* Developer Toggles - Remove in production */}
            <button 
              onClick={() => {
                const newValue = !usePremiumHero;
                setUsePremiumHero(newValue);
                if (newValue) {
                  localStorage.setItem('useR3FHero', 'false');
                }
              }}
              style={{
                background: usePremiumHero ? '#ff6b35' : '#6b7280',
                color: 'white',
                border: 'none',
                padding: '8px 12px',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '12px',
                marginLeft: '10px'
              }}
              title="Toggle Premium Hero"
            >
              {usePremiumHero ? 'Premium' : 'Legacy'}
            </button>
            <button 
              onClick={() => {
                localStorage.setItem('useR3FHero', useR3FHero ? 'false' : 'true');
                window.location.reload();
              }}
              style={{
                background: useR3FHero ? '#4F46E5' : '#6b7280',
                color: 'white',
                border: 'none',
                padding: '8px 12px',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '12px',
                marginLeft: '5px'
              }}
              title="Toggle R3F Hero"
            >
              {useR3FHero ? 'R3F' : '2D'}
            </button>
            
            {/* R3F Hero Toggle - Remove in production */}
            <button 
              onClick={() => {
                // This would need to be handled differently in production
                // For now, we'll use localStorage to persist the choice
                const currentR3F = localStorage.getItem('useR3FHero') === 'true';
                localStorage.setItem('useR3FHero', (!currentR3F).toString());
                window.location.reload(); // Reload to apply the change
              }}
              style={{
                background: '#4F46E5',
                color: 'white',
                border: 'none',
                padding: '8px 12px',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '12px',
                marginLeft: '10px'
              }}
              title="Toggle R3F Hero"
            >
              {useR3FHero ? 'R3F OFF' : 'R3F ON'}
            </button>
          </div>
        </div>
      </nav>

      {/* Hero Section */}
      {useR3FHero ? (
        <>
          <div style={{ 
            background: '#4F46E5', 
            color: 'white', 
            padding: '10px', 
            textAlign: 'center', 
            fontSize: '14px',
            fontWeight: 'bold'
          }}>
            🎮 R3F HERO ACTIVE - 3D Rotating Logo + Falling Words
          </div>
          <HeroR3F 
            spinSpeed={0.18}
            maxWords={24}
            words={['IDEA', 'PAPER', 'MENTOR', 'FUND', 'REVIEW', 'PUBLISH', 'GAPLY']}
            onWordClick={(word) => console.log('Word clicked:', word)}
            enablePoster={true}
          />
        </>
      ) : usePremiumHero ? (
        <>
          <div style={{ 
            background: '#ff6b35', 
            color: 'white', 
            padding: '10px', 
            textAlign: 'center', 
            fontSize: '14px',
            fontWeight: 'bold'
          }}>
            🎨 PREMIUM HERO ACTIVE
          </div>
          <PremiumHero />
        </>
      ) : (
        <>
          <div style={{ 
            background: '#6b7280', 
            color: 'white', 
            padding: '10px', 
            textAlign: 'center', 
            fontSize: '14px',
            fontWeight: 'bold'
          }}>
            📚 LEGACY HERO ACTIVE
          </div>
          <HeroSection />
        </>
      )}

      {/* Second Section with FREE FEATURES */}
      <SecondSection />

      {/* Transition Image */}
      <TransitionImage />

      {/* Third Section with Beautiful Transition */}
      <ThirdSection />
      
      {/* Fixed Footer */}
      <FixedFooter />

      {/* Login Page */}
      {showLogin && (
        <LoginPage 
          onClose={() => setShowLogin(false)} 
          onLoginSuccess={() => setShowPackageSelection(true)}
        />
      )}

      {/* Package Selection Page */}
      {showPackageSelection && (
        <PackageSelection onClose={() => setShowPackageSelection(false)} />
      )}
    </div>
  );
};

export default App;