import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';

const PremiumFeatures3D: React.FC = () => {
  const [hoveredCard, setHoveredCard] = useState<string | null>(null);
  const navigate = useNavigate();

  const portfolioItems = [
    {
      id: 'research-gap',
      title: 'Research Gap Finder AI Agent',
      videoSrc: '/videos/r.mov',
      description: 'Analyze 5 base papers to identify research gaps and opportunities'
    },
    {
      id: 'journal-evaluator',
      title: 'Journal Evaluator AI Agent',
      videoSrc: '/videos/Journal_Evaluator.mov',
      description: 'Evaluate your research paper with 70+ parameters'
    },
    {
      id: 'talk-gaply',
      title: 'Talk with Gaply.ai',
      videoSrc: '/videos/ai.mov',
      description: 'Chat with AI about your research papers and doubts'
    },
  ];

  const handleCardClick = (itemId: string) => {
    console.log(`Clicked on ${itemId}`);
    
    // Navigate to the appropriate premium feature page
    switch (itemId) {
      case 'research-gap':
        navigate('/research-gap-finder');
        break;
      case 'journal-evaluator':
        navigate('/journal-evaluator');
        break;
      case 'talk-gaply':
        navigate('/talk-gaply');
        break;
      default:
        console.log(`Unknown feature: ${itemId}`);
    }
  };

  return (
    <div style={{ 
      minHeight: '80vh', 
      backgroundColor: '#000000', 
      padding: window.innerWidth <= 768 ? '60px 20px' : '80px 40px',
      color: 'white',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
    }}>
      {/* Header */}
      <div style={{ textAlign: 'center', marginBottom: window.innerWidth <= 768 ? '40px' : '60px' }}>
        <h2 style={{ 
          fontSize: window.innerWidth <= 480 ? 'clamp(1.6rem, 6.4vw, 2.4rem)' : window.innerWidth <= 768 ? 'clamp(2rem, 4.8vw, 3.2rem)' : '4rem', 
          fontWeight: '300',
          letterSpacing: '-0.02em',
          marginBottom: '30px',
          color: '#ffffff',
          lineHeight: '1.1'
        }}>
          Premium Features
        </h2>
        <div style={{
          width: window.innerWidth <= 768 ? '40px' : '60px',
          height: '1px',
          backgroundColor: '#ffffff',
          margin: '0 auto',
          opacity: '0.3'
        }} />
      </div>

      {/* Cards Container */}
      <div style={{ 
        maxWidth: '1400px', 
        margin: '0 auto',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        gap: '40px',
        perspective: '1000px'
      }}>
        {portfolioItems.map((item, index) => {
          const isMiddle = index === 1;
          const isLeft = index === 0;
          
          return (
            <div
              key={item.id}
              style={{
                backgroundColor: '#000000',
                borderRadius: '24px',
                overflow: 'hidden',
                border: '1px solid rgba(255, 255, 255, 0.1)',
                cursor: 'pointer',
                transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                transform: isMiddle 
                  ? 'translateZ(20px) scale(1.05)' 
                  : isLeft 
                    ? 'translateZ(-10px) translateX(-20px) scale(0.95)' 
                    : 'translateZ(-10px) translateX(20px) scale(0.95)',
                boxShadow: isMiddle 
                  ? '0 40px 80px rgba(255, 255, 255, 0.15), 0 0 0 1px rgba(255, 255, 255, 0.2)' 
                  : '0 20px 40px rgba(0, 0, 0, 0.4)',
                position: 'relative',
                width: '320px',
                opacity: isMiddle ? '1' : '0.9'
              }}
              onMouseEnter={() => setHoveredCard(item.id)}
              onMouseLeave={() => setHoveredCard(null)}
              onClick={() => handleCardClick(item.id)}
            >
            {/* Video Section */}
            <div style={{ 
              position: 'relative', 
              height: window.innerWidth <= 480 ? '180px' : window.innerWidth <= 768 ? '220px' : '280px', 
              overflow: 'hidden',
              background: 'linear-gradient(135deg, #1a1a1a 0%, #000000 100%)'
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
                background: 'linear-gradient(135deg, rgba(0,0,0,0.1) 0%, rgba(0,0,0,0.4) 100%)',
                transition: 'opacity 0.6s ease',
                opacity: hoveredCard === item.id ? '0.3' : '0.6'
              }} />
              

              {/* Elegant PREMIUM Badge */}
              <div style={{
                position: 'absolute',
                top: '24px',
                right: '24px',
                background: 'linear-gradient(135deg, rgba(255, 215, 0, 0.2) 0%, rgba(255, 165, 0, 0.2) 100%)',
                color: '#FFD700',
                padding: '8px 16px',
                borderRadius: '20px',
                fontSize: '12px',
                fontWeight: '500',
                letterSpacing: '0.5px',
                backdropFilter: 'blur(20px)',
                border: '1px solid rgba(255, 215, 0, 0.3)',
                textTransform: 'uppercase',
                boxShadow: '0 4px 12px rgba(255, 215, 0, 0.2)'
              }}>
                Premium
              </div>
            </div>

            {/* Content Section */}
            <div style={{ 
              padding: '32px',
              background: 'linear-gradient(135deg, #0a0a0a 0%, #000000 100%)',
              borderTop: '1px solid rgba(255, 255, 255, 0.05)'
            }}>
              {/* Feature Name */}
              <h3 style={{ 
                fontSize: '1.5rem', 
                fontWeight: '400', 
                marginBottom: '12px',
                color: '#ffffff',
                letterSpacing: '-0.01em',
                lineHeight: '1.3',
                transition: 'color 0.6s ease'
              }}>
                {item.title}
              </h3>
              
              {/* Description */}
              <p style={{ 
                color: 'rgba(255, 255, 255, 0.7)', 
                marginBottom: '32px',
                fontSize: '1rem',
                lineHeight: '1.6',
                fontWeight: '300',
                letterSpacing: '0.01em'
              }}>
                {item.description}
              </p>

              {/* Elegant Button */}
              <button style={{
                width: '100%',
                background: 'linear-gradient(135deg, rgba(255, 215, 0, 0.1) 0%, rgba(255, 165, 0, 0.1) 100%)',
                color: '#FFD700',
                border: '1px solid rgba(255, 215, 0, 0.3)',
                padding: '16px 32px',
                borderRadius: '12px',
                fontSize: '14px',
                fontWeight: '500',
                letterSpacing: '0.5px',
                cursor: 'pointer',
                transition: 'all 0.6s ease',
                textTransform: 'uppercase',
                position: 'relative',
                overflow: 'hidden'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = 'linear-gradient(135deg, rgba(255, 215, 0, 0.2) 0%, rgba(255, 165, 0, 0.2) 100%)';
                e.currentTarget.style.borderColor = 'rgba(255, 215, 0, 0.6)';
                e.currentTarget.style.transform = 'translateY(-2px)';
                e.currentTarget.style.boxShadow = '0 8px 20px rgba(255, 215, 0, 0.3)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'linear-gradient(135deg, rgba(255, 215, 0, 0.1) 0%, rgba(255, 165, 0, 0.1) 100%)';
                e.currentTarget.style.borderColor = 'rgba(255, 215, 0, 0.3)';
                e.currentTarget.style.transform = 'translateY(0)';
                e.currentTarget.style.boxShadow = 'none';
              }}
              >
                Explore Premium
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
              background: 'linear-gradient(135deg, rgba(255, 215, 0, 0.05) 0%, rgba(255, 165, 0, 0.02) 100%)',
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
        marginTop: '60px',
        paddingTop: '60px',
        borderTop: '1px solid rgba(255, 255, 255, 0.1)'
      }}>
        <p style={{ 
          color: 'rgba(255, 255, 255, 0.5)', 
          fontSize: '1rem',
          fontWeight: '300',
          letterSpacing: '0.02em'
        }}>
          Click on any card to explore premium features
        </p>
      </div>
    </div>
  );
};

export default PremiumFeatures3D;

