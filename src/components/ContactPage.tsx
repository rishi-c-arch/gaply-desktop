import React, { useState } from 'react';
import SEO from './SEO';
import { useNavigate } from 'react-router-dom';

const ContactPage: React.FC = () => {
  const navigate = useNavigate();
  const [formData, setFormData] = useState({
    name: '',
    email: '',
    subject: '',
    message: ''
  });
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitStatus, setSubmitStatus] = useState<'idle' | 'success' | 'error'>('idle');

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    const { name, value } = e.target;
    setFormData(prev => ({
      ...prev,
      [name]: value
    }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSubmitting(true);
    
    // Simulate form submission
    setTimeout(() => {
      setIsSubmitting(false);
      setSubmitStatus('success');
      setFormData({ name: '', email: '', subject: '', message: '' });
      
      // Reset status after 3 seconds
      setTimeout(() => setSubmitStatus('idle'), 3000);
    }, 1500);
  };

  const contactMethods = [
    {
      title: 'Email Support',
      description: 'Get direct help from our research experts',
      contact: 'helloresearcher@gaply.in',
      action: 'Send Email',
      href: 'mailto:helloresearcher@gaply.in',
      color: 'rgba(0, 122, 255, 0.1)',
      borderColor: 'rgba(0, 122, 255, 0.3)'
    },
    {
      title: 'Instagram',
      description: 'Follow us for updates and research insights',
      contact: '@gaply.in_',
      action: 'Follow Us',
      href: 'https://www.instagram.com/gaply.in_?igsh=MW01bXdmMG04YjN4bw%3D%3D&utm_source=qr',
      color: 'rgba(255, 45, 85, 0.1)',
      borderColor: 'rgba(255, 45, 85, 0.3)'
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
        title="Contact Gaply - Academic Research Support & Expert Consultation"
        description="Get in touch with Gaply's academic research experts. Contact us for thesis writing help, journal matching, research paper editing, AI content detection, and statistical analysis support. Email: helloresearcher@gaply.in"
        keywords="contact academic research support, thesis writing help contact, journal matching consultation, research paper editing contact, AI content detection support, dissertation writing assistance contact, PhD thesis support consultation, academic proofreading contact, plagiarism checker support, statistical analysis help contact, Scopus journal finder consultation, research methodology guidance contact, academic writing service support, literature review help contact, conference paper preparation consultation, research proposal writing contact, data analysis support consultation, academic consultation contact"
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
          Contact Us
        </h1>
        
        <p style={{
          fontSize: 'clamp(1.2rem, 3vw, 1.8rem)',
          color: '#a0a0a0',
          marginBottom: '60px',
          lineHeight: '1.4',
          maxWidth: '800px',
          margin: '0 auto 60px'
        }}>
          Get in touch with our research experts. We're here to help accelerate your academic journey.
        </p>
      </div>

      {/* Contact Methods */}
      <div style={{
        padding: '0 40px 80px',
        maxWidth: '1200px',
        margin: '0 auto'
      }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
          gap: '24px',
          marginBottom: '80px'
        }}>
          {contactMethods.map((method, index) => (
            <div
              key={index}
              style={{
                background: 'rgba(255, 255, 255, 0.02)',
                borderRadius: '16px',
                padding: '32px',
                border: `1px solid ${method.borderColor}`,
                transition: 'all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                cursor: 'pointer',
                position: 'relative',
                overflow: 'hidden'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.transform = 'translateY(-4px)';
                e.currentTarget.style.borderColor = method.borderColor.replace('0.3', '0.6');
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.transform = 'translateY(0)';
                e.currentTarget.style.borderColor = method.borderColor;
              }}
            >
              <h3 style={{
                fontSize: '20px',
                fontWeight: '600',
                marginBottom: '12px',
                color: '#ffffff',
                letterSpacing: '-0.01em'
              }}>
                {method.title}
              </h3>

              <p style={{
                fontSize: '14px',
                color: '#a0a0a0',
                marginBottom: '20px',
                lineHeight: '1.4'
              }}>
                {method.description}
              </p>

              <div style={{
                fontSize: '16px',
                color: '#e0e0e0',
                marginBottom: '24px',
                fontWeight: '500'
              }}>
                {method.contact}
              </div>

              <a
                href={method.href}
                target={method.href.startsWith('mailto:') ? '_self' : '_blank'}
                rel={method.href.startsWith('mailto:') ? '' : 'noopener noreferrer'}
                style={{
                  display: 'inline-block',
                  padding: '12px 24px',
                  borderRadius: '12px',
                  background: 'rgba(255, 255, 255, 0.1)',
                  color: '#ffffff',
                  textDecoration: 'none',
                  fontSize: '14px',
                  fontWeight: '600',
                  transition: 'all 0.3s ease',
                  border: '1px solid rgba(255, 255, 255, 0.2)'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)';
                  e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.4)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                  e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                }}
              >
                {method.action}
              </a>

              {/* Hover Effect */}
              <div style={{
                position: 'absolute',
                top: 0,
                left: 0,
                right: 0,
                bottom: 0,
                background: `linear-gradient(135deg, ${method.color} 0%, transparent 100%)`,
                opacity: '0',
                transition: 'opacity 0.4s ease',
                pointerEvents: 'none',
                borderRadius: '16px'
              }} />
            </div>
          ))}
        </div>
      </div>

      {/* Contact Form */}
      <div style={{
        padding: '0 40px 120px',
        maxWidth: '800px',
        margin: '0 auto'
      }}>
        <div style={{
          background: 'rgba(255, 255, 255, 0.02)',
          borderRadius: '24px',
          padding: '48px',
          border: '1px solid rgba(255, 255, 255, 0.1)'
        }}>
          <h2 style={{
            fontSize: '32px',
            fontWeight: '700',
            marginBottom: '16px',
            color: '#ffffff',
            textAlign: 'center',
            letterSpacing: '-0.02em'
          }}>
            Send us a Message
          </h2>
          
          <p style={{
            fontSize: '16px',
            color: '#a0a0a0',
            marginBottom: '40px',
            textAlign: 'center',
            lineHeight: '1.5'
          }}>
            Have a question or need assistance? Fill out the form below and we'll get back to you within 24 hours.
          </p>

          <form onSubmit={handleSubmit}>
            <div style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
              gap: '24px',
              marginBottom: '24px'
            }}>
              <div>
                <label htmlFor="name" style={{
                  display: 'block',
                  fontSize: '14px',
                  fontWeight: '600',
                  color: '#e0e0e0',
                  marginBottom: '8px'
                }}>
                  Name
                </label>
                <input
                  type="text"
                  id="name"
                  name="name"
                  value={formData.name}
                  onChange={handleInputChange}
                  required
                  style={{
                    width: '100%',
                    padding: '16px',
                    borderRadius: '12px',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    backgroundColor: 'rgba(255, 255, 255, 0.03)',
                    color: '#ffffff',
                    fontSize: '16px',
                    fontFamily: 'inherit',
                    outline: 'none',
                    transition: 'all 0.3s ease',
                    boxSizing: 'border-box'
                  }}
                  onFocus={(e) => e.currentTarget.style.borderColor = 'rgba(0, 122, 255, 0.5)'}
                  onBlur={(e) => e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)'}
                />
              </div>

              <div>
                <label htmlFor="email" style={{
                  display: 'block',
                  fontSize: '14px',
                  fontWeight: '600',
                  color: '#e0e0e0',
                  marginBottom: '8px'
                }}>
                  Email
                </label>
                <input
                  type="email"
                  id="email"
                  name="email"
                  value={formData.email}
                  onChange={handleInputChange}
                  required
                  style={{
                    width: '100%',
                    padding: '16px',
                    borderRadius: '12px',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    backgroundColor: 'rgba(255, 255, 255, 0.03)',
                    color: '#ffffff',
                    fontSize: '16px',
                    fontFamily: 'inherit',
                    outline: 'none',
                    transition: 'all 0.3s ease',
                    boxSizing: 'border-box'
                  }}
                  onFocus={(e) => e.currentTarget.style.borderColor = 'rgba(0, 122, 255, 0.5)'}
                  onBlur={(e) => e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)'}
                />
              </div>
            </div>

            <div style={{ marginBottom: '24px' }}>
              <label htmlFor="subject" style={{
                display: 'block',
                fontSize: '14px',
                fontWeight: '600',
                color: '#e0e0e0',
                marginBottom: '8px'
              }}>
                Subject
              </label>
              <input
                type="text"
                id="subject"
                name="subject"
                value={formData.subject}
                onChange={handleInputChange}
                required
                style={{
                  width: '100%',
                  padding: '16px',
                  borderRadius: '12px',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  backgroundColor: 'rgba(255, 255, 255, 0.03)',
                  color: '#ffffff',
                  fontSize: '16px',
                  fontFamily: 'inherit',
                  outline: 'none',
                  transition: 'all 0.3s ease',
                  boxSizing: 'border-box'
                }}
                onFocus={(e) => e.currentTarget.style.borderColor = 'rgba(0, 122, 255, 0.5)'}
                onBlur={(e) => e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)'}
              />
            </div>

            <div style={{ marginBottom: '32px' }}>
              <label htmlFor="message" style={{
                display: 'block',
                fontSize: '14px',
                fontWeight: '600',
                color: '#e0e0e0',
                marginBottom: '8px'
              }}>
                Message
              </label>
              <textarea
                id="message"
                name="message"
                value={formData.message}
                onChange={handleInputChange}
                required
                rows={6}
                style={{
                  width: '100%',
                  padding: '16px',
                  borderRadius: '12px',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  backgroundColor: 'rgba(255, 255, 255, 0.03)',
                  color: '#ffffff',
                  fontSize: '16px',
                  fontFamily: 'inherit',
                  outline: 'none',
                  transition: 'all 0.3s ease',
                  resize: 'vertical',
                  boxSizing: 'border-box'
                }}
                onFocus={(e) => e.currentTarget.style.borderColor = 'rgba(0, 122, 255, 0.5)'}
                onBlur={(e) => e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)'}
              />
            </div>

            <button
              type="submit"
              disabled={isSubmitting}
              style={{
                width: '100%',
                padding: '18px 32px',
                borderRadius: '16px',
                background: isSubmitting 
                  ? 'linear-gradient(135deg, #4a4a4a, #3a3a3a)' 
                  : 'linear-gradient(135deg, #007AFF, #005bb5)',
                color: '#ffffff',
                fontSize: '16px',
                fontWeight: '600',
                border: 'none',
                cursor: isSubmitting ? 'not-allowed' : 'pointer',
                transition: 'all 0.3s ease',
                boxShadow: isSubmitting ? 'none' : '0 10px 20px rgba(0, 122, 255, 0.2)',
                fontFamily: 'inherit'
              }}
              onMouseEnter={(e) => {
                if (!isSubmitting) {
                  e.currentTarget.style.boxShadow = '0 15px 25px rgba(0, 122, 255, 0.3)';
                  e.currentTarget.style.transform = 'translateY(-2px)';
                }
              }}
              onMouseLeave={(e) => {
                if (!isSubmitting) {
                  e.currentTarget.style.boxShadow = '0 10px 20px rgba(0, 122, 255, 0.2)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }
              }}
            >
              {isSubmitting ? 'Sending...' : 'Send Message'}
            </button>

            {submitStatus === 'success' && (
              <div style={{
                marginTop: '20px',
                padding: '16px',
                borderRadius: '12px',
                background: 'rgba(52, 199, 89, 0.1)',
                border: '1px solid rgba(52, 199, 89, 0.3)',
                color: '#34C759',
                textAlign: 'center',
                fontSize: '14px',
                fontWeight: '500'
              }}>
                ✓ Message sent successfully! We'll get back to you soon.
              </div>
            )}
          </form>
        </div>
      </div>
    </div>
  );
};

export default ContactPage;
