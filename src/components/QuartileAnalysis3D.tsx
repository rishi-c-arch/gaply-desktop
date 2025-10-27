import React, { useEffect, useRef, useState } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';

const InteractiveChart: React.FC<{ data: number[], colors: string[], title: string, subtitle: string, quartile: string }> = ({ data, colors, title, subtitle, quartile }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [isHovered, setIsHovered] = useState(false);

  useEffect(() => {
    let isMounted = true;
    const s = document.createElement('script');
    s.src = 'https://cdn.jsdelivr.net/npm/chart.js';
    s.async = true;
    s.onload = () => {
      if (!isMounted) return;
      // @ts-ignore
      const Chart = (window as any).Chart;
      if (!Chart || !canvasRef.current) return;

      Chart.defaults.color = '#ffffff';
      Chart.defaults.font.family = '-apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif';
      Chart.defaults.borderColor = 'rgba(255,255,255,0.3)';

      const ctx = canvasRef.current.getContext('2d');
      if (!ctx) return;

      const chart = new Chart(ctx, {
        type: 'doughnut',
        data: {
          labels: new Array(data.length).fill(''),
          datasets: [{
            data,
            backgroundColor: colors,
            borderColor: 'rgba(255,255,255,0.3)',
            borderWidth: 2,
            hoverOffset: 8,
            shadowOffsetX: 4,
            shadowOffsetY: 4,
            shadowBlur: 10,
            shadowColor: 'rgba(0,0,0,0.3)'
          }]
        },
        options: {
          maintainAspectRatio: false,
          responsive: true,
          cutout: '70%',
          plugins: { 
            legend: { display: false }, 
            tooltip: { 
              enabled: true,
              backgroundColor: 'rgba(0,0,0,0.8)',
              titleColor: '#ffffff',
              bodyColor: '#ffffff',
              borderColor: 'rgba(255,255,255,0.3)',
              borderWidth: 1,
              padding: 12,
              callbacks: {
                label: (context: any) => `${context.parsed}%`
              }
            }
          },
          animation: {
            animateRotate: true,
            animateScale: true,
            duration: 1500,
            easing: 'easeOutCubic'
          },
          onHover: (event: any) => {
            if (event.native) {
              const canvas = event.native.target as HTMLCanvasElement;
              canvas.style.cursor = 'pointer';
            }
          },
          onClick: () => {
            window.open(`/reports/journal-${quartile.toLowerCase()}-analysis.html`, '_blank');
          }
        }
      });

      return () => {
        if (chart) chart.destroy();
      };
    };
    document.body.appendChild(s);
    return () => { 
      isMounted = false; 
      try { document.body.removeChild(s); } catch(e) {}
    };
  }, [data, colors]);

  return (
    <div 
      style={{ 
        position: 'relative',
        cursor: 'pointer',
        transition: 'transform 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
        transform: isHovered ? 'scale(1.05)' : 'scale(1)'
      }}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <div style={{
        position: 'absolute',
        inset: 0,
        background: isHovered 
          ? 'radial-gradient(circle, rgba(147, 51, 234, 0.15) 0%, transparent 70%)'
          : 'transparent',
        borderRadius: '50%',
        transition: 'all 0.6s ease',
        pointerEvents: 'none',
        zIndex: 1
      }} />
      <canvas ref={canvasRef} style={{ 
        width: '100%', 
        height: '100%',
        filter: isHovered ? 'brightness(1.1) drop-shadow(0 10px 30px rgba(147, 51, 234, 0.4))' : 'drop-shadow(0 5px 15px rgba(0,0,0,0.2))',
        transition: 'all 0.6s ease',
        position: 'relative',
        zIndex: 2
      }} />
    </div>
  );
};

const QuartileAnalysis3D: React.FC = () => {
  const q1Ref = useRef<HTMLCanvasElement>(null);
  const q2Ref = useRef<HTMLCanvasElement>(null);
  const q3Ref = useRef<HTMLCanvasElement>(null);
  const q4Ref = useRef<HTMLCanvasElement>(null);

  // Luxury purple-blue gradient colors
  const colorSchemes = {
    q1: ['#667eea', '#764ba2', '#8b5cf6', '#a78bfa'], // Purple to violet
    q2: ['#4c51bf', '#667eea', '#7c3aed', '#9333ea'], // Deep purple
    q3: ['#5b21b6', '#7c3aed', '#9d4edd', '#c084fc'], // Rich purple
    q4: ['#6d28d9', '#8b5cf6', '#a855f7', '#c084fc'] // Vibrant purple-blue
  };

  const quartileData = {
    q1: [25, 25, 25, 25],
    q2: [20, 20, 20, 20, 20],
    q3: [20, 20, 20, 20, 20],
    q4: [25, 25, 25, 25]
  };

  return (
    <div style={{ 
      minHeight: '80vh', 
      backgroundColor: '#000000',
      background: `
        linear-gradient(135deg, #000000 0%, #0a0a0a 25%, #1a0a2e 50%, #16213e 75%, #000000 100%),
        radial-gradient(ellipse at top right, rgba(147, 51, 234, 0.15), transparent 50%),
        radial-gradient(ellipse at bottom left, rgba(59, 130, 246, 0.15), transparent 50%)
      `,
      padding: window.innerWidth <= 768 ? '80px 20px' : '120px 40px',
      color: 'white',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif',
      position: 'relative',
      overflow: 'hidden'
    }}>
      {/* Subtle background pattern */}
      <div style={{
        position: 'absolute',
        inset: 0,
        background: `
          repeating-linear-gradient(
            45deg,
            transparent,
            transparent 50px,
            rgba(147, 51, 234, 0.02) 50px,
            rgba(147, 51, 234, 0.02) 100px
          )
        `,
        pointerEvents: 'none'
      }} />

      {/* Header */}
      <div style={{ textAlign: 'center', marginBottom: window.innerWidth <= 768 ? '60px' : '80px' }}>
        <h2 style={{ 
          fontSize: window.innerWidth <= 480 ? 'clamp(1.6rem, 6.4vw, 2.4rem)' : window.innerWidth <= 768 ? 'clamp(2rem, 4.8vw, 3.2rem)' : '4rem', 
          fontWeight: '300',
          letterSpacing: '-0.02em',
          marginBottom: '20px',
          color: '#ffffff',
          lineHeight: '1.1',
          background: 'linear-gradient(135deg, #ffffff 0%, #e0e7ff 50%, #ddd6fe 100%)',
          WebkitBackgroundClip: 'text',
          WebkitTextFillColor: 'transparent',
          backgroundClip: 'text'
        }}>
          Journal Quartile Analysis
        </h2>
        <p style={{
          color: 'rgba(255, 255, 255, 0.7)',
          fontSize: '1.1rem',
          fontWeight: '300',
          letterSpacing: '0.02em',
          maxWidth: '800px',
          margin: '0 auto'
        }}>
          Interactive exploration of Q1-Q4 journal categories with detailed insights and guidelines
        </p>
        <div style={{
          width: window.innerWidth <= 768 ? '40px' : '60px',
          height: '1px',
          background: 'linear-gradient(90deg, transparent, rgba(147, 51, 234, 0.5), transparent)',
          margin: '30px auto',
          opacity: '0.6'
        }} />
      </div>

      {/* Quartile Cards */}
      <div style={{ 
        maxWidth: '1400px', 
        margin: '0 auto',
        display: 'grid',
        gridTemplateColumns: window.innerWidth <= 768 ? 'repeat(2, 1fr)' : 'repeat(4, 1fr)',
        gap: '30px'
      }}>
        {[
          { quartile: 'q1', title: 'Q1', colors: colorSchemes.q1, subtitle: 'TOP-TIER INTERNATIONAL JOURNALS', data: quartileData.q1 },
          { quartile: 'q2', title: 'Q2', colors: colorSchemes.q2, subtitle: 'REPUTABLE INTERNATIONAL JOURNALS', data: quartileData.q2 },
          { quartile: 'q3', title: 'Q3', colors: colorSchemes.q3, subtitle: 'SPECIALIZED INTERNATIONAL JOURNALS', data: quartileData.q3 },
          { quartile: 'q4', title: 'Q4', colors: colorSchemes.q4, subtitle: 'EMERGING INTERNATIONAL JOURNALS', data: quartileData.q4 }
        ].map((item, index) => (
          <div
            key={item.quartile}
            style={{
              backgroundColor: 'rgba(255, 255, 255, 0.02)',
              borderRadius: '24px',
              padding: '40px 32px',
              border: '1px solid rgba(147, 51, 234, 0.2)',
              cursor: 'pointer',
              transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
              position: 'relative',
              overflow: 'hidden'
            }}
            onMouseEnter={(e) => {
              (e.currentTarget as HTMLDivElement).style.borderColor = 'rgba(147, 51, 234, 0.5)';
              (e.currentTarget as HTMLDivElement).style.backgroundColor = 'rgba(147, 51, 234, 0.05)';
              (e.currentTarget as HTMLDivElement).style.transform = 'translateY(-8px)';
            }}
            onMouseLeave={(e) => {
              (e.currentTarget as HTMLDivElement).style.borderColor = 'rgba(147, 51, 234, 0.2)';
              (e.currentTarget as HTMLDivElement).style.backgroundColor = 'rgba(255, 255, 255, 0.02)';
              (e.currentTarget as HTMLDivElement).style.transform = 'translateY(0)';
            }}
            onClick={() => window.open(`/reports/journal-${item.quartile}-analysis.html`, '_blank')}
          >
            {/* Gradient glow effect */}
            <div style={{
              position: 'absolute',
              top: 0,
              left: 0,
              right: 0,
              height: '50%',
              background: `linear-gradient(135deg, ${item.colors[0]}20, ${item.colors[1]}10, transparent)`,
              opacity: 0,
              transition: 'opacity 0.6s ease',
              pointerEvents: 'none',
              zIndex: 0
            }} className="quartile-glow" />

            {/* Content */}
            <div style={{ position: 'relative', zIndex: 2, textAlign: 'center' }}>
              <div style={{
                fontSize: '1.8rem',
                fontWeight: '400',
                marginBottom: '20px',
                color: '#ffffff',
                letterSpacing: '-0.01em'
              }}>
                {item.title}
              </div>

              <div style={{
                height: window.innerWidth <= 768 ? '180px' : '220px',
                width: '100%',
                maxWidth: '280px',
                margin: '0 auto 24px',
                position: 'relative'
              }}>
                <InteractiveChart
                  data={item.data}
                  colors={item.colors}
                  title={item.title}
                  subtitle={item.subtitle}
                  quartile={item.quartile}
                />
              </div>

              <div style={{
                fontSize: window.innerWidth <= 768 ? '10px' : '12px',
                fontWeight: '500',
                letterSpacing: '0.1em',
                color: 'rgba(255, 255, 255, 0.6)',
                textTransform: 'uppercase',
                lineHeight: '1.6'
              }}>
                {item.subtitle}
              </div>
            </div>

            {/* Hover glow effect */}
            <div style={{
              position: 'absolute',
              bottom: 0,
              left: 0,
              right: 0,
              height: '2px',
              background: `linear-gradient(90deg, ${item.colors[0]}, ${item.colors[1]}, ${item.colors[2]})`,
              opacity: 0,
              transition: 'opacity 0.6s ease',
              pointerEvents: 'none'
            }} className="quartile-border-glow" />
          </div>
        ))}
      </div>

      {/* Footer */}
      <div style={{ 
        textAlign: 'center', 
        marginTop: '80px',
        paddingTop: '60px',
        borderTop: '1px solid rgba(147, 51, 234, 0.2)'
      }}>
        <p style={{ 
          color: 'rgba(255, 255, 255, 0.5)', 
          fontSize: '1rem',
          fontWeight: '300',
          letterSpacing: '0.02em'
        }}>
          Click on any quartile to explore detailed analysis
        </p>
      </div>
    </div>
  );
};

export default QuartileAnalysis3D;
