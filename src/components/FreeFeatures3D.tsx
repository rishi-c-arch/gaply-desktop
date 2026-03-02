import React, { useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import DNAHelix3D from './DNAHelix3D';
import { useInView } from '../hooks/useInView';
import { getParticleCount } from '../hooks/useMediaQuery';
import { Search, FileSearch } from 'lucide-react';
import '../styles/dna-sections.css';

const FreeFeatures3D: React.FC = () => {
  const navigate = useNavigate();
  const sectionRef = useRef<HTMLElement>(null);
  const inView = useInView(sectionRef, { threshold: 0.1 });
  const particleCount = getParticleCount();

  const features = [
    {
      id: 'journal-matching',
      title: 'Journal Matching',
      description: 'Find the perfect journal for your research paper with our AI-powered matching system',
      icon: Search,
      path: '/journal-matching',
    },
    {
      id: 'paper-search',
      title: 'Paper Search',
      description: 'Search through millions of academic papers and find relevant research instantly',
      icon: FileSearch,
      path: '/paper-search',
    },
  ];

  return (
    <section
      ref={sectionRef}
      className="dna-section-free"
      aria-labelledby="free-features-heading"
    >
      <div className="section-container">
        <DNAHelix3D
          position="left"
          colors={{
            strand1: '#6366F1',
            strand2: '#8B5CF6',
            particles: '#06B6D4',
            connectors: 'rgba(99, 102, 241, 0.3)',
          }}
          geometry={{
            radius: 2.5,
            height: 400,
            turns: 3.5,
            tubeRadius: 0.12,
          }}
          particles={{
            count: 80,
            size: 0.06,
            glowIntensity: 0.6,
          }}
          animation={{
            autoRotate: true,
            rotationSpeed: 0.005,
            particleOrbit: true,
          }}
          inView={inView}
          particleCountOverride={particleCount}
        />
        <div className="section-content">
          <h2 id="free-features-heading" className="section-heading">
            Discover Our Free Features
          </h2>
          <span className="free-badge">Free</span>
          <div className="feature-cards">
            {features.map((feature) => (
              <article
                key={feature.id}
                className="feature-card"
                tabIndex={0}
                aria-label={`${feature.title} feature`}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    navigate(feature.path);
                  }
                }}
                onClick={() => navigate(feature.path)}
                style={{ cursor: 'pointer' }}
              >
                <div style={{ display: 'flex', alignItems: 'flex-start', gap: 16 }}>
                  <div
                    style={{
                      width: 48,
                      height: 48,
                      borderRadius: 12,
                      background: 'rgba(99, 102, 241, 0.2)',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      flexShrink: 0,
                    }}
                  >
                    <feature.icon size={24} style={{ color: '#6366F1' }} aria-hidden />
                  </div>
                  <div>
                    <h3 style={{ margin: '0 0 8px', fontSize: '1.15rem', fontWeight: 600 }}>
                      {feature.title}
                    </h3>
                    <p style={{ margin: 0, fontSize: '0.95rem', lineHeight: 1.55, opacity: 0.9 }}>
                      {feature.description}
                    </p>
                  </div>
                </div>
              </article>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
};

export default FreeFeatures3D;
