import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import SEO from './SEO';

const CareerPage: React.FC = () => {
  const navigate = useNavigate();
  const [hoveredCard, setHoveredCard] = useState<string | null>(null);

  const careerPositions = [
    {
      id: 'research-expert',
      title: 'Research Expert',
      subtitle: 'Comprehensive academic support specialist',
      description: 'Provide expert guidance across all aspects of academic research including peer review, editing, supervision, and consultation. Help researchers achieve publication success while earning competitive compensation.',
      requirements: [
        'PhD holder or equivalent research experience',
        'Minimum 3-4 published papers in reputed journals',
        'Strong analytical and critical thinking skills',
        'Experience in peer review processes',
        'Mastery in academic formatting standards (APA, MLA, IEEE)',
        'Exceptional grammar and writing skills',
        'Proficiency in statistical software (SPSS/R)',
        'Strong mentorship and communication skills'
      ],
      benefits: [
        'Recognition in academic community',
        'Flexible remote work opportunities',
        'Competitive compensation rates',
        'Access to latest research',
        'Diverse project portfolio',
        'Professional development opportunities',
        'Global collaboration opportunities',
        'Academic recognition and growth'
      ]
    }
  ];

  const coreValues = [
    {
      title: 'Academic Excellence',
      description: 'We maintain the highest standards of academic integrity and scholarly rigor.'
    },
    {
      title: 'Global Collaboration',
      description: 'Connect with researchers worldwide and contribute to international scholarship.'
    },
    {
      title: 'Flexible Opportunities',
      description: 'Work remotely on your schedule while maintaining work-life balance.'
    },
    {
      title: 'Professional Growth',
      description: 'Develop your expertise and gain recognition in the academic community.'
    }
  ];

  const handleApplyClick = () => {
    window.open('mailto:helloresearcher@gaply.in?subject=Career Application - Gaply.in&body=Dear Gaply Team,%0D%0A%0D%0AI am interested in joining your expert research team. Please find my application details below:%0D%0A%0D%0APosition of Interest:%0D%0AQualifications:%0D%0AExperience:%0D%0A%0D%0AThank you for your consideration.%0D%0A%0D%0ABest regards,', '_blank');
  };

  return (
    <div style={{
      minHeight: '100vh',
      background: '#000000',
      color: '#ffffff',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
      overflowX: 'hidden'
    }}>
      <SEO 
        title="Career at Gaply - Join Our Academic Research Platform | Research Expert Positions"
        description="Join Gaply's academic research platform as a Research Expert. Work with PhD students and researchers worldwide on thesis writing, journal matching, research paper editing, and statistical analysis. Apply now!"
        keywords="career academic research platform, research expert jobs, thesis writing jobs, journal matching careers, research paper editing jobs, AI content detection careers, dissertation writing jobs, PhD thesis support careers, academic proofreading jobs, plagiarism checker careers, statistical analysis jobs, Scopus journal finder careers, research methodology jobs, academic writing careers, literature review jobs, conference paper preparation careers, research proposal writing jobs, data analysis careers, academic consultation jobs"
      />
      {/* Navigation Bar */}
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
        padding: '12px 0'
      }}>
        <div style={{
          maxWidth: 1200,
          margin: '0 auto',
          padding: '0 24px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between'
        }}>
          <div style={{ fontWeight: 800, letterSpacing: '.02em' }}>
            <h1 style={{ 
              margin: 0, 
              fontSize: 18, 
              color: '#ffffff',
              fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
            }}>
              Gaply
            </h1>
          </div>
          <button
            onClick={() => navigate('/')}
            style={{
              background: 'rgba(255, 255, 255, 0.1)',
              color: '#ffffff',
              padding: '10px 20px',
              borderRadius: '12px',
              border: '1px solid rgba(255, 255, 255, 0.2)',
              cursor: 'pointer',
              fontSize: '1rem',
              fontWeight: '500',
              transition: 'all 0.3s ease',
              backdropFilter: 'blur(10px)',
              WebkitBackdropFilter: 'blur(10px)'
            }}
            onMouseEnter={(e) => e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)'}
            onMouseLeave={(e) => e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)'}
          >
            Back to Home
          </button>
        </div>
      </div>

      {/* Hero Section */}
      <div style={{
        paddingTop: '120px',
        paddingBottom: '80px',
        textAlign: 'center',
        background: 'linear-gradient(135deg, #000000 0%, #1a1a1a 100%)'
      }}>
        <div style={{ maxWidth: '1000px', margin: '0 auto', padding: '0 40px' }}>
          <h1 style={{
            fontSize: 'clamp(3rem, 6vw, 5rem)',
            fontWeight: '700',
            color: '#ffffff',
            marginBottom: '24px',
            letterSpacing: '-0.02em',
            lineHeight: '1.1'
          }}>
            Join Our Expert Research Team
          </h1>
          <p style={{
            fontSize: 'clamp(1.2rem, 2.5vw, 1.8rem)',
            color: 'rgba(255, 255, 255, 0.8)',
            marginBottom: '40px',
            lineHeight: '1.4',
            fontWeight: '300'
          }}>
            Connect brilliant minds with emerging scholars to shape the next generation of impactful research
          </p>
          <div style={{
            width: '80px',
            height: '2px',
            backgroundColor: '#ffffff',
            margin: '0 auto',
            opacity: '0.3'
          }} />
        </div>
      </div>

      {/* Core Values Section */}
      <div style={{
        padding: '80px 40px',
        background: '#000000'
      }}>
        <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
          <h2 style={{
            fontSize: 'clamp(2.5rem, 4vw, 3.5rem)',
            fontWeight: '600',
            color: '#ffffff',
            textAlign: 'center',
            marginBottom: '60px',
            letterSpacing: '-0.02em'
          }}>
            Why Join Gaply?
          </h2>
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
            gap: '40px',
            marginBottom: '80px'
          }}>
            {coreValues.map((value, index) => (
              <div
                key={index}
                style={{
                  background: 'rgba(255, 255, 255, 0.05)',
                  borderRadius: '16px',
                  padding: '24px',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                  textAlign: 'center'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.08)';
                  e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                  e.currentTarget.style.transform = 'translateY(-8px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
                  e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                <div style={{
                  fontSize: '2rem',
                  marginBottom: '20px',
                  opacity: '0.9'
                }}>
                </div>
                <h3 style={{
                  fontSize: '1.3rem',
                  fontWeight: '600',
                  color: '#ffffff',
                  marginBottom: '12px',
                  letterSpacing: '-0.01em'
                }}>
                  {value.title}
                </h3>
                <p style={{
                  fontSize: '0.9rem',
                  color: 'rgba(255, 255, 255, 0.7)',
                  lineHeight: '1.5',
                  margin: 0
                }}>
                  {value.description}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Positions Section */}
      <div style={{
        padding: '80px 40px',
        background: 'linear-gradient(135deg, #1a1a1a 0%, #000000 100%)'
      }}>
        <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
          <h2 style={{
            fontSize: 'clamp(2.5rem, 4vw, 3.5rem)',
            fontWeight: '600',
            color: '#ffffff',
            textAlign: 'center',
            marginBottom: '60px',
            letterSpacing: '-0.02em'
          }}>
            Available Position
          </h2>
          <div style={{
            display: 'flex',
            justifyContent: 'center',
            marginBottom: '80px'
          }}>
            {careerPositions.map((position, index) => (
              <div
                key={position.id}
                style={{
                  background: 'rgba(255, 255, 255, 0.05)',
                  borderRadius: '20px',
                  padding: '32px',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                  position: 'relative',
                  overflow: 'hidden',
                  maxWidth: '600px',
                  width: '100%'
                }}
                onMouseEnter={() => setHoveredCard(position.id)}
                onMouseLeave={() => setHoveredCard(null)}
              >

                {/* Position Title */}
                <h3 style={{
                  fontSize: '1.6rem',
                  fontWeight: '700',
                  color: '#ffffff',
                  marginBottom: '8px',
                  letterSpacing: '-0.01em'
                }}>
                  {position.title}
                </h3>

                {/* Position Subtitle */}
                <p style={{
                  fontSize: '1rem',
                  color: 'rgba(255, 255, 255, 0.8)',
                  marginBottom: '16px',
                  fontWeight: '500'
                }}>
                  {position.subtitle}
                </p>

                {/* Description */}
                <p style={{
                  fontSize: '0.9rem',
                  color: 'rgba(255, 255, 255, 0.7)',
                  lineHeight: '1.5',
                  marginBottom: '24px'
                }}>
                  {position.description}
                </p>

                {/* Requirements */}
                <div style={{ marginBottom: '20px' }}>
                  <h4 style={{
                    fontSize: '0.9rem',
                    fontWeight: '600',
                    color: '#ffffff',
                    marginBottom: '8px',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em',
                    opacity: '0.9'
                  }}>
                    Requirements
                  </h4>
                  <ul style={{
                    listStyle: 'none',
                    padding: 0,
                    margin: 0
                  }}>
                    {position.requirements.map((req, reqIndex) => (
                      <li key={reqIndex} style={{
                        fontSize: '0.8rem',
                        color: 'rgba(255, 255, 255, 0.7)',
                        marginBottom: '4px',
                        paddingLeft: '16px',
                        position: 'relative'
                      }}>
                        <span style={{
                          position: 'absolute',
                          left: 0,
                          top: '4px',
                          width: '4px',
                          height: '4px',
                          backgroundColor: '#007AFF',
                          borderRadius: '50%'
                        }} />
                        {req}
                      </li>
                    ))}
                  </ul>
                </div>

                {/* Benefits */}
                <div>
                  <h4 style={{
                    fontSize: '0.9rem',
                    fontWeight: '600',
                    color: '#ffffff',
                    marginBottom: '8px',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em',
                    opacity: '0.9'
                  }}>
                    Benefits
                  </h4>
                  <ul style={{
                    listStyle: 'none',
                    padding: 0,
                    margin: 0
                  }}>
                    {position.benefits.map((benefit, benefitIndex) => (
                      <li key={benefitIndex} style={{
                        fontSize: '0.8rem',
                        color: 'rgba(255, 255, 255, 0.7)',
                        marginBottom: '4px',
                        paddingLeft: '16px',
                        position: 'relative'
                      }}>
                        <span style={{
                          position: 'absolute',
                          left: 0,
                          top: '4px',
                          width: '4px',
                          height: '4px',
                          backgroundColor: '#34C759',
                          borderRadius: '50%'
                        }} />
                        {benefit}
                      </li>
                    ))}
                  </ul>
                </div>

                {/* Hover Effect */}
                <div style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  right: 0,
                  bottom: 0,
                  background: 'linear-gradient(135deg, rgba(0, 122, 255, 0.05) 0%, rgba(52, 199, 89, 0.05) 100%)',
                  opacity: hoveredCard === position.id ? '1' : '0',
                  transition: 'opacity 0.6s ease',
                  pointerEvents: 'none',
                  borderRadius: '20px'
                }} />
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Application Section */}
      <div style={{
        padding: '80px 40px',
        background: '#000000',
        textAlign: 'center'
      }}>
        <div style={{ maxWidth: '800px', margin: '0 auto' }}>
          <h2 style={{
            fontSize: 'clamp(2.5rem, 4vw, 3.5rem)',
            fontWeight: '600',
            color: '#ffffff',
            marginBottom: '24px',
            letterSpacing: '-0.02em'
          }}>
            Ready to Join Our Platform?
          </h2>
          <p style={{
            fontSize: '1.2rem',
            color: 'rgba(255, 255, 255, 0.8)',
            marginBottom: '40px',
            lineHeight: '1.6'
          }}>
            Send us your application and let's discuss how you can contribute to helping researchers achieve publication success while earning competitive compensation.
          </p>
          <button
            onClick={handleApplyClick}
            style={{
              background: 'linear-gradient(135deg, #007AFF, #005bb5)',
              color: '#ffffff',
              padding: '20px 40px',
              borderRadius: '16px',
              border: 'none',
              fontSize: '1.2rem',
              fontWeight: '600',
              cursor: 'pointer',
              transition: 'all 0.3s ease',
              boxShadow: '0 10px 20px rgba(0, 122, 255, 0.2)',
              fontFamily: 'inherit'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.boxShadow = '0 15px 25px rgba(0, 122, 255, 0.3)';
              e.currentTarget.style.transform = 'translateY(-2px)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.boxShadow = '0 10px 20px rgba(0, 122, 255, 0.2)';
              e.currentTarget.style.transform = 'translateY(0)';
            }}
          >
            Apply Now
          </button>
          <p style={{
            fontSize: '1rem',
            color: 'rgba(255, 255, 255, 0.6)',
            marginTop: '24px',
            fontStyle: 'italic'
          }}>
            Email us at: <strong>helloresearcher@gaply.in</strong>
          </p>
        </div>
      </div>

      {/* Footer */}
      <div style={{
        padding: '60px 40px',
        background: 'rgba(255, 255, 255, 0.02)',
        borderTop: '1px solid rgba(255, 255, 255, 0.1)',
        textAlign: 'center'
      }}>
        <p style={{
          color: 'rgba(255, 255, 255, 0.5)',
          fontSize: '1rem',
          margin: 0
        }}>
          © 2025 Gaply. All rights reserved.
        </p>
      </div>
    </div>
  );
};

export default CareerPage;
