import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';

const FreeFeatures3D: React.FC = () => {
  const [hoveredCard, setHoveredCard] = useState<string | null>(null);
  const navigate = useNavigate();

  const portfolioItems = [
    {
      id: 'search',
      title: 'Research Paper Search',
      videoSrc: '/videos/P.mov',
      description: 'Discover relevant academic papers'
    },
    {
      id: 'journals',
      title: 'Journal Matching',
      videoSrc: '/videos/Journal_Matching_v2.mov',
      description: 'Find the perfect journal for your research'
    },
  ];

  const handleCardClick = (itemId: string) => {
    console.log(`Clicked on ${itemId}`);
    
    // Navigate to the appropriate feature page
    switch (itemId) {
      case 'search':
        navigate('/paper-search');
        break;
      case 'journals':
        navigate('/journal-matching');
        break;
      default:
        console.log(`Unknown feature: ${itemId}`);
    }
  };

  return (
    <div style={{ 
      minHeight: '80vh', 
      backgroundColor: 'var(--section-bg)', 
      padding: 'clamp(56px, 6vw, 90px) clamp(20px, 5vw, 48px)',
      color: 'var(--section-text)',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
    }}>
      {/* Header */}
      <div style={{ 
        textAlign: 'center', 
        marginBottom: 'clamp(28px, 5vw, 64px)',
        maxWidth: 'min(92vw, 960px)',
        marginLeft: 'auto',
        marginRight: 'auto',
        padding: '0 6px'
      }}>
        <h2 style={{ 
          fontSize: 'clamp(1.6rem, 4.2vw, 3.4rem)', 
          fontWeight: '300',
          letterSpacing: '-0.02em',
          marginBottom: 'clamp(16px, 3vw, 28px)',
          color: 'var(--section-text)',
          lineHeight: '1.15',
          wordBreak: 'break-word'
        }}>
          Discover Our Free Features
        </h2>
        <div style={{
          width: 'clamp(36px, 6vw, 64px)',
          height: '1px',
          backgroundColor: 'var(--divider)',
          margin: '0 auto',
          opacity: '0.3'
        }} />
      </div>

      {/* Cards Container */}
      <div style={{ 
        maxWidth: '1000px', 
        margin: '0 auto',
        width: '100%',
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
        gap: 'clamp(16px, 3.5vw, 32px)',
        alignItems: 'stretch'
      }}>
        {portfolioItems.map((item) => {
          return (
            <div
              key={item.id}
              style={{
                backgroundColor: 'var(--card-bg)',
                borderRadius: '20px',
                overflow: 'hidden',
                border: '1px solid var(--card-border)',
                cursor: 'pointer',
                transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                transform: hoveredCard === item.id ? 'translateY(-6px)' : 'translateY(0)',
                boxShadow: hoveredCard === item.id
                  ? 'var(--card-shadow-hover)' 
                  : 'var(--card-shadow)',
                position: 'relative',
                width: '100%',
                opacity: '1'
              }}
              onMouseEnter={() => setHoveredCard(item.id)}
              onMouseLeave={() => setHoveredCard(null)}
              onClick={() => handleCardClick(item.id)}
            >
            {/* Video Section */}
            <div style={{ 
              position: 'relative', 
              height: 'clamp(160px, 22vw, 250px)', 
              overflow: 'hidden',
              background: 'var(--media-bg)'
            }}>
              <video
                style={{
                  width: '100%',
                  height: '100%',
                  objectFit: 'contain',
                  objectPosition: 'center',
                  transition: 'all 0.8s ease',
                  filter: hoveredCard === item.id ? 'brightness(1.1) contrast(1.05)' : 'brightness(1) contrast(1)',
                  transform: hoveredCard === item.id ? 'scale(1.02)' : 'scale(1)'
                }}
                autoPlay
                loop
                muted
                playsInline
              >
                <source src={item.videoSrc} type="video/quicktime" />
                <source src={item.videoSrc} type="video/mp4" />
                Your browser does not support the video tag.
              </video>
              
              {/* Elegant Overlay */}
              <div style={{
                position: 'absolute',
                top: 0,
                left: 0,
                right: 0,
                bottom: 0,
                background: 'var(--media-overlay)',
                transition: 'opacity 0.6s ease',
                opacity: hoveredCard === item.id ? '0.3' : '0.6'
              }} />
              

              {/* Elegant FREE Badge */}
              <div style={{
                position: 'absolute',
                top: '20px',
                right: '20px',
                backgroundColor: 'var(--badge-bg)',
                color: 'var(--badge-text)',
                padding: '6px 14px',
                borderRadius: '20px',
                fontSize: '11px',
                fontWeight: '600',
                letterSpacing: '0.6px',
                backdropFilter: 'blur(20px)',
                border: '1px solid var(--badge-border)',
                textTransform: 'uppercase'
              }}>
                Free
              </div>
            </div>

            {/* Content Section */}
            <div style={{ 
              padding: 'clamp(18px, 3vw, 28px)',
              background: 'var(--content-bg)',
              borderTop: '1px solid var(--divider)'
            }}>
              {/* Feature Name */}
              <h3 style={{ 
                fontSize: 'clamp(1.05rem, 2vw, 1.3rem)', 
                fontWeight: '400', 
                marginBottom: '12px',
                color: 'var(--section-text)',
                letterSpacing: '-0.01em',
                lineHeight: '1.3',
                transition: 'color 0.6s ease'
              }}>
                {item.title}
              </h3>
              
              {/* Description */}
              <p style={{ 
                color: 'var(--muted-text)', 
                marginBottom: 'clamp(18px, 3vw, 28px)',
                fontSize: 'clamp(0.85rem, 1.6vw, 0.95rem)',
                lineHeight: '1.55',
                fontWeight: '300',
                letterSpacing: '0.01em'
              }}>
                {item.description}
              </p>

              {/* Elegant Button */}
              <button
                style={{
                  width: '100%',
                  backgroundColor: 'var(--button-bg)',
                  color: 'var(--button-text)',
                  border: '1px solid var(--button-border)',
                  padding: '12px 24px',
                  borderRadius: '10px',
                  fontSize: '12px',
                  fontWeight: '600',
                  letterSpacing: '0.5px',
                  cursor: 'pointer',
                  transition: 'all 0.6s ease',
                  textTransform: 'uppercase',
                  position: 'relative',
                  overflow: 'hidden'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--button-bg-hover)';
                  e.currentTarget.style.borderColor = 'var(--button-border-hover)';
                  e.currentTarget.style.transform = 'translateY(-2px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--button-bg)';
                  e.currentTarget.style.borderColor = 'var(--button-border)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                Explore Feature
              </button>
            </div>

            {/* Subtle Glow Effect */}
            <div style={{
              position: 'absolute',
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              borderRadius: '24px',
              background: 'var(--card-glow)',
              opacity: hoveredCard === item.id ? '1' : '0',
              transition: 'opacity 0.6s ease',
              pointerEvents: 'none'
            }} />
            </div>
          );
        })}
      </div>

      {/* Elegant Footer */}
      <div style={{ 
        textAlign: 'center', 
        marginTop: 'clamp(40px, 6vw, 70px)',
        paddingTop: 'clamp(36px, 5vw, 60px)',
        borderTop: '1px solid var(--divider)'
      }}>
        <p style={{ 
          color: 'var(--muted-text)', 
          fontSize: 'clamp(0.9rem, 2vw, 1rem)',
          fontWeight: '300',
          letterSpacing: '0.02em'
        }}>
          Click on any card to explore the feature
        </p>
      </div>
    </div>
  );
};

export default FreeFeatures3D;// Force rebuild
