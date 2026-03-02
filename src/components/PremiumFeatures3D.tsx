import React, { useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import DNAHelix3D from './DNAHelix3D';
import { useInView } from '../hooks/useInView';
import { getParticleCount } from '../hooks/useMediaQuery';
import { Rocket, BarChart3, Shield, MessageSquare } from 'lucide-react';
import '../styles/dna-sections.css';

const PremiumFeatures3D: React.FC = () => {
  const navigate = useNavigate();
  const sectionRef = useRef<HTMLElement>(null);
  const inView = useInView(sectionRef, { threshold: 0.1 });
  const particleCount = getParticleCount();

  const features = [
    {
      id: 'publishready',
      title: 'PublishReady',
      description: 'Complete journal submission preparation and formatting service',
      icon: Rocket,
      path: '/dashboard/publishready',
    },
    {
      id: 'datamaestro',
      title: 'DataMaestro',
      description: 'Advanced statistical analysis and SPSS support for your research',
      icon: BarChart3,
      path: '/datamaestro-pro',
    },
    {
      id: 'ai-detection',
      title: 'AI Detection',
      description: 'Advanced AI content detection and humanization for academic papers',
      icon: Shield,
      path: '/academic-ai-remover',
    },
    {
      id: 'research-assistant',
      title: 'Research Assistant',
      description: '24/7 AI-powered research guidance and writing support',
      icon: MessageSquare,
      path: '/paper-search',
    },
  ];

  return (
    <section
      ref={sectionRef}
      className="dna-section-premium"
      aria-labelledby="premium-features-heading"
    >
      <div className="section-container">
        <div className="section-content">
          <h2 id="premium-features-heading" className="section-heading">
            Premium Features
          </h2>
          <span className="premium-badge">Premium</span>
          <div className="premium-grid">
            {features.map((feature) => (
              <article
                key={feature.id}
                className="premium-card"
                tabIndex={0}
                aria-label={`${feature.title} feature`}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    navigate(feature.path);
                  }
                }}
                onClick={() => navigate(feature.path)}
              >
                <div
                  style={{
                    width: 44,
                    height: 44,
                    borderRadius: 12,
                    background: 'rgba(245, 158, 11, 0.2)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    marginBottom: 12,
                  }}
                >
                  <feature.icon size={22} style={{ color: '#F59E0B' }} aria-hidden />
                </div>
                <h3 style={{ margin: '0 0 8px', fontSize: '1.1rem', fontWeight: 600 }}>
                  {feature.title}
                </h3>
                <p style={{ margin: 0, fontSize: '0.9rem', lineHeight: 1.5, opacity: 0.9 }}>
                  {feature.description}
                </p>
              </article>
            ))}
          </div>
          <button
            type="button"
            className="get-premium-btn"
            onClick={() => navigate('/pricing')}
            aria-label="Get Premium subscription"
          >
            Get Premium
          </button>
        </div>
        <DNAHelix3D
          position="right"
          colors={{
            strand1: '#F59E0B',
            strand2: '#D97706',
            particles: '#FBBF24',
            connectors: 'rgba(245, 158, 11, 0.3)',
          }}
          geometry={{
            radius: 2.5,
            height: 500,
            turns: 4,
            tubeRadius: 0.12,
          }}
          particles={{
            count: 100,
            size: 0.07,
            glowIntensity: 0.7,
          }}
          animation={{
            autoRotate: true,
            rotationSpeed: -0.005,
            particleOrbit: true,
          }}
          inView={inView}
          particleCountOverride={particleCount}
        />
      </div>
    </section>
  );
};

export default PremiumFeatures3D;
