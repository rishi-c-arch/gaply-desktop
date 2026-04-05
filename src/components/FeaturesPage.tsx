import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../contexts/AuthContext';
import SEO from './SEO';

const FEATURES_PAGE_URL = 'https://www.gaply.in/features';

const FeaturesPage: React.FC = () => {
  const navigate = useNavigate();
  const { isAuthenticated, isLoading } = useAuth();
  const [activeSection, setActiveSection] = useState<'free' | 'premium'>('free');
  const [hoveredFeature, setHoveredFeature] = useState<string | null>(null);

  /** Premium dashboard routes: send guests straight to login/signup with return URL (no silent bounce). */
  const openPremiumPath = (path: string) => {
    if (!isLoading && !isAuthenticated) {
      navigate(`/login?redirect=${encodeURIComponent(path)}`);
      return;
    }
    navigate(path);
  };

  // Free Features Data
  const freeFeatures = [
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
    },
    {
      id: 'citation-generator',
      title: 'Citation Generator',
      subtitle: 'APA, Vancouver & Harvard · DOI, ISBN & PubMed',
      description:
        'APA, Vancouver & Harvard in your browser. DOI, ISBN, PubMed lookup, BibTeX / RIS / JSON converter—saved lists stay on your device.',
      features: [
        'APA, Vancouver, and Harvard reference styles in your browser',
        'Metadata lookup by DOI, ISBN, or PubMed ID',
        'Convert and export BibTeX, RIS, CSL-JSON, and related formats',
        'Saved citation lists remain on your device (privacy-first)',
        'Free core workflows without a paywall for standard use'
      ],
      color: 'rgba(10, 132, 255, 0.1)',
      borderColor: 'rgba(10, 132, 255, 0.3)'
    },
    {
      id: 'conference-finder',
      title: 'Conference Finder',
      subtitle: 'Research conferences in India by field and city',
      description:
        'Research conferences in India—upcoming and recent events across STEM, medicine, social sciences, and more. Filter by field, search by city or name. Always verify dates on the organizer\'s site.',
      features: [
        'Curated academic and research conferences held in India',
        'STEM, medicine, social sciences, humanities, law, and multidisciplinary tracks',
        'Filter by subject area and search by city or conference name',
        'Upcoming and recent events with emphasis on official sources',
        "Verify every date, venue, and CFP on the organizer's website"
      ],
      color: 'rgba(175, 82, 222, 0.1)',
      borderColor: 'rgba(175, 82, 222, 0.3)'
    }
  ];

  // Premium Features Data
  const premiumFeatures = [
    {
      id: 'journal-verification',
      title: 'Journal Verification',
      subtitle: 'Know if your target journal is real or risky before you submit',
      description:
        'Verify the journal you are targeting for publication against legitimacy signals—indexing claims, publisher patterns, and predatory red flags—so you can decide with confidence (always confirm on official sites).',
      features: [
        'Structured checks to distinguish credible journals from misleading or predatory offers',
        'Ideal before paying APCs, agreeing to fast-track claims, or committing your manuscript',
        'Built for funded labs, unfunded scholars, and individual PhD researchers alike',
        'Chat with Gaply to interpret results and plan your next verification step',
        'Optional audio overview of key checks (English and Hindi)',
        'Complements Gaply journal matching: verify the exact title you intend to submit to',
        'Export-friendly notes you can share with supervisors or committees'
      ],
      color: 'rgba(255, 179, 64, 0.12)',
      borderColor: 'rgba(255, 179, 64, 0.35)'
    },
    {
      id: 'research-deep-analysis',
      title: 'Research Deep Analysis',
      subtitle: 'Research gaps, problem statements, and viable directions',
      description:
        'Surface research gaps you can pursue as a funded, unfunded, or solo researcher—with a clear problem statement, articulated gap, plausible working title, and methodology direction you can refine with Gaply.',
      features: [
        'Gap-led framing for non-funded, funded, and independent researchers',
        'Structured outputs: problem statement, research gap, and expected contribution',
        'Suggested working title and methodology direction aligned to your domain',
        'Chat with Gaply to stress-test scope, novelty, and feasibility',
        'Audio story walkthrough of the analysis (English and Hindi)',
        'Use early in a thesis, grant, or paper planning cycle',
        'Iterative refinement: tighten claims before you invest in data collection'
      ],
      color: 'rgba(94, 92, 230, 0.12)',
      borderColor: 'rgba(94, 92, 230, 0.35)'
    },
    {
      id: 'publish-ready',
      title: 'PublishReady',
      subtitle: 'Refine. Optimize. Succeed.',
      description:
        'The definitive algorithmic companion for manuscript perfection: align your draft with your target journal, see how publication-ready you are, and fix issues with prioritized guidance—plus Chat with Gaply and audio summaries in English and Hindi.',
      features: [
        'Manuscript vs journal expectations: structure, limits, figures, tables, and references',
        'Signals for publication fit and what to fix first (guidelines + integrity checks)',
        'Similarity and AI-use insights with transparent, reviewer-friendly context',
        'Chat with Gaply for clarifications, rewrites, and submission strategy',
        'Audio story of your evaluation report (English and Hindi)',
        'Annotated outputs you can share with co-authors or mentors',
        'Three-step flow: pick journal, upload manuscript, review and refine'
      ],
      color: 'rgba(90, 200, 250, 0.1)',
      borderColor: 'rgba(90, 200, 250, 0.3)'
    },
    {
      id: 'data-maestro',
      title: 'DataMaestro',
      subtitle: 'Domain-correct analysis—your tests or Gaply-recommended',
      description:
        'Run rigorous statistical analysis across domains: supply your title, hypotheses, objectives, methodology, and field—then choose analyses manually or let Gaply recommend appropriate tests. Chapter-style results help supervisors verify that student research is analytically sound.',
      features: [
        'Upload tabular or document data; describe design, variables, and hypotheses',
        'Manual test selection or Gaply-suggested analyses matched to your research story',
        'Publication-style Methods, Results, interpretation, tables, figures, and LaTeX',
        'Plain-language explanations so mentors can sanity-check conclusions',
        'Chat with Gaply tied to tables, plots, and methodological choices',
        'One-click re-run after cleaning data or revising hypotheses',
        'Honest guardrails: guidance is strong; final scientific judgment stays with you'
      ],
      color: 'rgba(255, 45, 85, 0.1)',
      borderColor: 'rgba(255, 45, 85, 0.3)'
    }
  ];

  function featureListItemUrl(id: string): string {
    switch (id) {
      case 'paper-search':
        return 'https://www.gaply.in/paper-search';
      case 'journal-matching':
        return 'https://www.gaply.in/journal-matching';
      case 'citation-generator':
        return 'https://www.gaply.in/citation-generator';
      case 'conference-finder':
        return 'https://www.gaply.in/conferences-india';
      case 'journal-verification':
        return 'https://www.gaply.in/dashboard/journal-verify';
      case 'research-deep-analysis':
        return 'https://www.gaply.in/dashboard/research-deep-analysis';
      case 'publish-ready':
        return 'https://www.gaply.in/dashboard/publishready';
      case 'data-maestro':
        return 'https://www.gaply.in/dashboard/datamaestro';
      default:
        return FEATURES_PAGE_URL;
    }
  }

  const featuresPageStructuredData = {
    '@context': 'https://schema.org',
    '@graph': [
      {
        '@type': 'WebPage',
        '@id': `${FEATURES_PAGE_URL}#webpage`,
        url: FEATURES_PAGE_URL,
        name: 'Gaply Features — Academic research tools',
        description:
          'Gaply free and premium tools: paper search, journal matching, citation generator, India conferences, journal verification, research gap analysis, PublishReady manuscript readiness, and DataMaestro statistical analysis—with Chat with Gaply and audio in English and Hindi on premium flows.'
      },
      {
        '@type': 'ItemList',
        name: 'Gaply free academic research features',
        description:
          'Free tools for researchers: paper search, journal matching, citation generator (APA, Vancouver, Harvard), and India conference discovery.',
        numberOfItems: freeFeatures.length,
        itemListElement: freeFeatures.map((f, i) => ({
          '@type': 'ListItem',
          position: i + 1,
          item: {
            '@type': 'WebApplication',
            name: f.title,
            description: `${f.subtitle} ${f.description}`,
            url: featureListItemUrl(f.id)
          }
        }))
      },
      {
        '@type': 'ItemList',
        name: 'Gaply premium academic research features',
        description:
          'Premium verification, research gap discovery, PublishReady journal-fit evaluation, and DataMaestro analysis—with conversational and audio support.',
        numberOfItems: premiumFeatures.length,
        itemListElement: premiumFeatures.map((f, i) => ({
          '@type': 'ListItem',
          position: i + 1,
          item: {
            '@type': 'WebApplication',
            name: f.title,
            description: `${f.subtitle} ${f.description}`,
            url: featureListItemUrl(f.id)
          }
        }))
      }
    ]
  };

  return (
    <div style={{
      minHeight: '100vh',
      background: 'var(--app-bg)',
      color: 'var(--app-text)',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
      paddingTop: '80px'
    }}>
      <SEO
        title="Gaply Features — Journal Verification, Research Gap Analysis, PublishReady, DataMaestro & Free Tools"
        description="Gaply features for researchers: free paper search, journal matching, APA/Vancouver/Harvard citation generator, India conference finder. Premium: journal verification (real vs predatory signals), Research Deep Analysis for gaps and problem statements, PublishReady manuscript refinement with journal-fit insight, DataMaestro statistics with manual or AI-chosen tests. Chat with Gaply and audio summaries (English, Hindi) on premium experiences."
        keywords="Gaply features, predatory journal check, journal verification tool, research gap analysis, problem statement generator PhD, PublishReady manuscript review, journal publication likelihood, DataMaestro statistical analysis, Chat with Gaply, research audio Hindi English, free citation generator, research conferences India, journal matching, academic research platform India"
        canonical={FEATURES_PAGE_URL}
      />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(featuresPageStructuredData) }}
      />
      {/* Header */}
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        zIndex: 1000,
        background: 'var(--header-bg)',
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)',
        borderBottom: '1px solid var(--header-border)',
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
            background: 'var(--toggle-bg)',
            color: 'var(--app-text)',
            padding: '10px 20px',
            borderRadius: '12px',
            border: '1px solid var(--toggle-border)',
            cursor: 'pointer',
            fontSize: '14px',
            fontWeight: '500',
            transition: 'all 0.3s ease',
            backdropFilter: 'blur(10px)',
            WebkitBackdropFilter: 'blur(10px)'
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--button-hover-bg)';
            e.currentTarget.style.borderColor = 'var(--button-hover-border)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'var(--toggle-bg)';
            e.currentTarget.style.borderColor = 'var(--toggle-border)';
          }}
        >
          ← Back to Home
        </button>
      </div>

      {/* Hero Section */}
      <div style={{
        padding: '120px clamp(16px, 4vw, 40px) 80px',
        textAlign: 'center',
        maxWidth: '1200px',
        margin: '0 auto'
      }}>
        <h1 style={{
          fontSize: 'clamp(3rem, 8vw, 6rem)',
          fontWeight: '700',
          letterSpacing: '-0.02em',
          marginBottom: '30px',
          background: 'linear-gradient(135deg, var(--app-text) 0%, var(--muted-text) 100%)',
          WebkitBackgroundClip: 'text',
          WebkitTextFillColor: 'transparent',
          backgroundClip: 'text'
        }}>
          Features
        </h1>
        
        <p style={{
          fontSize: 'clamp(1.2rem, 3vw, 1.8rem)',
          color: 'var(--muted-text)',
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
          background: 'var(--toggle-bg)',
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
              background: activeSection === 'free' ? 'var(--toggle-active-bg)' : 'transparent',
              color: 'var(--app-text)',
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
              background: activeSection === 'premium' ? 'var(--toggle-active-bg)' : 'transparent',
              color: 'var(--app-text)',
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

      {/* Features Grid — semantic articles for SEO; layout unchanged */}
      <div style={{
        padding: '0 clamp(16px, 4vw, 40px) 120px',
        maxWidth: '1200px',
        margin: '0 auto'
      }}>
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
            gap: '24px'
          }}
        >
          {(activeSection === 'free' ? freeFeatures : premiumFeatures).map((feature) => (
            <article
              key={feature.id}
              itemScope
              itemType="https://schema.org/SoftwareApplication"
              style={{
                background: 'var(--card-bg)',
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
              onClick={() => {
                if (feature.id === 'publish-ready') openPremiumPath('/dashboard/publishready');
                if (feature.id === 'data-maestro') openPremiumPath('/dashboard/datamaestro');
                if (feature.id === 'paper-search') navigate('/paper-search');
                if (feature.id === 'journal-matching') navigate('/journal-matching');
                if (feature.id === 'citation-generator') navigate('/citation-generator');
                if (feature.id === 'conference-finder') navigate('/conferences-india');
                if (feature.id === 'journal-verification') openPremiumPath('/dashboard/journal-verify');
                if (feature.id === 'research-deep-analysis') openPremiumPath('/dashboard/research-deep-analysis');
              }}
            >
              <meta itemProp="applicationCategory" content="EducationalApplication" />
              <meta itemProp="operatingSystem" content="Web" />
              {/* Title */}
              <h3 style={{
                fontSize: '20px',
                fontWeight: '600',
                marginBottom: '8px',
                color: 'var(--section-text)',
                letterSpacing: '-0.01em',
                lineHeight: '1.3'
              }}>
                <span itemProp="name">{feature.title}</span>
              </h3>

              {/* Subtitle */}
              <p style={{
                fontSize: '14px',
                color: 'var(--muted-text)',
                marginBottom: '16px',
                fontWeight: '400',
                lineHeight: '1.4'
              }}>
                <span itemProp="alternateName">{feature.subtitle}</span>
              </p>

              {/* Description */}
              <p style={{
                fontSize: '13px',
                color: 'var(--section-text)',
                marginBottom: '20px',
                lineHeight: '1.5'
              }}>
                <span itemProp="description">{feature.description}</span>
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
                    color: 'var(--muted-text)',
                    lineHeight: '1.4'
                  }}>
                    <div style={{
                      width: '4px',
                      height: '4px',
                      backgroundColor: 'var(--button-bg)',
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
                    color: 'var(--muted-text)',
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
            </article>
          ))}
        </div>
      </div>

      {/* CTA Section */}
      <div style={{
        padding: '80px clamp(16px, 4vw, 40px)',
        textAlign: 'center',
        background: 'linear-gradient(135deg, rgba(0, 122, 255, 0.1) 0%, var(--app-bg) 100%)',
        borderTop: '1px solid var(--divider)'
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
          color: 'var(--muted-text)',
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
              color: 'var(--button-text)',
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
              color: 'var(--app-text)',
              fontSize: '16px',
              fontWeight: '600',
              border: '1px solid var(--button-border)',
              cursor: 'pointer',
              transition: 'all 0.3s ease'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--button-hover-bg)';
              e.currentTarget.style.borderColor = 'var(--button-hover-border)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
              e.currentTarget.style.borderColor = 'var(--button-border)';
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
