import React, { useEffect, useRef } from 'react';
import gsap from 'gsap';

// TypeScript declaration for model-viewer
declare global {
  namespace JSX {
    interface IntrinsicElements {
      'model-viewer': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement> & {
        src?: string;
        poster?: string;
        alt?: string;
        ar?: boolean;
        autoplay?: boolean;
        'auto-rotate'?: boolean;
        'rotation-per-second'?: string;
        'camera-controls'?: boolean;
        'shadow-intensity'?: string;
        onMouseEnter?: () => void;
        onMouseLeave?: () => void;
        onTouchStart?: () => void;
        onTouchEnd?: () => void;
      };
    }
  }
}

const PremiumHero: React.FC = () => {
  const headerRef = useRef<HTMLElement>(null);
  const heroRef = useRef<HTMLElement>(null);
  const headlineRef = useRef<HTMLHeadingElement>(null);
  const modelRef = useRef<any>(null);

  useEffect(() => {
    // Load model-viewer script
    const script = document.createElement('script');
    script.type = 'module';
    script.src = 'https://unpkg.com/@google/model-viewer/dist/model-viewer.min.js';
    document.head.appendChild(script);

    // Header entrance animation
    gsap.fromTo(headerRef.current,
      { y: -100, opacity: 0 },
      { y: 0, opacity: 1, duration: 0.8, ease: "power3.out" }
    );

    // Headline entrance animation
    gsap.fromTo(headlineRef.current,
      { y: 50, opacity: 0 },
      { y: 0, opacity: 1, duration: 1, ease: "power3.out", delay: 0.3 }
    );

    // Parallax effect for model
    const handleScroll = () => {
      const scrollY = window.scrollY;
      if (modelRef.current) {
        gsap.to(modelRef.current, {
          y: scrollY * 0.3,
          duration: 0.3,
          ease: "none"
        });
      }
    };

    window.addEventListener('scroll', handleScroll);
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  const handleModelInteraction = () => {
    if (modelRef.current) {
      modelRef.current.pause = true;
    }
  };

  const handleModelLeave = () => {
    if (modelRef.current) {
      modelRef.current.pause = false;
    }
  };

  return (
    <>
      {/* Sticky Header */}
      <header 
        ref={headerRef}
        className="sticky top-0 z-50 backdrop-blur-md bg-white/40 border-b border-white/20"
        style={{ height: window.innerWidth >= 768 ? '72px' : '56px' }}
      >
        <div className="max-w-6xl mx-auto px-6 h-full flex items-center justify-between">
          {/* Left: Logo */}
          <div className="flex items-center gap-4">
            <a href="/" aria-label="Gaply home" className="flex items-center">
              <div className="h-8 w-8 bg-gradient-to-br from-indigo-600 to-purple-600 rounded-lg flex items-center justify-center">
                <span className="text-white font-bold text-lg">G</span>
              </div>
              <span className="ml-2 text-xl font-bold text-gray-900">GAPLY</span>
            </a>
          </div>

          {/* Center: Optional Search */}
          <div className="hidden lg:flex items-center">
            <button 
              className="flex items-center gap-2 px-3 py-2 rounded-lg bg-white/60 backdrop-blur-sm border border-gray-200 text-gray-500 hover:text-gray-700 transition-colors"
              aria-label="Search (Ctrl+K)"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
              </svg>
              <span className="text-sm">Search...</span>
              <kbd className="text-xs bg-gray-100 px-1 rounded">⌘K</kbd>
            </button>
          </div>

          {/* Right: Navigation & CTAs */}
          <div className="hidden md:flex items-center gap-6">
            <nav className="flex gap-6 text-sm">
              <a href="/about" className="text-gray-700 hover:text-gray-900 transition-colors">About</a>
              <a href="/mentors" className="text-gray-700 hover:text-gray-900 transition-colors">Mentors</a>
              <a href="/projects" className="text-gray-700 hover:text-gray-900 transition-colors">Projects</a>
            </nav>
            <div className="flex items-center gap-3">
              <a 
                href="/pricing" 
                className="px-4 py-2 rounded-lg text-sm border border-gray-200 text-gray-700 hover:bg-gray-50 transition-colors"
                aria-label="View pricing plans"
              >
                Pricing
              </a>
              <a 
                href="/premium" 
                className="px-5 py-2 rounded-lg bg-indigo-600 text-white font-semibold shadow-sm hover:shadow-lg hover:scale-105 transition-all duration-160"
                aria-label="Get premium access"
              >
                Get Premium
              </a>
            </div>
          </div>

          {/* Mobile Menu Button */}
          <button 
            className="md:hidden p-2 rounded-lg hover:bg-white/60 transition-colors"
            aria-label="Open mobile menu"
          >
            <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
            </svg>
          </button>
        </div>
      </header>

      {/* Hero Section */}
      <main 
        ref={heroRef}
        className="bg-gradient-to-b from-[rgba(120,120,128,0.5)] via-transparent to-transparent min-h-screen"
      >
        <section className="max-w-6xl mx-auto px-6 py-20 lg:grid lg:grid-cols-2 gap-12 items-center">
          {/* Left Column - Content */}
          <div className="lg:w-[52%]">
            <h1 
              ref={headlineRef}
              className="text-3xl md:text-5xl lg:text-6xl font-extrabold leading-[1.02] text-gray-900"
              style={{ fontFamily: 'Inter, SF Pro, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif' }}
            >
              THIS IS THE PLATFORM THAT MAKES <span className="text-indigo-600">GENIUS</span> LOOK EFFORTLESS
            </h1>

            <p className="mt-6 text-lg md:text-xl text-gray-700 max-w-xl font-medium">
              Publish a 300-word idea. Get fast peer feedback. Find mentors & funding.
            </p>

            {/* CTA Buttons */}
            <div className="mt-10 flex gap-4">
              <a 
                href="/start" 
                className="px-6 py-3 rounded-xl bg-indigo-600 text-white font-medium shadow-md hover:scale-[1.03] transition-transform duration-160"
                aria-label="Start free trial"
              >
                Start Free
              </a>
              <a 
                href="/mentors" 
                className="px-5 py-3 rounded-xl border border-gray-200 text-gray-700 hover:bg-gray-50 transition-colors"
                aria-label="Explore mentors"
              >
                Explore Mentors
              </a>
            </div>

            {/* Stats Block */}
            <div className="mt-10 grid grid-cols-3 gap-6 max-w-md">
              <div>
                <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>10K+</div>
                <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Research Papers</div>
              </div>
              <div>
                <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>500+</div>
                <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Journals</div>
              </div>
              <div>
                <div className="text-3xl md:text-4xl font-bold text-gray-900" style={{ letterSpacing: '0.5px' }}>95%</div>
                <div className="text-xs uppercase tracking-wider text-gray-500 mt-1">Accuracy</div>
              </div>
            </div>
          </div>

          {/* Right Column - 3D Model */}
          <div className="lg:w-[48%] flex justify-center lg:justify-end mt-12 lg:mt-0">
            <div 
              className="w-[420px] md:w-[520px] lg:w-[620px] h-[420px] md:h-[520px] lg:h-[620px] rounded-2xl shadow-2xl bg-white/60 backdrop-blur-sm overflow-hidden relative"
              style={{ minWidth: '420px', maxWidth: '640px', minHeight: '420px', maxHeight: '640px' }}
            >
              {/* 3D Model Viewer */}
              <model-viewer
                ref={modelRef}
                src="/models/research_scene_draco.glb"
                poster="/images/model-poster.png"
                alt="Interactive 3D research scene showing academic papers and research tools"
                ar
                autoplay
                auto-rotate
                rotation-per-second="0.02"
                camera-controls
                shadow-intensity="1"
                style={{ 
                  width: "100%", 
                  height: "100%", 
                  background: "transparent" 
                }}
                onMouseEnter={handleModelInteraction}
                onMouseLeave={handleModelLeave}
                onTouchStart={handleModelInteraction}
                onTouchEnd={handleModelLeave}
              >
                {/* Fallback poster image */}
                <div className="absolute inset-0 bg-gradient-to-br from-indigo-100 to-purple-100 flex items-center justify-center">
                  <div className="text-center">
                    <div className="w-24 h-24 mx-auto mb-4 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-2xl flex items-center justify-center">
                      <svg className="w-12 h-12 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                      </svg>
                    </div>
                    <p className="text-gray-600 font-medium">3D Research Scene</p>
                    <p className="text-sm text-gray-500 mt-1">Loading interactive model...</p>
                  </div>
                </div>
              </model-viewer>

              {/* Soft drop shadow */}
              <div className="absolute -bottom-4 left-4 right-4 h-8 bg-gradient-to-t from-black/10 to-transparent rounded-full blur-xl"></div>
            </div>
          </div>
        </section>
      </main>
    </>
  );
};

export default PremiumHero;
