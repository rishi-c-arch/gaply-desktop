import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { recommendJournals } from '../api/freeFeatures';
import './JournalMatchingPage.css';

interface JournalResult {
  journal: string;
  publisher: string;
  quartile: string;
  impactFactor: number;
  acceptanceRate: string;
  domain: string;
  submissionUrl: string;
  guidelinesUrl: string;
  scope: string;
  indexing: string[];
  openAccess: boolean;
  processingTime: string;
  articleProcessingCharge?: string;
}

interface DomainAnalysis {
  primaryDomain: string;
  secondaryDomains: string[];
  keywords: string[];
  methodology: string;
  researchType: string;
}

const JournalMatchingPage: React.FC = () => {
  const navigate = useNavigate();
  const [jmTitle, setJmTitle] = useState('');
  const [jmAbstract, setJmAbstract] = useState('');
  const [journalResults, setJournalResults] = useState<JournalResult[]>([]);
  const [isMatching, setIsMatching] = useState(false);
  const [domainAnalysis, setDomainAnalysis] = useState<DomainAnalysis | null>(null);

  // Comprehensive journal database by domain
  const journalDatabase = {
    'Computer Science & AI': [
      {
        journal: 'Nature Machine Intelligence',
        publisher: 'Nature Publishing Group',
        quartile: 'Q1',
        impactFactor: 25.9,
        acceptanceRate: '8%',
        domain: 'Computer Science & AI',
        submissionUrl: 'https://www.nature.com/nmachintell/for-authors',
        guidelinesUrl: 'https://www.nature.com/nmachintell/for-authors/submission-guidelines',
        scope: 'AI, machine learning, robotics, computer vision, NLP',
        indexing: ['SCI', 'SCIE', 'SSCI', 'Scopus'],
        openAccess: true,
        processingTime: '3-6 months',
        articleProcessingCharge: '$2,190'
      },
      {
        journal: 'IEEE Transactions on Pattern Analysis and Machine Intelligence',
        publisher: 'IEEE',
        quartile: 'Q1',
        impactFactor: 24.3,
        acceptanceRate: '15%',
        domain: 'Computer Science & AI',
        submissionUrl: 'https://www.computer.org/csdl/journal/pa',
        guidelinesUrl: 'https://www.computer.org/csdl/journal/pa/author-guidelines',
        scope: 'Computer vision, pattern recognition, machine learning',
        indexing: ['SCI', 'SCIE', 'Scopus'],
        openAccess: false,
        processingTime: '4-8 months',
        articleProcessingCharge: '$2,195'
      },
      {
        journal: 'Journal of Machine Learning Research',
        publisher: 'JMLR',
        quartile: 'Q1',
        impactFactor: 6.0,
        acceptanceRate: '25%',
        domain: 'Computer Science & AI',
        submissionUrl: 'https://jmlr.org/submit.html',
        guidelinesUrl: 'https://jmlr.org/submit.html',
        scope: 'Machine learning theory and applications',
        indexing: ['SCI', 'Scopus'],
        openAccess: true,
        processingTime: '2-4 months',
        articleProcessingCharge: 'Free'
      }
    ],
    'Environmental Sciences': [
      {
        journal: 'Nature Climate Change',
        publisher: 'Nature Publishing Group',
        quartile: 'Q1',
        impactFactor: 28.7,
        acceptanceRate: '6%',
        domain: 'Environmental Sciences',
        submissionUrl: 'https://www.nature.com/nclimate/for-authors',
        guidelinesUrl: 'https://www.nature.com/nclimate/for-authors/submission-guidelines',
        scope: 'Climate change impacts, adaptation, mitigation',
        indexing: ['SCI', 'SCIE', 'Scopus'],
        openAccess: true,
        processingTime: '3-6 months',
        articleProcessingCharge: '$2,190'
      },
      {
        journal: 'Environmental Science & Technology',
        publisher: 'American Chemical Society',
        quartile: 'Q1',
        impactFactor: 11.4,
        acceptanceRate: '20%',
        domain: 'Environmental Sciences',
        submissionUrl: 'https://pubs.acs.org/page/esthag/submission/index.html',
        guidelinesUrl: 'https://pubs.acs.org/page/esthag/submission/index.html',
        scope: 'Environmental chemistry, pollution control, sustainability',
        indexing: ['SCI', 'SCIE', 'Scopus'],
        openAccess: true,
        processingTime: '2-4 months',
        articleProcessingCharge: '$2,500'
      }
    ],
    'Business & Management': [
      {
        journal: 'Academy of Management Journal',
        publisher: 'Academy of Management',
        quartile: 'Q1',
        impactFactor: 9.4,
        acceptanceRate: '8%',
        domain: 'Business & Management',
        submissionUrl: 'https://journals.aom.org/journal/amj',
        guidelinesUrl: 'https://journals.aom.org/journal/amj/submission-guidelines',
        scope: 'Strategic management, organizational behavior, leadership',
        indexing: ['SSCI', 'Scopus'],
        openAccess: false,
        processingTime: '6-12 months',
        articleProcessingCharge: 'N/A'
      },
      {
        journal: 'Journal of Business Ethics',
        publisher: 'Springer',
        quartile: 'Q1',
        impactFactor: 6.4,
        acceptanceRate: '15%',
        domain: 'Business & Management',
        submissionUrl: 'https://www.springer.com/journal/10551/submission-guidelines',
        guidelinesUrl: 'https://www.springer.com/journal/10551/submission-guidelines',
        scope: 'Business ethics, CSR, sustainability, corporate governance',
        indexing: ['SSCI', 'Scopus'],
        openAccess: true,
        processingTime: '3-6 months',
        articleProcessingCharge: '$2,390'
      }
    ],
    'Social Sciences': [
      {
        journal: 'American Sociological Review',
        publisher: 'American Sociological Association',
        quartile: 'Q1',
        impactFactor: 8.2,
        acceptanceRate: '5%',
        domain: 'Social Sciences',
        submissionUrl: 'https://journals.sagepub.com/home/asr',
        guidelinesUrl: 'https://journals.sagepub.com/home/asr/submission-guidelines',
        scope: 'Sociological theory, social research, inequality',
        indexing: ['SSCI', 'Scopus'],
        openAccess: false,
        processingTime: '6-12 months',
        articleProcessingCharge: 'N/A'
      }
    ],
    'Health & Medicine': [
      {
        journal: 'The Lancet',
        publisher: 'Elsevier',
        quartile: 'Q1',
        impactFactor: 202.7,
        acceptanceRate: '3%',
        domain: 'Health & Medicine',
        submissionUrl: 'https://www.thelancet.com/lancet/for-authors',
        guidelinesUrl: 'https://www.thelancet.com/lancet/for-authors/submission-guidelines',
        scope: 'Clinical medicine, public health, global health',
        indexing: ['SCI', 'SCIE', 'Scopus'],
        openAccess: true,
        processingTime: '2-4 months',
        articleProcessingCharge: '$5,000'
      },
      {
        journal: 'Nature Medicine',
        publisher: 'Nature Publishing Group',
        quartile: 'Q1',
        impactFactor: 87.2,
        acceptanceRate: '5%',
        domain: 'Health & Medicine',
        submissionUrl: 'https://www.nature.com/nm/for-authors',
        guidelinesUrl: 'https://www.nature.com/nm/for-authors/submission-guidelines',
        scope: 'Translational medicine, clinical research, therapeutics',
        indexing: ['SCI', 'SCIE', 'Scopus'],
        openAccess: true,
        processingTime: '3-6 months',
        articleProcessingCharge: '$2,190'
      }
    ],
    'Engineering': [
      {
        journal: 'Science',
        publisher: 'American Association for the Advancement of Science',
        quartile: 'Q1',
        impactFactor: 47.7,
        acceptanceRate: '2%',
        domain: 'Engineering',
        submissionUrl: 'https://www.science.org/content/page/science-information-authors',
        guidelinesUrl: 'https://www.science.org/content/page/science-information-authors',
        scope: 'Interdisciplinary scientific research',
        indexing: ['SCI', 'SCIE', 'Scopus'],
        openAccess: true,
        processingTime: '2-4 months',
        articleProcessingCharge: '$4,500'
      }
    ],
    'Political Science & International Relations': [
      {
        journal: 'International Organization',
        publisher: 'Cambridge University Press',
        quartile: 'Q1',
        impactFactor: 4.8,
        acceptanceRate: '12%',
        domain: 'Political Science & International Relations',
        submissionUrl: 'https://www.cambridge.org/core/journals/international-organization/information/author-instructions',
        guidelinesUrl: 'https://www.cambridge.org/core/journals/international-organization/information/author-instructions',
        scope: 'International relations, global governance, political economy',
        indexing: ['SSCI', 'Scopus'],
        openAccess: false,
        processingTime: '4-8 months',
        articleProcessingCharge: 'N/A'
      },
      {
        journal: 'American Political Science Review',
        publisher: 'Cambridge University Press',
        quartile: 'Q1',
        impactFactor: 4.2,
        acceptanceRate: '8%',
        domain: 'Political Science & International Relations',
        submissionUrl: 'https://www.cambridge.org/core/journals/american-political-science-review/information/author-instructions',
        guidelinesUrl: 'https://www.cambridge.org/core/journals/american-political-science-review/information/author-instructions',
        scope: 'Political science theory, comparative politics, international relations',
        indexing: ['SSCI', 'Scopus'],
        openAccess: false,
        processingTime: '6-12 months',
        articleProcessingCharge: 'N/A'
      },
      {
        journal: 'Journal of Peace Research',
        publisher: 'SAGE Publications',
        quartile: 'Q1',
        impactFactor: 3.9,
        acceptanceRate: '15%',
        domain: 'Political Science & International Relations',
        submissionUrl: 'https://journals.sagepub.com/home/jpr',
        guidelinesUrl: 'https://journals.sagepub.com/home/jpr/submission-guidelines',
        scope: 'Conflict resolution, peace studies, international security',
        indexing: ['SSCI', 'Scopus'],
        openAccess: true,
        processingTime: '3-6 months',
        articleProcessingCharge: '$2,000'
      }
    ]
  };

  // Advanced domain analysis function
  const analyzeDomain = (title: string, abstract: string): DomainAnalysis => {
    const text = (title + ' ' + abstract).toLowerCase();
    
    // Enhanced domain keyword mapping with more comprehensive terms
    const domainKeywords = {
      'Political Science & International Relations': [
        'political', 'politics', 'diplomacy', 'diplomatic', 'international', 'foreign policy', 'governance', 
        'government', 'state', 'nation', 'country', 'region', 'geopolitics', 'security', 'conflict', 
        'peace', 'war', 'treaty', 'alliance', 'sovereignty', 'democracy', 'authoritarian', 'regime',
        'china', 'southeast asia', 'asean', 'gunboat', 'maritime', 'territorial', 'dispute', 'tension',
        'policy', 'strategy', 'military', 'defense', 'intelligence', 'espionage', 'sanctions'
      ],
      'Computer Science & AI': [
        'machine learning', 'artificial intelligence', 'deep learning', 'neural network', 'algorithm', 
        'computer vision', 'nlp', 'natural language processing', 'robotics', 'data science', 'big data', 
        'blockchain', 'cybersecurity', 'software', 'programming', 'computing', 'digital', 'automation',
        'ai', 'ml', 'neural', 'deep', 'computer', 'computational', 'algorithmic', 'data mining'
      ],
      'Environmental Sciences': [
        'climate change', 'environmental', 'sustainability', 'renewable energy', 'pollution', 
        'ecosystem', 'biodiversity', 'carbon', 'green', 'clean energy', 'environmental impact',
        'global warming', 'deforestation', 'conservation', 'ecology', 'environment', 'sustainable'
      ],
      'Business & Management': [
        'management', 'business', 'strategy', 'leadership', 'entrepreneurship', 'marketing', 
        'finance', 'economics', 'organizational', 'corporate', 'startup', 'innovation',
        'investment', 'market', 'trade', 'commerce', 'enterprise', 'company', 'firm'
      ],
      'Social Sciences': [
        'social', 'sociology', 'psychology', 'anthropology', 'culture', 'society', 'community',
        'social behavior', 'social structure', 'social change', 'social theory', 'social research',
        'human behavior', 'social psychology', 'cultural studies', 'social work'
      ],
      'Health & Medicine': [
        'health', 'medical', 'clinical', 'medicine', 'healthcare', 'pharmaceutical', 'therapy', 
        'treatment', 'disease', 'patient', 'public health', 'epidemiology', 'medical research',
        'clinical trial', 'diagnosis', 'prognosis', 'medical device', 'biomedical'
      ],
      'Engineering': [
        'engineering', 'technology', 'system', 'design', 'optimization', 'mechanical', 'electrical', 
        'civil', 'chemical', 'materials', 'manufacturing', 'construction', 'infrastructure',
        'technical', 'technological', 'innovation', 'development', 'prototype'
      ]
    };

    const keywordScores: { [key: string]: number } = {};
    
    Object.entries(domainKeywords).forEach(([domain, keywords]) => {
      keywordScores[domain] = keywords.reduce((score, keyword) => {
        const matches = (text.match(new RegExp(keyword, 'g')) || []).length;
        return score + matches;
      }, 0);
    });

    // Find the domain with the highest score, but default to 'Multidisciplinary' if no matches
    const maxScore = Math.max(...Object.values(keywordScores));
    const primaryDomain = maxScore > 0 
      ? Object.keys(keywordScores).find(domain => keywordScores[domain] === maxScore) || 'Multidisciplinary'
      : 'Multidisciplinary';

    const secondaryDomains = Object.entries(keywordScores)
      .filter(([domain, score]) => domain !== primaryDomain && score > 0)
      .sort(([,a], [,b]) => b - a)
      .slice(0, 2)
      .map(([domain]) => domain);

    const detectedKeywords = primaryDomain !== 'Multidisciplinary' 
      ? Object.entries(domainKeywords[primaryDomain as keyof typeof domainKeywords])
          .filter(([, keyword]) => text.includes(keyword))
          .map(([, keyword]) => keyword)
      : [];

    const methodology = text.includes('quantitative') ? 'Quantitative' : 
                       text.includes('qualitative') ? 'Qualitative' : 
                       text.includes('mixed') ? 'Mixed Methods' : 'Empirical';

    const researchType = text.includes('review') ? 'Review' :
                        text.includes('case study') ? 'Case Study' :
                        text.includes('experiment') ? 'Experimental' :
                        text.includes('survey') ? 'Survey' : 'Research Article';

    return {
      primaryDomain,
      secondaryDomains,
      keywords: detectedKeywords,
      methodology,
      researchType
    };
  };

  const [matchSource, setMatchSource] = useState<'backend' | 'fallback'>('backend');

  // Journal matching: backend first (Gaply API), fallback to local database
  const handleMatch = async () => {
    if (!jmTitle.trim() || !jmAbstract.trim()) {
      setJournalResults([]);
      setDomainAnalysis(null);
      return;
    }

    setIsMatching(true);

    try {
      // 1. Try backend (gaply-enhanced-backend POST /api/recommend-journals)
      const resp = await recommendJournals({
        title: jmTitle,
        abstract: jmAbstract,
        filters: { limit: 10 },
      });
      if (resp?.recommendations?.length) {
        const analysis = analyzeDomain(jmTitle, jmAbstract);
        setDomainAnalysis(analysis);
        const mapped: JournalResult[] = resp.recommendations.map((r) => {
          const quartileVal = r.quartiles ? Object.values(r.quartiles)[0] : 'Q2';
          const apc = r.apc_usd != null ? `$${r.apc_usd}` : 'N/A';
          return {
            journal: r.name,
            publisher: r.publisher || '—',
            quartile: quartileVal || 'Q2',
            impactFactor: r.sjr_value ?? 0,
            acceptanceRate: '—',
            domain: r.subject_matches?.join(', ') || 'Multidisciplinary',
            submissionUrl: r.submission_url || r.homepage || '#',
            guidelinesUrl: r.submission_url || r.homepage || '#',
            scope: r.aims_scope_snippet || 'Academic journal',
            indexing: r.subject_matches?.length ? r.subject_matches : ['Scopus'],
            openAccess: r.open_access,
            processingTime: '—',
            articleProcessingCharge: apc,
          };
        });
        setJournalResults(mapped);
        setMatchSource('backend');
        setIsMatching(false);
        return;
      }
    } catch (err) {
      console.warn('Backend journal match failed, using fallback:', err);
    }

    // 2. Fallback: local domain-based matching
    try {
      const analysis = analyzeDomain(jmTitle, jmAbstract);
      setDomainAnalysis(analysis);
      const relevantJournals: JournalResult[] = [];
      if (journalDatabase[analysis.primaryDomain as keyof typeof journalDatabase]) {
        relevantJournals.push(...journalDatabase[analysis.primaryDomain as keyof typeof journalDatabase]);
      }
      analysis.secondaryDomains.forEach((domain) => {
        if (journalDatabase[domain as keyof typeof journalDatabase]) {
          relevantJournals.push(...journalDatabase[domain as keyof typeof journalDatabase].slice(0, 2));
        }
      });
      const sortedJournals = relevantJournals
        .sort((a, b) => b.impactFactor - a.impactFactor)
        .slice(0, 8);
      setJournalResults(sortedJournals);
      setMatchSource('fallback');
    } catch (error) {
      console.error('Journal matching error:', error);
      setJournalResults([
        {
          journal: 'PLOS ONE',
          publisher: 'PLOS',
          quartile: 'Q2',
          impactFactor: 3.75,
          acceptanceRate: '48%',
          domain: 'Multidisciplinary',
          submissionUrl: 'https://journals.plos.org/plosone/s/submission-guidelines',
          guidelinesUrl: 'https://journals.plos.org/plosone/s/submission-guidelines',
          scope: 'All scientific disciplines',
          indexing: ['SCI', 'SCIE', 'Scopus'],
          openAccess: true,
          processingTime: '2-4 months',
          articleProcessingCharge: '$1,695',
        },
      ]);
      setMatchSource('fallback');
    }

    setIsMatching(false);
  };

  return (
    <div className="journal-matching-page">
      {/* Navigation Bar */}
      <div className="journal-matching-header">
        <h1 className="journal-matching-title">Journal Matching</h1>
        
        <button
          onClick={() => navigate('/')}
          className="journal-matching-back"
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--button-bg-hover)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'var(--button-bg)';
          }}
        >
          ← Back to Home
        </button>
      </div>

      {/* Main Content */}
      <div className="journal-matching-card">
        <p className="journal-matching-description">
          Get intelligent journal recommendations based on your research domain. Our AI analyzes your title and abstract to match you with the most suitable journals, complete with submission guidelines and direct links.
        </p>

        <input
          value={jmTitle}
          onChange={(e) => setJmTitle(e.target.value)}
          placeholder="Paper title..."
          className="journal-matching-input"
        />

        <textarea
          value={jmAbstract}
          onChange={(e) => setJmAbstract(e.target.value)}
          placeholder="Abstract..."
          className="journal-matching-textarea"
        />

        <button
          onClick={handleMatch}
          disabled={isMatching}
          className="journal-matching-button"
          style={{
            background: !isMatching ? 'var(--button-bg-hover)' : 'var(--button-bg)',
            cursor: !isMatching ? 'pointer' : 'not-allowed',
          }}
          onMouseEnter={(e) => {
            if (!isMatching) {
              e.currentTarget.style.background = 'var(--button-bg-hover)';
            }
          }}
          onMouseLeave={(e) => {
            if (!isMatching) {
              e.currentTarget.style.background = 'var(--button-bg)';
            }
          }}
        >
          {isMatching ? 'Finding Matches...' : 'Find Matching Journals'}
        </button>


        {/* Results */}
        {journalResults.length > 0 && (
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '24px',
          }}>
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: '8px',
            }}>
              <h3 style={{
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                fontSize: '1.25rem',
                fontWeight: '600',
                color: 'var(--section-text)',
                margin: 0,
              }}>
                🎯 Recommended Journals ({journalResults.length})
              </h3>
              <div style={{
                fontSize: '0.875rem',
                color: 'var(--muted-text)',
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
              }}>
                {matchSource === 'backend' ? 'Powered by Gaply Backend' : 'Sorted by Impact Factor'}
              </div>
            </div>

            {journalResults.map((result, i) => (
              <div
                key={i}
                style={{
                  padding: '32px',
                  borderRadius: '24px',
                  background: 'var(--card-bg)',
                  border: '1px solid var(--card-border)',
                  transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                  position: 'relative',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'var(--card-bg-hover)';
                  e.currentTarget.style.borderColor = 'var(--card-border-hover)';
                  e.currentTarget.style.transform = 'translateY(-2px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'var(--card-bg)';
                  e.currentTarget.style.borderColor = 'var(--card-border)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                {/* Journal Header */}
                <div style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'flex-start',
                  marginBottom: '20px',
                }}>
                  <div style={{ flex: 1 }}>
                    <h4 style={{
                      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                      fontSize: '1.4rem',
                      fontWeight: '600',
                      color: 'var(--section-text)',
                      margin: '0 0 8px 0',
                      lineHeight: '1.3',
                    }}>
                      {result.journal}
                    </h4>
                    <p style={{
                      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                      fontSize: '1rem',
                      color: 'var(--muted-text)',
                      margin: '0 0 12px 0',
                    }}>
                      {result.publisher}
                    </p>
                    <div style={{
                      display: 'flex',
                      gap: '12px',
                      flexWrap: 'wrap',
                      alignItems: 'center',
                    }}>
                      <div style={{
                        background: result.quartile === 'Q1' ? 'rgba(34, 197, 94, 0.2)' : 
                                   result.quartile === 'Q2' ? 'rgba(59, 130, 246, 0.2)' :
                                   result.quartile === 'Q3' ? 'rgba(245, 158, 11, 0.2)' : 'rgba(239, 68, 68, 0.2)',
                        color: result.quartile === 'Q1' ? '#22C55E' : 
                               result.quartile === 'Q2' ? '#3B82F6' :
                               result.quartile === 'Q3' ? '#F59E0B' : '#EF4444',
                        padding: '6px 12px',
                        borderRadius: '12px',
                        fontSize: '0.85rem',
                        fontWeight: '600',
                        fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                        border: `1px solid ${result.quartile === 'Q1' ? 'rgba(34, 197, 94, 0.3)' : 
                                               result.quartile === 'Q2' ? 'rgba(59, 130, 246, 0.3)' :
                                               result.quartile === 'Q3' ? 'rgba(245, 158, 11, 0.3)' : 'rgba(239, 68, 68, 0.3)'}`,
                      }}>
                        {result.quartile}
                      </div>
                      <div style={{
                        background: 'var(--badge-bg)',
                        color: 'var(--accent-blue)',
                        padding: '6px 12px',
                        borderRadius: '12px',
                        fontSize: '0.85rem',
                        fontWeight: '600',
                        fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                        border: '1px solid var(--badge-border)',
                      }}>
                        IF: {result.impactFactor}
                      </div>
                      <div style={{
                        background: 'var(--badge-bg)',
                        color: 'var(--section-text)',
                        padding: '6px 12px',
                        borderRadius: '12px',
                        fontSize: '0.85rem',
                        fontWeight: '500',
                        fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                        border: '1px solid var(--badge-border)',
                      }}>
                        {result.acceptanceRate} acceptance
                      </div>
                      {result.openAccess && (
                        <div style={{
                          background: 'rgba(34, 197, 94, 0.2)',
                          color: '#22C55E',
                          padding: '6px 12px',
                          borderRadius: '12px',
                          fontSize: '0.85rem',
                          fontWeight: '600',
                          fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                          border: '1px solid rgba(34, 197, 94, 0.3)',
                        }}>
                          Open Access
                        </div>
                      )}
                    </div>
                  </div>
                </div>

                {/* Scope and Details */}
                <div style={{
                  marginBottom: '20px',
                }}>
                  <p style={{
                    fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                    fontSize: '0.95rem',
                    color: 'var(--section-text)',
                    margin: '0 0 12px 0',
                    lineHeight: '1.5',
                  }}>
                    <strong style={{ color: 'var(--section-text)' }}>Scope:</strong> {result.scope}
                  </p>
                  <div style={{
                    display: 'flex',
                    gap: '20px',
                    flexWrap: 'wrap',
                    fontSize: '0.9rem',
                    color: 'var(--muted-text)',
                  }}>
                    <span><strong style={{ color: 'var(--section-text)' }}>Processing Time:</strong> {result.processingTime}</span>
                    <span><strong style={{ color: 'var(--section-text)' }}>Indexing:</strong> {result.indexing.join(', ')}</span>
                    {result.articleProcessingCharge && (
                      <span><strong style={{ color: 'var(--section-text)' }}>APC:</strong> {result.articleProcessingCharge}</span>
                    )}
                  </div>
                </div>

                {/* Action Buttons */}
                <div style={{
                  display: 'flex',
                  gap: '12px',
                  flexWrap: 'wrap',
                }}>
                  <a
                    href={result.submissionUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    style={{
                      background: 'linear-gradient(135deg, #007AFF, #005BB5)',
                      color: '#ffffff',
                      padding: '12px 24px',
                      borderRadius: '12px',
                      fontSize: '0.9rem',
                      fontWeight: '600',
                      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                      textDecoration: 'none',
                      transition: 'all 0.3s ease',
                      border: 'none',
                      cursor: 'pointer',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = 'linear-gradient(135deg, #005BB5, #004494)';
                      e.currentTarget.style.transform = 'translateY(-1px)';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = 'linear-gradient(135deg, #007AFF, #005BB5)';
                      e.currentTarget.style.transform = 'translateY(0)';
                    }}
                  >
                    📝 Submit Paper
                  </a>
                  <a
                    href={result.guidelinesUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    style={{
                      background: 'transparent',
                      color: 'var(--button-text)',
                      padding: '12px 24px',
                      borderRadius: '12px',
                      fontSize: '0.9rem',
                      fontWeight: '600',
                      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                      textDecoration: 'none',
                      transition: 'all 0.3s ease',
                      border: '1px solid var(--button-border)',
                      cursor: 'pointer',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = 'var(--button-bg-hover)';
                      e.currentTarget.style.borderColor = 'var(--button-border-hover)';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = 'transparent';
                      e.currentTarget.style.borderColor = 'var(--button-border)';
                    }}
                  >
                    📋 Guidelines
                  </a>
                </div>

                {/* External Link Indicator */}
                <div style={{
                  position: 'absolute',
                  top: '24px',
                  right: '24px',
                  fontSize: '0.8rem',
                  color: 'var(--muted-text)',
                  fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                }}>
                  ↗
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

export default JournalMatchingPage;

