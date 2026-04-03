import React from 'react';
import { useNavigate } from 'react-router-dom';
import SEO from './SEO';

const PricingSection: React.FC = () => {
  const navigate = useNavigate();

  const plans: Array<{
    id: string;
    name: string;
    price: string;
    description: string;
    features: string[];
    buttonText: string;
    popular: boolean;
    color: string;
    ctaVariant: 'solid' | 'outline';
    ctaHref: string;
    note?: string;
  }> = [
    {
      id: "premium-pro",
      name: "Premium Pro",
      price: "₹7,999",
      description: "One-time payment. Lifetime credits for all Pro features.",
      features: [
        "1× PublishReady Pro — Full AI manuscript preparation",
        "1× DataMaestro Pro — Advanced data analysis & visualization",
        "5× Journal Verification Pro — Predatory journal detection",
        "1× Research Deep Analysis Pro — Comprehensive research breakdown",
        "Priority processing",
        "Downloadable reports",
        "Lifetime access to purchased credits"
      ],
      buttonText: "Get Premium Pro",
      popular: true,
      color: "var(--card-bg)",
      ctaVariant: "solid" as const,
      ctaHref: "/checkout/premium-pro",
      note: "Credits never expire. Use them anytime."
    },
    {
      id: "premium-4999",
      name: "Gaply Premium",
      price: "₹4,999",
      description: "PublishReady + DataMaestro bundled access with priority-quality outputs.",
      features: [
        "PublishReady (2 uses)",
        "DataMaestro (1 use)",
        "Unlimited chat with Gaply.AI in both features",
        "Priority evaluation queue",
        "Valid for 365 days"
      ],
      buttonText: "Get Gaply Premium",
      popular: false,
      color: "var(--card-bg)",
      ctaVariant: "solid" as const,
      ctaHref: "/login"
    },
    {
      id: "enterprise",
      name: "Gaply Enterprise",
      price: "Quotation basis",
      description: "Custom pricing and tailored onboarding for institutions, labs, and research teams.",
      features: [
        "Custom usage limits for PublishReady + DataMaestro",
        "Team access and admin controls",
        "Dedicated onboarding and support",
        "Flexible billing and invoicing",
        "Security and compliance review"
      ],
      buttonText: "Request a Quote",
      popular: false,
      color: "var(--card-bg)",
      ctaVariant: "outline" as const,
      ctaHref: "/contact"
    }
  ];

  const handlePlanClick = (planHref: string) => {
    navigate(planHref);
  };

  return (
    <div style={{
      minHeight: '100vh',
      backgroundColor: 'var(--app-bg)',
      padding: 'clamp(96px, 12vw, 140px) clamp(16px, 4vw, 40px)',
      color: 'var(--app-text)',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif'
    }}>
      <SEO 
        title="Gaply Pricing - Affordable Academic Research Plans | Thesis Writing & Journal Matching"
        description="Choose from Gaply's affordable academic research plans: Basic (₹471), Plus (₹1,061), Pro (₹2,241). Get AI content detection, thesis writing help, journal matching, and research support for PhD students and researchers."
        keywords="academic research pricing, thesis writing cost, journal matching price, AI content detection cost, research paper editing price, dissertation writing assistance pricing, PhD thesis support cost, academic proofreading price, plagiarism checker cost, statistical analysis help pricing, Scopus journal finder cost, research methodology guidance price, academic writing service cost, literature review help price, conference paper preparation cost, research proposal writing price, data analysis support cost, academic consultation pricing"
      />
      {/* Header */}
      <div style={{ textAlign: 'center', marginBottom: 'clamp(48px, 8vw, 110px)' }}>
        <h2 style={{
          fontSize: 'clamp(2.6rem, 6vw, 5rem)',
          fontWeight: '600',
          letterSpacing: '-0.02em',
          marginBottom: '24px',
          color: 'var(--app-text)',
          lineHeight: '1.1'
        }}>
          Choose Your Plan
        </h2>
        <p style={{
          fontSize: 'clamp(1.05rem, 2.2vw, 1.5rem)',
          color: 'var(--muted-text)',
          marginBottom: '32px',
          fontWeight: '400',
          lineHeight: '1.4'
        }}>
          Select the research plan that fits your needs
        </p>
        <div style={{
          width: '60px',
          height: '1px',
          backgroundColor: 'var(--divider)',
          margin: '0 auto',
          opacity: '0.3'
        }} />
      </div>

      {/* Plans Grid */}
      <div style={{
        maxWidth: '1400px',
        margin: '0 auto',
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
        gap: 'clamp(20px, 4vw, 40px)',
        alignItems: 'stretch'
      }}>
        {plans.map((plan) => (
          <div
            key={plan.id}
            style={{
              background: `linear-gradient(180deg, rgba(255, 255, 255, 0.06) 0%, rgba(255, 255, 255, 0) 55%), ${plan.color}`,
              borderRadius: '24px',
              padding: 'clamp(28px, 5vw, 48px) clamp(22px, 4vw, 40px)',
              border: plan.popular 
                ? '2px solid rgba(0, 122, 255, 0.3)' 
                : '1px solid var(--card-border)',
              cursor: 'pointer',
              transition: 'all 0.6s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
              position: 'relative',
              backdropFilter: 'blur(20px)',
              WebkitBackdropFilter: 'blur(20px)',
              transform: 'translateZ(0)',
              transformStyle: 'preserve-3d',
              boxShadow: plan.popular ? '0 30px 70px rgba(0, 122, 255, 0.18)' : 'var(--card-shadow)',
              display: 'flex',
              flexDirection: 'column'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.transform = 'translateZ(16px) scale(1.02)';
              e.currentTarget.style.borderColor = plan.popular 
                ? 'rgba(0, 122, 255, 0.6)' 
                : 'var(--card-border-hover)';
              e.currentTarget.style.boxShadow = plan.popular
                ? '0 40px 80px rgba(0, 122, 255, 0.2), 0 0 0 1px rgba(0, 122, 255, 0.4)'
                : 'var(--card-shadow-hover)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.transform = 'translateZ(0) scale(1)';
              e.currentTarget.style.borderColor = plan.popular 
                ? 'rgba(0, 122, 255, 0.3)' 
                : 'var(--card-border)';
              e.currentTarget.style.boxShadow = plan.popular ? '0 30px 70px rgba(0, 122, 255, 0.18)' : 'var(--card-shadow)';
            }}
            onClick={() => handlePlanClick(plan.ctaHref)}
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
              color: 'var(--app-text)',
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
                fontSize: plan.id === 'enterprise' ? 'clamp(2rem, 5vw, 2.8rem)' : 'clamp(2.4rem, 6vw, 3.5rem)',
                fontWeight: '600',
                color: plan.id === 'enterprise' ? 'var(--app-text)' : 'var(--accent-blue)',
                letterSpacing: '-0.01em',
                textShadow: plan.id === 'enterprise' ? 'none' : '0 6px 20px rgba(0, 122, 255, 0.25)',
                lineHeight: '1'
              }}>
                {plan.price}
              </span>
            </div>

            {/* Description */}
            <p style={{
              fontSize: '1rem',
              color: 'var(--muted-text)',
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
                  color: 'var(--section-text)'
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
            <button
              style={{
                width: '100%',
                background: plan.ctaVariant === 'outline'
                  ? 'var(--pricing-cta-outline-bg)'
                  : 'var(--pricing-cta-bg)',
                color: plan.ctaVariant === 'outline'
                  ? 'var(--pricing-cta-outline-text)'
                  : 'var(--pricing-cta-text)',
                fontWeight: '600',
                padding: '18px 32px',
                borderRadius: '16px',
                fontSize: '1.05rem',
                border: plan.ctaVariant === 'outline'
                  ? '1px solid var(--pricing-cta-outline-border)'
                  : '1px solid var(--pricing-cta-border)',
                cursor: 'pointer',
                transition: 'all 0.6s ease',
                textTransform: 'uppercase',
                letterSpacing: '0.04em',
                position: 'relative',
                overflow: 'hidden',
                transform: 'translateZ(0)',
                transformStyle: 'preserve-3d',
                boxShadow: plan.ctaVariant === 'outline'
                  ? 'none'
                  : '0 12px 30px rgba(15, 23, 42, 0.18)',
                marginTop: 'auto'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = plan.ctaVariant === 'outline'
                  ? 'var(--pricing-cta-outline-hover-bg)'
                  : 'var(--pricing-cta-bg-hover)';
                e.currentTarget.style.borderColor = plan.ctaVariant === 'outline'
                  ? 'var(--pricing-cta-outline-border)'
                  : 'var(--pricing-cta-border)';
                e.currentTarget.style.transform = 'translateZ(10px) scale(1.03)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = plan.ctaVariant === 'outline'
                  ? 'var(--pricing-cta-outline-bg)'
                  : 'var(--pricing-cta-bg)';
                e.currentTarget.style.borderColor = plan.ctaVariant === 'outline'
                  ? 'var(--pricing-cta-outline-border)'
                  : 'var(--pricing-cta-border)';
                e.currentTarget.style.transform = 'translateZ(0) scale(1)';
              }}
              onClick={(e) => {
                e.stopPropagation();
                handlePlanClick(plan.ctaHref);
              }}
            >
              {plan.buttonText}
            </button>
            {plan.note && (
              <p style={{
                marginTop: '16px',
                fontSize: '0.9rem',
                color: 'var(--muted-text)',
                textAlign: 'center'
              }}>
                {plan.note}
              </p>
            )}
          </div>
        ))}
      </div>

      {/* Expert Section */}
      <div style={{
        maxWidth: '800px',
        margin: '100px auto 0',
        textAlign: 'center',
        padding: 'clamp(36px, 6vw, 60px) clamp(20px, 4vw, 40px)',
        background: 'var(--card-bg)',
        borderRadius: '24px',
        border: '1px solid var(--card-border)',
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)'
      }}>
        <h3 style={{
          fontSize: 'clamp(1.8rem, 4vw, 2.5rem)',
          fontWeight: '600',
          color: 'var(--app-text)',
          marginBottom: '24px'
        }}>
          Need Expert Help?
        </h3>
        <p style={{
          fontSize: 'clamp(1rem, 2.5vw, 1.2rem)',
          color: 'var(--muted-text)',
          marginBottom: '32px',
          lineHeight: '1.6'
        }}>
          Hire our research experts for personalized guidance and support at competitive rates
        </p>
        <button
          onClick={() => navigate('/contact')}
          style={{
            background: 'var(--hero-cta-bg)',
            color: 'var(--hero-cta-text)',
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
            e.currentTarget.style.background = 'var(--hero-cta-bg-hover)';
            e.currentTarget.style.transform = 'translateZ(10px) scale(1.05)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'var(--hero-cta-bg)';
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
        borderTop: '1px solid var(--divider)'
      }}>
        <p style={{
          color: 'var(--muted-text)',
          fontSize: '0.9rem'
        }}>
          Need help choosing? Contact our support team
        </p>
      </div>
    </div>
  );
};

export default PricingSection;
