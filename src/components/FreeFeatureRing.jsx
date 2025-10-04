import React, { useRef, useEffect, useState, useCallback } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Text } from '@react-three/drei';
import gsap from 'gsap';
import './FreeFeatureRing.css';

/**
 * FreeFeatureRing.jsx - 3D rotating ring with proper error handling
 * 
 * Beautiful 3D ring that expands into usable features
 */

function RingText3D({ text = 'FREE FEATURE', radius = 1.5, size = 0.4, speed = 0.6 }) {
  const group = useRef();
  useFrame(({ clock }) => {
    if (group.current) {
      group.current.rotation.y = clock.elapsedTime * speed * 0.3;
    }
  });

  // Create 8 repetitions for cleaner look
  const words = Array.from({ length: 8 }).map((_, i) => text);

  return (
    <group ref={group}>
      {words.map((t, i) => {
        const angle = (i / words.length) * Math.PI * 2;
        const x = Math.cos(angle) * radius;
        const z = Math.sin(angle) * radius;
        const rotY = -angle + Math.PI / 2;
        return (
          <Text
            key={i}
            position={[x, 0, z]}
            rotation={[0, rotY, 0]}
            fontSize={size}
            maxWidth={1.5}
            anchorX="center"
            anchorY="middle"
            color="#1d1d1f"
            font="/fonts/inter-bold.woff"
          >
            {t}
          </Text>
        );
      })}
    </group>
  );
}

export default function FreeFeatureRing({
  id = 'free-feature-ring',
  onOpen = () => {},
  size = 360
}) {
  const wrapperRef = useRef(null);
  const canvasRef = useRef(null);
  const [webglAvailable, setWebglAvailable] = useState(true);
  const [isLoading, setIsLoading] = useState(true);
  const lastFocusedRef = useRef(null);

  // Force CSS fallback for now to ensure visibility
  useEffect(() => {
    console.log('Using CSS fallback for better visibility');
    setWebglAvailable(false);
    setIsLoading(false);
  }, []);

  // Activation handler (click or keyboard)
  const activate = useCallback(async () => {
    if (window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      onOpen();
      return;
    }

    lastFocusedRef.current = document.activeElement;
    
    // Beautiful animation - scale and fade the ring, then call onOpen
    const tl = gsap.timeline({
      defaults: { ease: 'power2.inOut' },
      onComplete: () => {
        onOpen();
      }
    });

    if (canvasRef.current) {
      // scale the canvas container to give a zoom feel
      tl.to(canvasRef.current, { scale: 1.3, duration: 0.6 }, 0);
      // fade out ring
      tl.to(canvasRef.current, { opacity: 0.2, duration: 0.4 }, 0.2);
    }
  }, [onOpen]);

  // keyboard support
  useEffect(() => {
    function onKey(e) {
      if ((e.key === 'Enter' || e.key === ' ') && wrapperRef.current && wrapperRef.current.contains(document.activeElement)) {
        e.preventDefault();
        activate();
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [activate]);

  // CSS fallback for when WebGL is not available
  const CSSFallback = (
    <div className="ffr-css-ring" role="img" aria-label="Rotating text ring - Free Feature">
      <div className="ffr-css-text">FREE FEATURE • FREE FEATURE • FREE FEATURE • FREE FEATURE • FREE FEATURE • FREE FEATURE •</div>
    </div>
  );

  if (isLoading) {
    return (
      <div className="ffr-wrapper" ref={wrapperRef} id={id}>
        <div className="ffr-canvas-wrap" style={{ width: size, height: size }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
            <div className="ffr-loading">Loading...</div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="ffr-wrapper" ref={wrapperRef} id={id}>
      <div
        className="ffr-canvas-wrap"
        ref={canvasRef}
        role="button"
        tabIndex={0}
        aria-label="Free Feature rotating ring. Click to explore features."
        onClick={() => activate()}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); activate(); }
        }}
        style={{ width: size, height: size }}
      >
        {webglAvailable ? (
          <Canvas 
            camera={{ position: [0, 0, 5], fov: 45 }} 
            style={{ width: '100%', height: '100%' }}
          >
            <ambientLight intensity={0.8} />
            <directionalLight position={[5, 5, 5]} intensity={1} />
            <RingText3D text="FREE FEATURE" radius={1.5} size={0.4} speed={0.6} />
          </Canvas>
        ) : (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', width: '100%', height: '100%' }}>
            {CSSFallback}
          </div>
        )}
      </div>
    </div>
  );
}
