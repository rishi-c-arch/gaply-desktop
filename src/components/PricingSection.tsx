import React from 'react';
import { useNavigate } from 'react-router-dom';
import SEO from './SEO';

const PricingSection: React.FC = () => {
  const navigate = useNavigate();

  const plans = [
    {
      id: "basic",
      name: "Gaply Basic",
      price: "₹471",
      description: "Research Gap Finder - Analyze 5 base papers to identify research gaps and opportunities",
      features: [
        "Research Gap Finder (1 use)",
        "Analyze 5 base papers",
        "Publishable problem statement",
        "Clear objectives & hypotheses",
        "Valid for 365 days"
      ],
      buttonText: "Get Gaply Basic",
      popular: false,
      color: "rgba(255, 255, 255, 0.05)"
    },
    {
      id: "plus",
      name: "Gaply Plus",
      price: "₹1,061",
      description: "Research Pro - Gap Finder (2 uses) + Deep Paper Analysis (1 use) with 70+ parameter evaluation",
      features: [
        "Research Gap Finder (2 uses)",
        "Deep Paper Analysis (1 use)",
        "70+ parameter evaluation",
        "Journal compliance checking",
        "Valid for 365 days"
      ],
      buttonText: "Get Gaply Plus",
      popular: true,
      color: "rgba(0, 122, 255, 0.1)"
    },
    {
      id: "pro",
      name: "Gaply Pro",
      price: "₹2,241",
      description: "Research Elite - Full access (5 uses each) + Team support with 30min session + WhatsApp support",
      features: [
        "Research Gap Finder (5 uses)",
        "Deep Paper Analysis (5 uses)",
        "Team support (30min session)",
        "WhatsApp support",
        "Valid for 365 days"
      ],
      buttonText: "Get Gaply Pro",
      popular: false,
      color: "rgba(255, 255, 255, 0.05)"
    }
  ];

  const handlePlanClick = (planId: string) => {
    navigate('/login');
  };

  return (
    <div style={{
      minHeight: '100vh',
      backgroundColor: '#000000',
      padding: '120px 40px',
      color: 'white',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
    }}>
      <SEO 
        title="Gaply Pricing - Affordable Academic Research Plans | Thesis Writing & Journal Matching"
        description="Choose from Gaply's affordable academic research plans: Basic (₹471), Plus (₹1,061), Pro (₹2,241). Get AI content detection, thesis writing help, journal matching, and research support for PhD students and researchers."
        keywords="academic research pricing, thesis writing cost, journal matching price, AI content detection cost, research paper editing price, dissertation writing assistance pricing, PhD thesis support cost, academic proofreading price, plagiarism checker cost, statistical analysis help pricing, Scopus journal finder cost, research methodology guidance price, academic writing service cost, literature review help price, conference paper preparation cost, research proposal writing price, data analysis support cost, academic consultation pricing"
      />
      {/* Header */}
      <div style={{ textAlign: 'center', marginBottom: '100px' }}>
        <h2 style={{
          fontSize: '5rem',
          fontWeight: '300',
          letterSpacing: '-0.02em',
          marginBottom: '30px',
          color: '#ffffff',
          lineHeight: '1.1'
        }}>
          Choose Your Plan
        </h2>
        <p style={{
          fontSize: '1.5rem',
          color: 'rgba(255, 255, 255, 0.7)',
          marginBottom: '40px',
          fontWeight: '400',
          lineHeight: '1.4'
        }}>
          Select the research plan that fits your needs
        </p>
        <div style={{
          width: '60px',
          height: '1px',
          backgroundColor: '#ffffff',
          margin: '0 auto',
          opacity: '0.3'
        }} />
      </div>

      {/* Plans Grid */}
      <div style={{
        maxWidth: '1400px',
        margin: '0 auto',
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fit, minmax(350px, 1fr))',
        gap: '40px',
        alignItems: 'stretch'
      }}>
        {plans.map((plan, index) => (
          <div
            key={plan.id}
            style={{
              backgroundColor: plan.color,
              borderRadius: '24px',
              padding: '48px 40px',
              border: plan.popular 
                ? '2px solid rgba(0, 122, 255, 0.3)' 
                : '1px solid rgba(255, 255, 255, 0.1)',
              cursor: 'pointer',
              transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
              position: 'relative',
              backdropFilter: 'blur(20px)',
              WebkitBackdropFilter: 'blur(20px)',
              transform: 'translateZ(0)',
              transformStyle: 'preserve-3d'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.transform = 'translateZ(20px) scale(1.02)';
              e.currentTarget.style.borderColor = plan.popular 
                ? 'rgba(0, 122, 255, 0.6)' 
                : 'rgba(255, 255, 255, 0.3)';
              e.currentTarget.style.boxShadow = plan.popular
                ? '0 40px 80px rgba(0, 122, 255, 0.2), 0 0 0 1px rgba(0, 122, 255, 0.4)'
                : '0 40px 80px rgba(255, 255, 255, 0.1)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.transform = 'translateZ(0) scale(1)';
              e.currentTarget.style.borderColor = plan.popular 
                ? 'rgba(0, 122, 255, 0.3)' 
                : 'rgba(255, 255, 255, 0.1)';
              e.currentTarget.style.boxShadow = 'none';
            }}
            onClick={() => handlePlanClick(plan.id)}
          >
            {/* Popular Badge */}
            {plan.popular && (
              <div style={{
                position: 'absolute',
                top: '-12px',
                left: '50%',
                transform: 'translateX(-50%)',
                background: 'linear-gradient(135deg, #007AFF, #0056CC)',
                color: '#ffffff',
                padding: '8px 24px',
                borderRadius: '20px',
                fontSize: '14px',
                fontWeight: '600',
                boxShadow: '0 8px 20px rgba(0, 122, 255, 0.3)',
                zIndex: 10
              }}>
                Most Popular
              </div>
            )}

            {/* Plan Name */}
            <h3 style={{
              fontSize: '2rem',
              fontWeight: '700',
              color: '#ffffff',
              marginBottom: '16px',
              textAlign: 'center'
            }}>
              {plan.name}
            </h3>

            {/* Price */}
            <div style={{
              textAlign: 'center',
              marginBottom: '24px'
            }}>
              <span style={{
                fontSize: '3.5rem',
                fontWeight: '300',
                color: '#007AFF',
                lineHeight: '1'
              }}>
                {plan.price}
              </span>
            </div>

            {/* Description */}
            <p style={{
              fontSize: '1rem',
              color: 'rgba(255, 255, 255, 0.8)',
              marginBottom: '32px',
              lineHeight: '1.6',
              textAlign: 'center'
            }}>
              {plan.description}
            </p>

            {/* Features */}
            <div style={{
              marginBottom: '40px'
            }}>
              {plan.features.map((feature, featureIndex) => (
                <div key={featureIndex} style={{
                  display: 'flex',
                  alignItems: 'center',
                  marginBottom: '16px',
                  fontSize: '1rem',
                  color: '#e0e0e0'
                }}>
                  <div style={{
                    width: '20px',
                    height: '20px',
                    backgroundColor: '#30D158',
                    borderRadius: '50%',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    marginRight: '16px',
                    flexShrink: '0'
                  }}>
                    <svg width="14" height="14" fill="white" viewBox="0 0 20 20">
                      <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                    </svg>
                  </div>
                  <span style={{ fontWeight: '500' }}>{feature}</span>
                </div>
              ))}
            </div>

            {/* Button */}
            <button style={{
              width: '100%',
              background: plan.popular 
                ? 'linear-gradient(135deg, #007AFF, #0056CC)' 
                : 'rgba(255, 255, 255, 0.1)',
              color: '#ffffff',
              fontWeight: '600',
              padding: '18px 32px',
              borderRadius: '16px',
              fontSize: '1.1rem',
              border: plan.popular 
                ? 'none' 
                : '1px solid rgba(255, 255, 255, 0.3)',
              cursor: 'pointer',
              transition: 'all 0.6s ease',
              textTransform: 'uppercase',
              position: 'relative',
              overflow: 'hidden',
              transform: 'translateZ(0)',
              transformStyle: 'preserve-3d'
            }}
            onMouseEnter={(e) => {
              if (plan.popular) {
                e.currentTarget.style.background = 'linear-gradient(135deg, #0056CC, #003D99)';
                e.currentTarget.style.transform = 'translateZ(10px) scale(1.05)';
              } else {
                e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.2)';
                e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.6)';
                e.currentTarget.style.transform = 'translateZ(10px) scale(1.05)';
              }
            }}
            onMouseLeave={(e) => {
              if (plan.popular) {
                e.currentTarget.style.background = 'linear-gradient(135deg, #007AFF, #0056CC)';
                e.currentTarget.style.transform = 'translateZ(0) scale(1)';
              } else {
                e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.1)';
                e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.3)';
                e.currentTarget.style.transform = 'translateZ(0) scale(1)';
              }
            }}
            >
              {plan.buttonText}
            </button>
          </div>
        ))}
      </div>

      {/* Expert Section */}
      <div style={{
        maxWidth: '800px',
        margin: '100px auto 0',
        textAlign: 'center',
        padding: '60px 40px',
        background: 'rgba(255, 255, 255, 0.05)',
        borderRadius: '24px',
        border: '1px solid rgba(255, 255, 255, 0.1)',
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)'
      }}>
        <h3 style={{
          fontSize: '2.5rem',
          fontWeight: '600',
          color: '#ffffff',
          marginBottom: '24px'
        }}>
          Need Expert Help?
        </h3>
        <p style={{
          fontSize: '1.2rem',
          color: 'rgba(255, 255, 255, 0.8)',
          marginBottom: '32px',
          lineHeight: '1.6'
        }}>
          Hire our research experts for personalized guidance and support at competitive rates
        </p>
        <button
          onClick={() => navigate('/contact')}
          style={{
            background: 'linear-gradient(135deg, #FF9500, #FF6B00)',
            color: '#ffffff',
            fontWeight: '600',
            padding: '18px 40px',
            borderRadius: '16px',
            fontSize: '1.1rem',
            border: 'none',
            cursor: 'pointer',
            transition: 'all 0.6s ease',
            textTransform: 'uppercase',
            transform: 'translateZ(0)',
            transformStyle: 'preserve-3d'
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'linear-gradient(135deg, #FF6B00, #E55A00)';
            e.currentTarget.style.transform = 'translateZ(10px) scale(1.05)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'linear-gradient(135deg, #FF9500, #FF6B00)';
            e.currentTarget.style.transform = 'translateZ(0) scale(1)';
          }}
        >
          Hire Expert
        </button>
      </div>

      {/* Footer */}
      <div style={{
        textAlign: 'center',
        marginTop: '80px',
        paddingTop: '40px',
        borderTop: '1px solid rgba(255, 255, 255, 0.1)'
      }}>
        <p style={{
          color: 'rgba(255, 255, 255, 0.6)',
          fontSize: '1rem',
          marginBottom: '16px'
        }}>
          All plans include 30-day money-back guarantee
        </p>
        <p style={{
          color: 'rgba(255, 255, 255, 0.5)',
          fontSize: '0.9rem'
        }}>
          Need help choosing? Contact our support team
        </p>
      </div>
    </div>
  );
};

export default PricingSection;
