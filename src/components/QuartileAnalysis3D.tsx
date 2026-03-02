import React, { useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import DNAHelix3D from './DNAHelix3D';
import { useInView } from '../hooks/useInView';
import { getParticleCount } from '../hooks/useMediaQuery';
import '../styles/dna-sections.css';

const quartiles = [
  {
    q: 'q1',
    label: 'Q1',
    stars: 5,
    range: 'Top 25%',
    description: 'Highest impact journals in your field',
  },
  {
    q: 'q2',
    label: 'Q2',
    stars: 4,
    range: '25-50%',
    description: 'Above average impact journals',
  },
  {
    q: 'q3',
    label: 'Q3',
    stars: 3,
    range: '50-75%',
    description: 'Average impact journals',
  },
  {
    q: 'q4',
    label: 'Q4',
    stars: 2,
    range: '75-100%',
    description: 'Emerging or specialized journals',
  },
];

const QuartileAnalysis3D: React.FC = () => {
  const navigate = useNavigate();
  const sectionRef = useRef<HTMLElement>(null);
  const inView = useInView(sectionRef, { threshold: 0.1 });
  const particleCount = getParticleCount();

  return (
    <section
      ref={sectionRef}
      className="dna-section-quartile"
      aria-labelledby="quartile-heading"
    >
      <DNAHelix3D
        position="background"
        colors={{
          strand1: '#6366F1',
          strand2: '#8B5CF6',
          particles: '#FFFFFF',
        }}
        geometry={{
          radius: 6,
          height: 800,
          turns: 2,
          tubeRadius: 0.08,
        }}
        particles={{
          count: 50,
          size: 0.04,
          glowIntensity: 0.3,
        }}
        animation={{
          autoRotate: true,
          rotationSpeed: 0.002,
          particleOrbit: false,
        }}
        opacity={0.15}
        inView={inView}
        particleCountOverride={particleCount}
      />
      <div className="section-inner">
        <h2 id="quartile-heading" className="section-heading" style={{ textAlign: 'center', marginBottom: 12 }}>
          Journal Quartile Analysis
        </h2>
        <div
          style={{
            width: 80,
            height: 2,
            background: 'linear-gradient(90deg, transparent, rgba(99,102,241,0.6), transparent)',
            margin: '0 auto 24px',
            opacity: 0.8,
          }}
        />
        <div className="quartile-grid">
          {quartiles.map((item) => (
            <article
              key={item.q}
              className={`quartile-card ${item.q}`}
              tabIndex={0}
              aria-label={`${item.label} quartile: ${item.range}`}
            >
              <div className="quartile-label">{item.label}</div>
              <div className="quartile-stars" aria-hidden>
                {'★'.repeat(item.stars)}
              </div>
              <div className="quartile-range">{item.range}</div>
              <p className="quartile-desc">{item.description}</p>
            </article>
          ))}
        </div>
        <p
          style={{
            textAlign: 'center',
            maxWidth: 560,
            margin: '0 auto 28px',
            fontSize: '1rem',
            lineHeight: 1.6,
            opacity: 0.9,
          }}
        >
          Understand journal rankings with our comprehensive quartile analysis tool. Make informed
          decisions about where to publish your research.
        </p>
        <div style={{ textAlign: 'center' }}>
          <button
            type="button"
            onClick={() => navigate('/journal-matching')}
            aria-label="Analyze journal quartiles"
            style={{
              background: 'linear-gradient(135deg, #6366F1, #8B5CF6)',
              color: 'white',
              padding: '14px 32px',
              borderRadius: 12,
              fontWeight: 600,
              border: 'none',
              cursor: 'pointer',
              fontSize: '1rem',
              transition: 'transform 0.3s ease, box-shadow 0.3s ease',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.transform = 'translateY(-2px)';
              e.currentTarget.style.boxShadow = '0 10px 30px rgba(99, 102, 241, 0.4)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.transform = 'translateY(0)';
              e.currentTarget.style.boxShadow = 'none';
            }}
          >
            Analyze Journal
          </button>
        </div>
      </div>
    </section>
  );
};

export default QuartileAnalysis3D;
