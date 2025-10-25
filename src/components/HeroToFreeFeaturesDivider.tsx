import React, { useEffect, useRef } from 'react';
import gsap from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';

// Register GSAP plugins
gsap.registerPlugin(ScrollTrigger);

const HeroToFreeFeaturesDivider: React.FC = () => {
  const dividerRef = useRef<HTMLDivElement>(null);
  const lineRef = useRef<HTMLDivElement>(null);
  const textRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!dividerRef.current || !lineRef.current || !textRef.current) return;

    // Create timeline for divider animation
    const tl = gsap.timeline({
      scrollTrigger: {
        trigger: dividerRef.current,
        start: "top 80%",
        end: "bottom 20%",
        toggleActions: "play none none reverse",
        markers: false, // Set to true for debugging
      }
    });

    // Animate line expansion
    tl.fromTo(lineRef.current, 
      { 
        scaleX: 0,
        opacity: 0
      },
      { 
        scaleX: 1,
        opacity: 1,
        duration: 1.5, 
        ease: "power3.out" 
      }
    );

    // Animate text appearance
    tl.fromTo(textRef.current, 
      { 
        opacity: 0,
        y: 30,
        scale: 0.8
      },
      { 
        opacity: 1,
        y: 0,
        scale: 1,
        duration: 0.8, 
        ease: "back.out(1.7)" 
      },
      "-=0.8"
    );

    // Cleanup
    return () => {
      ScrollTrigger.getAll().forEach(trigger => trigger.kill());
    };
  }, []);

  return (
    <div 
      ref={dividerRef}
      style={{
        position: 'relative',
        width: '100%',
        height: '200px',
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'center',
        alignItems: 'center',
        background: 'linear-gradient(180deg, rgba(0, 0, 0, 0.8) 0%, rgba(0, 0, 0, 1) 100%)',
        overflow: 'hidden',
      }}
    >
      {/* Animated divider line */}
      <div
        ref={lineRef}
        style={{
          width: '80%',
          height: '2px',
          background: 'linear-gradient(90deg, transparent 0%, #007AFF 20%, #5856D6 50%, #AF52DE 80%, transparent 100%)',
          borderRadius: '1px',
          marginBottom: '40px',
          transformOrigin: 'center',
        }}
      />

      {/* Divider text */}
      <div
        ref={textRef}
        style={{
          fontSize: 'clamp(1.2rem, 3vw, 1.8rem)',
          fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
          fontWeight: '600',
          color: '#a0a0a0',
          letterSpacing: '-0.01em',
          textAlign: 'center',
          opacity: 0.8,
        }}
      >
        Discover Our Free Features
      </div>

      {/* Subtle background particles */}
      <div
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          width: '100%',
          height: '100%',
          pointerEvents: 'none',
          zIndex: 0,
        }}
      >
        {Array.from({ length: 20 }).map((_, i) => (
          <div
            key={i}
            style={{
              position: 'absolute',
              width: '2px',
              height: '2px',
              background: `hsl(${200 + i * 5}, 60%, 60%)`,
              borderRadius: '50%',
              left: `${5 + i * 4.5}%`,
              top: `${30 + (i % 3) * 20}%`,
              opacity: 0.3,
              animation: `float ${3 + i * 0.2}s ease-in-out infinite alternate`,
              animationDelay: `${i * 0.1}s`,
            }}
          />
        ))}
      </div>

      <style dangerouslySetInnerHTML={{
        __html: `
          @keyframes float {
            0% { transform: translateY(0px) scale(1); opacity: 0.3; }
            100% { transform: translateY(-10px) scale(1.1); opacity: 0.6; }
          }
        `
      }} />
    </div>
  );
};

export default HeroToFreeFeaturesDivider;
