import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import SEO from './SEO';

const FeaturesPage: React.FC = () => {
  const navigate = useNavigate();
  const [activeSection, setActiveSection] = useState<'free' | 'premium'>('free');
  const [hoveredFeature, setHoveredFeature] = useState<string | null>(null);

  // Free Features Data
  const freeFeatures = [
    {
      id: 'ai-remover',
      title: 'Academic AI Remover',
      subtitle: 'Transform AI text to scholarly excellence',
      description: 'Advanced paraphrasing engine that converts AI-generated text into authentic academic writing with scholarly vocabulary and natural flow.',
      features: [
        'Bypasses Turnitin, GPTZero, Crossplag',
        'Advanced academic language processing',
        'Perfect grammar and structure',
        'Human-like writing patterns',
        'Real-time text transformation'
      ],
      color: 'rgba(0, 122, 255, 0.1)',
      borderColor: 'rgba(0, 122, 255, 0.3)'
    },
    {
      id: 'paper-search',
      title: 'Research Paper Search',
      subtitle: 'Discover relevant academic papers',
      description: 'Intelligent search engine that finds the most relevant academic papers based on your research topic and keywords.',
      features: [
        'Multi-database integration (arXiv, CrossRef, OpenAlex)',
        'Smart keyword matching',
        'Direct links to full papers',
        'Citation tracking and metrics',
        'Real-time academic database access'
      ],
      color: 'rgba(52, 199, 89, 0.1)',
      borderColor: 'rgba(52, 199, 89, 0.3)'
    },
    {
      id: 'journal-matching',
      title: 'Journal Matching',
      subtitle: 'Find the perfect journal for your research',
      description: 'AI-powered journal recommendation system that matches your research with the most suitable academic journals.',
      features: [
        'Intelligent domain analysis',
        'Impact factor consideration',
        'Submission guidelines integration',
        'Success rate optimization',
        'Direct submission links'
      ],
      color: 'rgba(255, 149, 0, 0.1)',
      borderColor: 'rgba(255, 149, 0, 0.3)'
    }
  ];

  // Premium Features Data
  const premiumFeatures = [
    {
      id: 'research-gaps',
      title: 'Research Gap Analysis',
      subtitle: 'Discover unexplored research opportunities',
      description: 'AI-powered analysis that identifies genuine research gaps based on existing literature and emerging trends.',
      features: [
        'Comprehensive literature analysis',
        'Trend identification and prediction',
        'Gap validation and verification',
        'Research opportunity scoring',
        'Publication potential assessment'
      ],
      color: 'rgba(255, 45, 85, 0.1)',
      borderColor: 'rgba(255, 45, 85, 0.3)'
    },
    {
      id: 'problem-statement',
      title: 'Problem Statement Generator',
      subtitle: 'Craft compelling research problems',
      description: 'Advanced AI that generates clear, testable problem statements with objectives and hypotheses.',
      features: [
        'Clear objective formulation',
        'Testable hypothesis generation',
        'Research question optimization',
        'Methodology alignment',
        'Academic writing standards'
      ],
      color: 'rgba(175, 82, 222, 0.1)',
      borderColor: 'rgba(175, 82, 222, 0.3)'
    },
    {
      id: 'deep-evaluation',
      title: 'Deep Research Evaluation',
      subtitle: '70+ point comprehensive analysis',
      description: 'Thorough evaluation system that checks journal fit, highlights weaknesses, and provides detailed proofreading.',
      features: [
        '70+ evaluation criteria',
        'Journal compatibility analysis',
        'Weakness identification',
        'Detailed proofreading',
        'Improvement recommendations'
      ],
      color: 'rgba(90, 200, 250, 0.1)',
      borderColor: 'rgba(90, 200, 250, 0.3)'
    },
    {
      id: 'mentor-matching',
      title: 'Expert Mentor Matching',
      subtitle: 'Connect with verified field experts',
      description: 'Personalized mentor matching system that connects you with verified experts in your research field.',
      features: [
        'Verified expert profiles',
        'Field-specific matching',
        'Publication history review',
        'One-on-one guidance',
        'Publication strategy planning'
      ],
      color: 'rgba(255, 204, 0, 0.1)',
      borderColor: 'rgba(255, 204, 0, 0.3)'
    }
  ];

  return (
    <div style={{
      minHeight: '100vh',
      background: '#000000',
      color: '#ffffff',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
      paddingTop: '80px'
    }}>
      <SEO 
        title="Gaply Features - AI Content Detection, Thesis Writing Help & Research Support Tools"
        description="Explore Gaply's comprehensive academic research features including AI content detection, thesis writing help, journal matching, research paper editing, plagiarism checking, and statistical analysis support for PhD students and researchers."
        keywords="academic research features, AI content detection tools, thesis writing help, journal matching service, research paper editing, plagiarism checker, statistical analysis help, academic writing tools, research methodology guidance, literature review help, conference paper preparation, dissertation writing assistance, PhD thesis support, academic proofreading, Scopus journal finder"
      />
      {/* Header */}
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        zIndex: 1000,
        background: 'rgba(0, 0, 0, 0.8)',
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)',
        borderBottom: '1px solid rgba(255, 255, 255, 0.1)',
        padding: '20px 40px',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center'
      }}>
        <h1 style={{
          fontSize: '24px',
          fontWeight: '600',
          margin: 0,
          letterSpacing: '-0.02em'
        }}>
          Gaply
        </h1>
        
        <button
          onClick={() => navigate('/')}
          style={{
            background: 'rgba(255, 255, 255, 0.1)',
            color: '#ffffff',
            padding: '10px 20px',
            borderRadius: '12px',
            border: '1px solid rgba(255, 255, 255, 0.2)',
            cursor: 'pointer',
            fontSize: '14px',
            fontWeight: '500',
            transition: 'all 0.3s ease',
            backdropFilter: 'blur(10px)',
            WebkitBackdropFilter: 'blur(10px)'
          }}
          onMouseEnter={(e) => e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)'}
          onMouseLeave={(e) => e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)'}
        >
          ← Back to Home
        </button>
      </div>

      {/* Hero Section */}
      <div style={{
        padding: '120px 40px 80px',
        textAlign: 'center',
        maxWidth: '1200px',
        margin: '0 auto'
      }}>
        <h1 style={{
          fontSize: 'clamp(3rem, 8vw, 6rem)',
          fontWeight: '700',
          letterSpacing: '-0.02em',
          marginBottom: '30px',
          background: 'linear-gradient(135deg, #ffffff 0%, #a0a0a0 100%)',
          WebkitBackgroundClip: 'text',
          WebkitTextFillColor: 'transparent',
          backgroundClip: 'text'
        }}>
          Features
        </h1>
        
        <p style={{
          fontSize: 'clamp(1.2rem, 3vw, 1.8rem)',
          color: '#a0a0a0',
          marginBottom: '60px',
          lineHeight: '1.4',
          maxWidth: '800px',
          margin: '0 auto 60px'
        }}>
          Powerful tools designed to accelerate your academic journey from idea to publication
        </p>

        {/* Section Toggle */}
        <div style={{
          display: 'flex',
          justifyContent: 'center',
          marginBottom: '80px',
          background: 'rgba(255, 255, 255, 0.05)',
          borderRadius: '16px',
          padding: '8px',
          width: 'fit-content',
          margin: '0 auto 80px'
        }}>
          <button
            onClick={() => setActiveSection('free')}
            style={{
              padding: '16px 32px',
              borderRadius: '12px',
              border: 'none',
              background: activeSection === 'free' ? 'rgba(255, 255, 255, 0.15)' : 'transparent',
              color: '#ffffff',
              fontSize: '16px',
              fontWeight: '600',
              cursor: 'pointer',
              transition: 'all 0.3s ease'
            }}
          >
            Free Features
          </button>
          <button
            onClick={() => setActiveSection('premium')}
            style={{
              padding: '16px 32px',
              borderRadius: '12px',
              border: 'none',
              background: activeSection === 'premium' ? 'rgba(255, 255, 255, 0.15)' : 'transparent',
              color: '#ffffff',
              fontSize: '16px',
              fontWeight: '600',
              cursor: 'pointer',
              transition: 'all 0.3s ease'
            }}
          >
            Premium Features
          </button>
        </div>
      </div>

      {/* Features Grid */}
      <div style={{
        padding: '0 40px 120px',
        maxWidth: '1200px',
        margin: '0 auto'
      }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
          gap: '24px'
        }}>
          {(activeSection === 'free' ? freeFeatures : premiumFeatures).map((feature) => (
            <div
              key={feature.id}
              style={{
                background: 'rgba(255, 255, 255, 0.02)',
                borderRadius: '16px',
                padding: '24px',
                border: `1px solid ${feature.borderColor}`,
                transition: 'all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                cursor: 'pointer',
                position: 'relative',
                overflow: 'hidden'
              }}
              onMouseEnter={() => setHoveredFeature(feature.id)}
              onMouseLeave={() => setHoveredFeature(null)}
            >
              {/* Title */}
              <h3 style={{
                fontSize: '20px',
                fontWeight: '600',
                marginBottom: '8px',
                color: '#ffffff',
                letterSpacing: '-0.01em',
                lineHeight: '1.3'
              }}>
                {feature.title}
              </h3>

              {/* Subtitle */}
              <p style={{
                fontSize: '14px',
                color: '#a0a0a0',
                marginBottom: '16px',
                fontWeight: '400',
                lineHeight: '1.4'
              }}>
                {feature.subtitle}
              </p>

              {/* Description */}
              <p style={{
                fontSize: '13px',
                color: '#e0e0e0',
                marginBottom: '20px',
                lineHeight: '1.5'
              }}>
                {feature.description}
              </p>

              {/* Features List */}
              <div style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '8px'
              }}>
                {feature.features.slice(0, 3).map((item, index) => (
                  <div key={index} style={{
                    display: 'flex',
                    alignItems: 'center',
                    fontSize: '12px',
                    color: '#c0c0c0',
                    lineHeight: '1.4'
                  }}>
                    <div style={{
                      width: '4px',
                      height: '4px',
                      backgroundColor: '#007AFF',
                      borderRadius: '50%',
                      marginRight: '8px',
                      flexShrink: '0'
                    }} />
                    <span>{item}</span>
                  </div>
                ))}
                {feature.features.length > 3 && (
                  <div style={{
                    fontSize: '11px',
                    color: '#888',
                    marginTop: '4px',
                    fontStyle: 'italic'
                  }}>
                    +{feature.features.length - 3} more features
                  </div>
                )}
              </div>

              {/* Hover Effect */}
              <div style={{
                position: 'absolute',
                top: 0,
                left: 0,
                right: 0,
                bottom: 0,
                background: `linear-gradient(135deg, ${feature.color} 0%, transparent 100%)`,
                opacity: hoveredFeature === feature.id ? '1' : '0',
                transition: 'opacity 0.4s ease',
                pointerEvents: 'none',
                borderRadius: '16px'
              }} />
            </div>
          ))}
        </div>
      </div>

      {/* CTA Section */}
      <div style={{
        padding: '80px 40px',
        textAlign: 'center',
        background: 'linear-gradient(135deg, rgba(0, 122, 255, 0.1) 0%, rgba(0, 0, 0, 0.8) 100%)',
        borderTop: '1px solid rgba(255, 255, 255, 0.1)'
      }}>
        <h2 style={{
          fontSize: 'clamp(2rem, 5vw, 3rem)',
          fontWeight: '700',
          marginBottom: '20px',
          letterSpacing: '-0.02em'
        }}>
          Ready to accelerate your research?
        </h2>
        
        <p style={{
          fontSize: '1.2rem',
          color: '#a0a0a0',
          marginBottom: '40px',
          maxWidth: '600px',
          margin: '0 auto 40px'
        }}>
          Start with our free features or unlock the full potential with premium access
        </p>

        <div style={{
          display: 'flex',
          gap: '20px',
          justifyContent: 'center',
          flexWrap: 'wrap'
        }}>
          <button
            onClick={() => navigate('/')}
            style={{
              padding: '18px 36px',
              borderRadius: '16px',
              background: 'linear-gradient(135deg, #007AFF, #005bb5)',
              color: '#ffffff',
              fontSize: '16px',
              fontWeight: '600',
              border: 'none',
              cursor: 'pointer',
              transition: 'all 0.3s ease',
              boxShadow: '0 10px 20px rgba(0, 122, 255, 0.2)'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.transform = 'translateY(-2px)';
              e.currentTarget.style.boxShadow = '0 15px 25px rgba(0, 122, 255, 0.3)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.transform = 'translateY(0)';
              e.currentTarget.style.boxShadow = '0 10px 20px rgba(0, 122, 255, 0.2)';
            }}
          >
            Try Free Features
          </button>
          
          <button
            onClick={() => navigate('/premium')}
            style={{
              padding: '18px 36px',
              borderRadius: '16px',
              background: 'transparent',
              color: '#ffffff',
              fontSize: '16px',
              fontWeight: '600',
              border: '1px solid rgba(255, 255, 255, 0.3)',
              cursor: 'pointer',
              transition: 'all 0.3s ease'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
              e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.6)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
              e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.3)';
            }}
          >
            Explore Premium
          </button>
        </div>
      </div>
    </div>
  );
};

export default FeaturesPage;
