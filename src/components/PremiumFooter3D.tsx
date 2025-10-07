import React, { useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';

const FooterOrbs: React.FC = () => {
  const groupRef = useRef<THREE.Group>(null);
  useFrame(({ clock }) => {
    if (!groupRef.current) return;
    groupRef.current.rotation.z = Math.sin(clock.elapsedTime * 0.08) * 0.02;
  });

  return (
    <group ref={groupRef}>
      <mesh position={[-2.8, 0.1, 0]}>
        <sphereGeometry args={[0.8, 32, 32]} />
        <meshStandardMaterial color="#0b1020" metalness={0.4} roughness={0.6} opacity={0.35} transparent />
      </mesh>
      <mesh position={[2.6, -0.2, 0]}>
        <sphereGeometry args={[0.6, 32, 32]} />
        <meshStandardMaterial color="#0e1429" metalness={0.4} roughness={0.6} opacity={0.28} transparent />
      </mesh>
      <mesh rotation={[Math.PI/2, 0, 0]}>
        <torusGeometry args={[4.2, 0.06, 16, 256]} />
        <meshBasicMaterial color="#0a0f1e" opacity={0.18} transparent />
      </mesh>
      <ambientLight intensity={0.6} />
      <directionalLight position={[3, 5, 6]} intensity={0.5} color={'#a5b4fc'} />
    </group>
  );
};

const PremiumFooter3D: React.FC = () => {
  return (
    <footer style={{
      position: 'relative',
      background: 'linear-gradient(180deg, #0b0f1a 0%, #0a0a0a 100%)',
      color: '#e5e7eb',
      overflow: 'hidden',
      borderTop: '1px solid rgba(255,255,255,0.06)'
    }}>
      {/* Seam blend: softly mix light page into dark footer (no animation) */}
      <div style={{
        position: 'absolute', left: 0, right: 0, top: -64, height: 64,
        background: 'linear-gradient(180deg, rgba(250,250,250,0) 0%, rgba(16,16,16,0.10) 45%, rgba(12,12,12,0.25) 70%, rgba(10,10,10,0.45) 100%)',
        pointerEvents: 'none'
      }} />
      <div style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
        <Canvas camera={{ position: [0, 0, 8], fov: 60 }} style={{ width: '100%', height: 240, background: 'transparent' }}>
          <FooterOrbs />
        </Canvas>
      </div>

      <div style={{ position: 'relative', zIndex: 2, maxWidth: 1200, margin: '0 auto', padding: '48px 20px' }}>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 24, alignItems: 'flex-start', justifyContent: 'space-between' }}>
          <div>
            <div style={{ fontSize: 24, fontWeight: 800, letterSpacing: '.02em' }}>Gaply</div>
            <div style={{ opacity: 0.7, marginTop: 8 }}>Accelerating academic research</div>
          </div>

          <nav style={{ display: 'grid', gridTemplateColumns: 'repeat(3, minmax(140px, 1fr))', gap: 16, minWidth: 420 }}>
            {[
              { label: 'Features', href: '#features' },
              { label: 'WHY WE EXIST', href: '#why' },
              { label: 'Pricing', href: '#pricing' },
              { label: 'Contact', href: '#contact' },
              { label: 'Privacy Policy', href: '#privacy' },
              { label: 'Terms of Service', href: '#terms' },
              { label: 'Careers', href: '#careers' },
              { label: 'Blog', href: '#blog' },
              { label: 'Docs', href: '#docs' },
              { label: 'Status', href: '#status' },
              { label: 'Support', href: '#support' },
            ].map((l) => (
              <a
                key={l.label}
                href={l.href}
                style={{
                  color: '#e5e7eb',
                  textDecoration: 'none',
                  opacity: 0.9,
                  padding: '10px 12px',
                  borderRadius: 10,
                  background: 'linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.02))',
                  border: '1px solid rgba(255,255,255,0.08)',
                  backdropFilter: 'blur(6px)',
                  transformStyle: 'preserve-3d',
                  transition: 'transform .2s ease, box-shadow .2s ease, opacity .2s ease'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.transform = 'translateZ(8px)';
                  e.currentTarget.style.boxShadow = '0 10px 24px -12px rgba(59,130,246,0.35)';
                  e.currentTarget.style.opacity = '1';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.transform = 'none';
                  e.currentTarget.style.boxShadow = 'none';
                  e.currentTarget.style.opacity = '0.9';
                }}
              >
                {l.label}
              </a>
            ))}
          </nav>
        </div>

        <div style={{ marginTop: 28, opacity: 0.6, fontSize: 12, display: 'flex', justifyContent: 'space-between', flexWrap: 'wrap', gap: 12 }}>
          <span>© {new Date().getFullYear()} Gaply. All rights reserved.</span>
          <span style={{ letterSpacing: '.03em' }}>Built with care for researchers worldwide.</span>
        </div>
      </div>
    </footer>
  );
};

export default PremiumFooter3D;


