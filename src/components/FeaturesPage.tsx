import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import SEO from './SEO';

const FEATURES_PAGE_URL = 'https://www.gaply.in/features';

const FeaturesPage: React.FC = () => {
  const navigate = useNavigate();
  const [activeSection, setActiveSection] = useState<'free' | 'premium'>('free');
  const [hoveredFeature, setHoveredFeature] = useState<string | null>(null);

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
        'Verify every date, venue, and CFP on the organizer’s website'
      ],
      color: 'rgba(175, 82, 222, 0.1)',
      borderColor: 'rgba(175, 82, 222, 0.3)'
    }
  ];

  // Premium Features Data
  const premiumFeatures = [
    {
      id: 'publish-ready',
      title: 'PublishReady',
      subtitle: 'Journal-fit evaluation in minutes',
      description: 'Plain-language checks that compare your manuscript to journal rules and return a publication-likelihood score with prioritized fixes.',
      features: [
        'Clear, trustable, instant evaluation from journal guidelines + DOI/CrossRef checks',
        'Upload manuscript (PDF/DOCX/TXT) + paste journal guideline link',
        'Compliance checks: structure, word limits, section order, figure/table rules',
        'Reference validation and formatting (APA/IEEE/etc.)',
        'Formatting + submission checklist with pass/fail items',
        'Plagiarism & similarity signals with exact-match highlights',
        'AI-use detection signal with transparent rationale',
        'Novelty estimate vs. similar papers',
        'Line-by-line edits with rewrite suggestions',
        'Annotated report + shareable PDF',
        'Interactive chat for clarifications, examples, and step-by-step fixes',
        '3 steps: choose journal -> upload manuscript -> review & fix'
      ],
      color: 'rgba(90, 200, 250, 0.1)',
      borderColor: 'rgba(90, 200, 250, 0.3)'
    },
    {
      id: 'data-maestro',
      title: 'DataMaestro',
      subtitle: 'Academic-grade analysis, fast and clear',
      description: 'Upload data, describe your study, and receive a chapter-style report with tables, figures, formulas, and plain-English interpretations.',
      features: [
        'Upload CSV/XLS/XLSX/TSV/PDF tables/DOCX datasets',
        'Enter title, objectives, hypotheses, methodology, variables',
        'Auto-select or manually choose tests (t-tests, ANOVA, regression, SEM, time-series)',
        'Publication-style chapters: Methods -> Results -> Interpretation -> Conclusion',
        'Professional tables & labeled figures, export-ready',
        'LaTeX formulas included for methods and reports',
        'Plain-language explanations for every result',
        'Interactive Q&A tied to specific tables/plots',
        'One-click re-run after edits or data fixes',
        'Outputs: executive summary, full chapter write-up, stats tables, high-quality figures',
        'Best for PhD students, supervisors, research groups, non-coders',
        'Honest note: guidance is reliable, final interpretation stays with you'
      ],
      color: 'rgba(255, 45, 85, 0.1)',
      borderColor: 'rgba(255, 45, 85, 0.3)'
    }
  ];

  const freeFeaturesStructuredData = {
    '@context': 'https://schema.org',
    '@graph': [
      {
        '@type': 'WebPage',
        '@id': `${FEATURES_PAGE_URL}#webpage`,
        url: FEATURES_PAGE_URL,
        name: 'Gaply Features — Academic research tools',
        description:
          'Discover Gaply free and premium features: literature search, journal matching, citation generator, India conference finder, PublishReady, and DataMaestro.'
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
            url:
              f.id === 'paper-search'
                ? 'https://www.gaply.in/paper-search'
                : f.id === 'journal-matching'
                  ? 'https://www.gaply.in/journal-matching'
                  : f.id === 'citation-generator'
                    ? 'https://www.gaply.in/citation-generator'
                    : f.id === 'conference-finder'
                      ? 'https://www.gaply.in/conferences-india'
                      : FEATURES_PAGE_URL
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
        title="Gaply Features — Paper Search, Journal Matching, Citation Generator & India Conferences"
        description="Free and premium academic tools: research paper search (arXiv, CrossRef, OpenAlex), AI journal matching, free citation generator with APA Vancouver Harvard and DOI ISBN PubMed lookup plus BibTeX RIS JSON export, and research conferences in India by field and city. Premium: PublishReady and DataMaestro for publication readiness and statistical reporting."
        keywords="Gaply features, free citation generator APA Vancouver Harvard, DOI citation lookup, PubMed citation, BibTeX converter, RIS to APA, academic references browser, research conferences India, academic conferences India STEM medicine, journal matching tool, research paper search OpenAlex CrossRef, PhD research tools, academic writing India"
        canonical={FEATURES_PAGE_URL}
      />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(freeFeaturesStructuredData) }}
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
                if (feature.id === 'publish-ready') navigate('/final-orchestrator');
                if (feature.id === 'data-maestro') navigate('/statistical-research');
                if (feature.id === 'paper-search') navigate('/paper-search');
                if (feature.id === 'journal-matching') navigate('/journal-matching');
                if (feature.id === 'citation-generator') navigate('/citation-generator');
                if (feature.id === 'conference-finder') navigate('/conferences-india');
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
