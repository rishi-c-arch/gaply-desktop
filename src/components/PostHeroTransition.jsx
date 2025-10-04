// src/components/PostHeroTransition.jsx
import React, { useEffect, useRef } from 'react';
import gsap from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';
import Lenis from 'lenis';

gsap.registerPlugin(ScrollTrigger);

export default function PostHeroTransition({ disable3D = false }) {
  const containerRef = useRef(null);
  const maskRef = useRef(null);
  const nextRef = useRef(null);
  const lenisRef = useRef(null);

  useEffect(() => {
    // Check for reduced motion preference
    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    
    if (prefersReducedMotion) {
      // Immediate reveal for reduced motion
      if (maskRef.current) {
        gsap.set(maskRef.current, { clipPath: 'circle(180% at 50% 50%)' });
      }
      if (nextRef.current) {
        gsap.set(nextRef.current, { autoAlpha: 1, y: 0 });
      }
      return;
    }

    // Init Lenis smooth scroller
    const lenis = new Lenis({
      duration: 1.2,
      easing: (t) => Math.min(1, 1.001 - Math.pow(2, -10 * t)), // smooth easing
      smooth: true,
      touchMultiplier: 2,
    });
    lenisRef.current = lenis;

    function raf(time) {
      lenis.raf(time);
      requestId = requestAnimationFrame(raf);
    }
    let requestId = requestAnimationFrame(raf);

    // GSAP + ScrollTrigger setup
    const ctx = gsap.context(() => {
      // references
      const container = containerRef.current;
      const mask = maskRef.current;
      const next = nextRef.current;

      if (!container || !mask || !next) return;

      // initial state: mask is small circle (covering center bottom) that will expand to reveal
      gsap.set(mask, { clipPath: 'circle(6% at 50% 80%)' });
      gsap.set(next, { autoAlpha: 0, y: 20 }); // hidden below

      // main scroll-driven timeline: pin the container, then expand mask
      const tl = gsap.timeline({
        scrollTrigger: {
          trigger: container,
          start: 'top top',
          end: 'bottom+=80%', // tune length as desired
          scrub: 0.6,
          pin: true,
          anticipatePin: 1,
          invalidateOnRefresh: true,
        }
      });

      // expand mask to wipe hero away (radius values tuned to give smooth diagonal-ish reveal)
      tl.to(mask, {
        duration: 0.9,
        ease: 'power2.inOut',
        clipPath: 'circle(180% at 50% 50%)' // huge radius to fully reveal
      }, 0);

      // stagger reveal of next section content as mask grows (starts slightly before full expansion)
      tl.to(next, {
        autoAlpha: 1,
        y: 0,
        duration: 0.6,
        ease: 'power3.out',
        stagger: 0.06
      }, 0.18);


      // cleanup on unmount
      return () => {
        tl.kill();
        ScrollTrigger.getAll().forEach(st => st.kill());
      };
    }, containerRef);

    return () => {
      // destroy Lenis
      if (requestId) cancelAnimationFrame(requestId);
      if (lenis) lenis.destroy();
      lenisRef.current = null;
      ctx.revert();
    };
  }, [disable3D]);

  return (
    <section ref={containerRef} style={{ position: 'relative', minHeight: '50vh', overflow: 'hidden' }}>
      {/* mask layer sits over hero + next content and its clipPath is animated */}
      <div ref={maskRef}
           style={{
             position: 'absolute',
             inset: 0,
             pointerEvents: 'none',
             // visual dim similar to Olha's site
             background: 'linear-gradient(180deg, rgba(0,0,0,0.04), rgba(0,0,0,0.06))',
             zIndex: 3
           }} />


      {/* Next section content is placed below but visually revealed as mask expands */}
      <div ref={nextRef} style={{ position: 'relative', zIndex: 2, paddingTop: '15vh', textAlign: 'center' }}>
        {/* Spinning Crown Animation */}
        <div className="crown-container" style={{ marginBottom: '2rem' }}>
          <div className="spinning-crown">
            <svg width="60" height="60" viewBox="0 0 60 60" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M30 5L35 20L50 20L38 30L43 45L30 35L17 45L22 30L10 20L25 20L30 5Z" fill="#FFD700" stroke="#FFA500" strokeWidth="2"/>
              <circle cx="30" cy="30" r="3" fill="#FFA500"/>
            </svg>
          </div>
          <h2 style={{ 
            fontSize: 'clamp(24px, 4vw, 36px)', 
            margin: '1rem 0 0 0', 
            color: '#1d1d1f',
            fontWeight: '800',
            letterSpacing: '-0.02em'
          }}>
            FREE FEATURES
          </h2>
        </div>

        {/* Image-based Free Features */}
        <div className="image-features-container" style={{ 
          display: 'flex', 
          justifyContent: 'center', 
          gap: '2rem', 
          marginTop: '2rem',
          flexWrap: 'wrap',
          maxWidth: '800px',
          margin: '2rem auto 0'
        }}>
          <div className="feature-item" style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            padding: '1.5rem',
            borderRadius: '16px',
            background: 'rgba(255, 255, 255, 0.8)',
            backdropFilter: 'blur(10px)',
            border: '1px solid rgba(255, 255, 255, 0.3)',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.1)',
            transition: 'all 0.3s ease',
            minWidth: '200px'
          }}>
            <div style={{ fontSize: '3rem', marginBottom: '1rem' }}>📝</div>
            <h3 style={{ margin: '0 0 0.5rem 0', color: '#1d1d1f', fontSize: '1.1rem', fontWeight: '600' }}>
              Academic AI Remover
            </h3>
            <p style={{ margin: 0, color: '#6e6e73', fontSize: '0.9rem', textAlign: 'center' }}>
              Transform AI text into scholarly excellence
            </p>
          </div>

          <div className="feature-item" style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            padding: '1.5rem',
            borderRadius: '16px',
            background: 'rgba(255, 255, 255, 0.8)',
            backdropFilter: 'blur(10px)',
            border: '1px solid rgba(255, 255, 255, 0.3)',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.1)',
            transition: 'all 0.3s ease',
            minWidth: '200px'
          }}>
            <div style={{ fontSize: '3rem', marginBottom: '1rem' }}>🔍</div>
            <h3 style={{ margin: '0 0 0.5rem 0', color: '#1d1d1f', fontSize: '1.1rem', fontWeight: '600' }}>
              Paper Search
            </h3>
            <p style={{ margin: 0, color: '#6e6e73', fontSize: '0.9rem', textAlign: 'center' }}>
              Find relevant research papers instantly
            </p>
          </div>

          <div className="feature-item" style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            padding: '1.5rem',
            borderRadius: '16px',
            background: 'rgba(255, 255, 255, 0.8)',
            backdropFilter: 'blur(10px)',
            border: '1px solid rgba(255, 255, 255, 0.3)',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.1)',
            transition: 'all 0.3s ease',
            minWidth: '200px'
          }}>
            <div style={{ fontSize: '3rem', marginBottom: '1rem' }}>🎯</div>
            <h3 style={{ margin: '0 0 0.5rem 0', color: '#1d1d1f', fontSize: '1.1rem', fontWeight: '600' }}>
              Journal Matching
            </h3>
            <p style={{ margin: 0, color: '#6e6e73', fontSize: '0.9rem', textAlign: 'center' }}>
              Get personalized journal recommendations
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
