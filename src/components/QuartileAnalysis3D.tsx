import React, { useEffect, useRef } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';

const HeadingDecoration: React.FC = () => {
  const ringRef = useRef<THREE.Mesh>(null);
  const particlesRef = useRef<THREE.Group>(null);

  useFrame(({ clock }) => {
    if (ringRef.current) {
      ringRef.current.rotation.x = Math.PI / 2;
      ringRef.current.rotation.z = clock.elapsedTime * 0.12;
    }
    if (particlesRef.current) {
      particlesRef.current.rotation.z = clock.elapsedTime * 0.05;
    }
  });

  return (
    <>
      <mesh ref={ringRef} position={[0, 0, 0]}>
        <torusGeometry args={[2.2, 0.08, 16, 256]} />
        <meshStandardMaterial color="#000000" opacity={0.08} transparent metalness={0.2} roughness={0.8} />
      </mesh>
      <group ref={particlesRef}>
        {Array.from({ length: 40 }).map((_, i) => (
          <mesh key={i} position={[
            Math.cos((i / 40) * Math.PI * 2) * (2.4 + Math.random() * 0.2),
            Math.sin((i / 40) * Math.PI * 2) * (0.2 + Math.random() * 0.1),
            (Math.random() - 0.5) * 0.3
          ]}>
            <sphereGeometry args={[0.03, 8, 8]} />
            <meshBasicMaterial color="#000000" opacity={0.1} transparent />
          </mesh>
        ))}
      </group>
      <ambientLight intensity={0.6} />
      <directionalLight position={[2, 3, 4]} intensity={0.4} />
    </>
  );
};

const QuartileAnalysis3D: React.FC = () => {
  const q1Ref = useRef<HTMLCanvasElement>(null);
  const q2Ref = useRef<HTMLCanvasElement>(null);
  const q3Ref = useRef<HTMLCanvasElement>(null);
  const q4Ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let isMounted = true;
    // Load Chart.js via CDN just-in-time
    const s = document.createElement('script');
    s.src = 'https://cdn.jsdelivr.net/npm/chart.js';
    s.async = true;
    s.onload = () => {
      if (!isMounted) return;
      // @ts-ignore
      const Chart = (window as any).Chart;
      if (!Chart) return;

      Chart.defaults.color = '#1f2937';
      Chart.defaults.font.family = 'Inter, system-ui, -apple-system, Segoe UI, Roboto, sans-serif';
      Chart.defaults.borderColor = 'rgba(31,41,55,0.08)';

      const makeDoughnut = (canvas: HTMLCanvasElement | null, data: number[], onClick: () => void) => {
        if (!canvas) return null;
        const ctx = canvas.getContext('2d');
        if (!ctx) return null;
        return new Chart(ctx, {
          type: 'doughnut',
          data: {
            labels: new Array(data.length).fill(''),
            datasets: [{
              data,
              backgroundColor: ['#475569','#64748b','#94a3b8','#cbd5e1','#e2e8f0','#9ca3af','#818cf8','#60a5fa'].slice(0, data.length),
              borderColor: 'rgba(255,255,255,0.6)',
              borderWidth: 2,
              hoverOffset: 6
            }]
          },
          options: {
            maintainAspectRatio: false,
            responsive: true,
            cutout: '68%',
            plugins: { legend: { display: false }, tooltip: { enabled: false } },
            onClick: () => onClick()
          }
        });
      };

      // Data mirrors repository buckets
      makeDoughnut(q1Ref.current, [25,25,25,25], () => window.open('/reports/journal-q1-analysis.html','_blank'));
      makeDoughnut(q2Ref.current, [20,20,20,20,20], () => window.open('/reports/journal-q2-analysis.html','_blank'));
      makeDoughnut(q3Ref.current, [20,20,20,20,20], () => window.open('/reports/journal-q3-analysis.html','_blank'));
      makeDoughnut(q4Ref.current, [25,25,25,25], () => window.open('/reports/journal-q4-analysis.html','_blank'));
    };
    document.body.appendChild(s);
    return () => { isMounted = false; document.body.removeChild(s); };
  }, []);

  return (
    <section className="third-section" style={{ 
      background: 
        'linear-gradient(180deg, rgba(26,26,26,1) 0%, rgba(26,26,26,0.85) 6%, rgba(26,26,26,0.5) 14%, rgba(26,26,26,0.2) 22%, rgba(26,26,26,0) 30%), ' +
        'radial-gradient(1200px 400px at 50% 0%, #f2f4f7 0%, #f7f7f8 40%, #fafafa 100%)', 
      color: '#111',
      position: 'relative'
    }}>
      {/* Color blending layer between sections */}
      <div style={{
        position: 'absolute',
        bottom: 0,
        left: 0,
        right: 0,
        height: '120px',
        background: 'linear-gradient(180deg, rgba(248,248,248,0) 0%, rgba(236,236,236,0.35) 25%, rgba(200,200,200,0.65) 55%, rgba(80,80,80,0.85) 85%, rgba(18,18,18,1) 100%)',
        pointerEvents: 'none',
        zIndex: 1
      }} />
      <div className="container" style={{ padding: '60px 20px', maxWidth: 1200, margin: '0 auto', position: 'relative', zIndex: 2 }}>
        <div style={{ position: 'relative', paddingTop: 20, paddingBottom: 20 }}>
          <div style={{
            position: 'absolute', left: 0, right: 0, top: 0, bottom: 0,
            pointerEvents: 'none'
          }}>
            <Canvas
              camera={{ position: [0, 0, 6], fov: 55 }}
              style={{ width: '100%', height: 160, background: 'transparent' }}
            >
              <HeadingDecoration />
            </Canvas>
          </div>

          <div style={{ position: 'relative', zIndex: 2, textAlign: 'center', perspective: 800 }}>
            <h2 style={{
              fontFamily: 'Inter, sans-serif', fontWeight: 800, letterSpacing: '.02em',
              fontSize: 'clamp(26px,5vw,48px)', margin: 0,
              transform: 'translateZ(20px)'
            }}>JOURNAL QUARTILE ANALYSIS</h2>
            <p style={{ opacity: 0.7, marginTop: 10, transform: 'translateZ(10px)' }}>
              Interactive exploration of Q1-Q4 journal categories with detailed insights and guidelines
            </p>
          </div>
        </div>

        <div style={{ position: 'relative', marginTop: 40 }}>
          {/* Subtle decorative background behind grid */}
          <div style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
            <Canvas camera={{ position: [0, 0, 10], fov: 60 }} style={{ width: '100%', height: 260, background: 'transparent' }}>
              <group position={[0, -0.5, 0]}>
                <mesh rotation={[Math.PI/2, 0, 0]}>
                  <torusGeometry args={[5, 0.12, 16, 256]} />
                  <meshBasicMaterial color={'#111111'} opacity={0.05} transparent />
                </mesh>
                {Array.from({ length: 24 }).map((_, i) => (
                  <mesh key={i} position={[
                    Math.cos((i/24)*Math.PI*2)*5.2,
                    Math.sin((i/24)*Math.PI*2)*0.2,
                    0
                  ]}>
                    <sphereGeometry args={[0.06, 8, 8]} />
                    <meshBasicMaterial color={'#111111'} opacity={0.06} transparent />
                  </mesh>
                ))}
              </group>
            </Canvas>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, minmax(220px,1fr))', gap: 28, alignItems: 'start', position: 'relative', zIndex: 2 }}>
          {[{title:'Q1', ref:q1Ref, sub:'Q1 TOP-TIER INTERNATIONAL JOURNALS BY DOMAIN'},
            {title:'Q2', ref:q2Ref, sub:'Q2 STRONG REPUTABLE INTERNATIONAL JOURNALS BY DOMAIN'},
            {title:'Q3', ref:q3Ref, sub:'Q3 REGIONAL SPECIALIZED INTERNATIONAL JOURNALS BY DOMAIN'},
            {title:'Q4', ref:q4Ref, sub:'Q4 EMERGING INTERNATIONAL JOURNALS BY DOMAIN'}].map((item) => (
            <div key={item.title} style={{ textAlign: 'center', transformStyle: 'preserve-3d', transition: 'transform 0.25s ease, box-shadow 0.25s ease', borderRadius: 16, padding: 10 }}
              onMouseEnter={(e) => { (e.currentTarget as HTMLDivElement).style.transform = 'perspective(800px) translateZ(8px) rotateX(2deg)'; (e.currentTarget as HTMLDivElement).style.boxShadow = '0 20px 40px -20px rgba(2,6,23,0.15)'; }}
              onMouseLeave={(e) => { (e.currentTarget as HTMLDivElement).style.transform = 'none'; (e.currentTarget as HTMLDivElement).style.boxShadow = 'none'; }}
            >
              <div style={{ fontWeight: 800, fontSize: 22, marginBottom: 14 }}>{item.title}</div>
              <div style={{ height: 180, width: '100%', maxWidth: 220, margin: '0 auto', filter: 'drop-shadow(0 10px 22px rgba(2,6,23,0.08))' }}>
                <canvas ref={item.ref} />
              </div>
              <div style={{ marginTop: 16, fontSize: 12, letterSpacing: 0.3, color: '#374151', textTransform: 'uppercase', fontWeight: 700 }}>{item.sub}</div>
            </div>
          ))}
          </div>
        </div>
      </div>
    </section>
  );
};

export default QuartileAnalysis3D;
