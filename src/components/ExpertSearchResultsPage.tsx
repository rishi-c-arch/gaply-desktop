import React, { useState, useEffect } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { expertSearchAPI, ExpertProfile, ExpertDomain, SearchResponse } from '../services/expertSearchAPI';
import ExpertSearch from './ExpertSearch';
import HireRequestDialog from './HireRequestDialog';

const ExpertSearchResultsPage: React.FC = () => {
  const location = useLocation();
  const navigate = useNavigate();
  const [searchResults, setSearchResults] = useState<SearchResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedExpert, setSelectedExpert] = useState<ExpertProfile | null>(null);
  const [isDialogOpen, setIsDialogOpen] = useState(false);

  useEffect(() => {
    // Get search query from URL params
    const urlParams = new URLSearchParams(location.search);
    const query = urlParams.get('q') || '';
    setSearchQuery(query);

    if (query) {
      performSearch(query);
    } else {
      setIsLoading(false);
    }
  }, [location.search]);

  const performSearch = async (query: string) => {
    setIsLoading(true);
    setError(null);

    try {
      // Try enhanced fuzzy search first
      const fuzzyData = await expertSearchAPI.searchExpertsFuzzy(query);
      if (fuzzyData.expert_profiles.length > 0) {
        setSearchResults(fuzzyData);
      } else {
        // Fallback to regular search
        const data = await expertSearchAPI.searchExperts(query);
        setSearchResults(data);
      }
    } catch (err) {
      console.error('Search failed:', err);
      setError(`Zero experts live for "${query}"`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleNewSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (searchQuery.trim()) {
      navigate(`/search-results?q=${encodeURIComponent(searchQuery)}`);
      performSearch(searchQuery);
    }
  };

  const handleExpertClick = (expert: ExpertProfile) => {
    // Navigate to expert detail page or open contact modal
    console.log('Selected expert:', expert);
    // You can implement expert detail view or contact functionality here
  };

  const handleHireExpert = (expert: ExpertProfile) => {
    setSelectedExpert(expert);
    setIsDialogOpen(true);
  };

  const handleCloseDialog = () => {
    setIsDialogOpen(false);
    setSelectedExpert(null);
  };

  const handleDomainClick = (domain: ExpertDomain) => {
    // Search for experts in this domain
    navigate(`/search-results?q=${encodeURIComponent(domain.name)}`);
    performSearch(domain.name);
  };

  return (
    <div style={{
      minHeight: '100vh',
      background: 'linear-gradient(135deg, #000000 0%, #0A0A0A 50%, #000000 100%)',
      color: '#ffffff',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
      paddingTop: '80px'
    }}>
      {/* Background Elements */}
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'radial-gradient(circle at 30% 20%, rgba(0, 122, 255, 0.05) 0%, transparent 50%), radial-gradient(circle at 70% 80%, rgba(88, 86, 214, 0.05) 0%, transparent 50%)',
        pointerEvents: 'none',
        zIndex: 0
      }} />

      {/* Floating Particles */}
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        pointerEvents: 'none',
        zIndex: 0
      }}>
        {Array.from({ length: 30 }).map((_, i) => (
          <div
            key={i}
            style={{
              position: 'absolute',
              width: '1px',
              height: '1px',
              background: 'rgba(255, 255, 255, 0.2)',
              borderRadius: '50%',
              left: `${Math.random() * 100}%`,
              top: `${Math.random() * 100}%`,
              animation: `float ${3 + Math.random() * 4}s ease-in-out infinite`,
              animationDelay: `${Math.random() * 2}s`
            }}
          />
        ))}
      </div>

      <div style={{ position: 'relative', zIndex: 1 }}>
        {/* Back Button */}
        <div style={{
          maxWidth: '1200px',
          margin: '0 auto',
          padding: '20px 20px 0 20px'
        }}>
          <button
            onClick={() => navigate('/hire-expert')}
            style={{
              background: 'rgba(255, 255, 255, 0.05)',
              border: '1px solid rgba(255, 255, 255, 0.1)',
              borderRadius: '12px',
              padding: '12px 20px',
              fontSize: '14px',
              fontWeight: '500',
              color: '#ffffff',
              fontFamily: 'inherit',
              letterSpacing: '-0.01em',
              cursor: 'pointer',
              transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
              textTransform: 'none',
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              backdropFilter: 'blur(20px)',
              WebkitBackdropFilter: 'blur(20px)'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
              e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
              e.currentTarget.style.transform = 'translateY(-2px)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
              e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
              e.currentTarget.style.transform = 'translateY(0)';
            }}
          >
            <span style={{ fontSize: '16px' }}>←</span>
            Back to Hire Expert
          </button>
        </div>

        {/* Header Section */}
        <div style={{
          maxWidth: '1200px',
          margin: '0 auto',
          padding: '40px 20px',
          textAlign: 'center'
        }}>
          {/* Search Bar */}
          <div style={{
            maxWidth: '600px',
            margin: '0 auto 40px auto',
            position: 'relative'
          }}>
            <form onSubmit={handleNewSearch}>
              <div style={{
                position: 'relative',
                background: 'rgba(255, 255, 255, 0.05)',
                borderRadius: '16px',
                border: '1px solid rgba(255, 255, 255, 0.1)',
                backdropFilter: 'blur(20px)',
                WebkitBackdropFilter: 'blur(20px)',
                overflow: 'hidden',
                transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                boxShadow: '0 8px 32px rgba(0, 0, 0, 0.2)',
                height: '56px'
              }}>
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search experts by name, expertise, or domain..."
                  style={{
                    width: '100%',
                    height: '100%',
                    padding: '0 140px 0 24px',
                    fontSize: '16px',
                    fontWeight: '400',
                    background: 'transparent',
                    border: 'none',
                    borderRadius: '16px',
                    color: '#ffffff',
                    fontFamily: 'inherit',
                    letterSpacing: '-0.01em',
                    outline: 'none',
                    transition: 'all 0.3s ease',
                    boxSizing: 'border-box'
                  }}
                  onFocus={(e) => {
                    const parentElement = e.target.parentElement;
                    if (parentElement) {
                      parentElement.style.borderColor = 'rgba(255, 255, 255, 0.25)';
                      parentElement.style.boxShadow = '0 12px 48px rgba(255, 255, 255, 0.15), 0 8px 32px rgba(0, 0, 0, 0.25)';
                    }
                  }}
                  onBlur={(e) => {
                    const parentElement = e.target.parentElement;
                    if (parentElement) {
                      parentElement.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                      parentElement.style.boxShadow = '0 8px 32px rgba(0, 0, 0, 0.2)';
                    }
                  }}
                />
                
                <button
                  type="submit"
                  disabled={!searchQuery.trim()}
                  style={{
                    position: 'absolute',
                    right: '8px',
                    top: '50%',
                    transform: 'translateY(-50%)',
                    background: searchQuery.trim() 
                      ? 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)' 
                      : 'rgba(255, 255, 255, 0.08)',
                    border: 'none',
                    borderRadius: '8px',
                    padding: '10px 24px',
                    fontSize: '14px',
                    fontWeight: '600',
                    color: searchQuery.trim() ? '#ffffff' : 'rgba(255, 255, 255, 0.4)',
                    fontFamily: 'inherit',
                    letterSpacing: '-0.01em',
                    cursor: searchQuery.trim() ? 'pointer' : 'not-allowed',
                    transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                    textTransform: 'none',
                    opacity: searchQuery.trim() ? '1' : '0.6',
                    height: '40px'
                  }}
                >
                  Search
                </button>
              </div>
            </form>
          </div>

          {/* Search Results Header */}
          {searchQuery && (
            <div style={{ marginBottom: '40px' }}>
              <h1 style={{
                fontSize: 'clamp(2rem, 4vw, 3rem)',
                fontWeight: '700',
                marginBottom: '16px',
                background: 'linear-gradient(135deg, #ffffff 0%, #e0e0e0 100%)',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
                backgroundClip: 'text',
                letterSpacing: '-0.02em'
              }}>
                Search Results
              </h1>
              <p style={{
                fontSize: 'clamp(1rem, 2vw, 1.2rem)',
                color: 'rgba(255, 255, 255, 0.7)',
                fontWeight: '400',
                letterSpacing: '-0.01em'
              }}>
                Found {searchResults?.total_results || 0} experts for "{searchQuery}"
              </p>
            </div>
          )}
        </div>

        {/* Loading State */}
        {isLoading && (
          <div style={{
            display: 'flex',
            justifyContent: 'center',
            alignItems: 'center',
            minHeight: '400px',
            flexDirection: 'column',
            gap: '20px'
          }}>
            <div style={{
              width: '40px',
              height: '40px',
              border: '3px solid rgba(255, 255, 255, 0.1)',
              borderTop: '3px solid #007AFF',
              borderRadius: '50%',
              animation: 'spin 1s linear infinite'
            }} />
            <p style={{
              fontSize: '16px',
              color: 'rgba(255, 255, 255, 0.7)',
              fontWeight: '400'
            }}>
              Searching experts...
            </p>
          </div>
        )}

        {/* Error State */}
        {error && (
          <div style={{
            maxWidth: '1200px',
            margin: '0 auto',
            padding: '40px 20px',
            textAlign: 'center'
          }}>
            <div style={{
              background: 'rgba(255, 0, 0, 0.1)',
              border: '1px solid rgba(255, 0, 0, 0.3)',
              borderRadius: '16px',
              padding: '32px',
              color: '#ff6b6b',
              fontSize: '16px'
            }}>
              {error}
            </div>
          </div>
        )}

        {/* Search Results */}
        {!isLoading && !error && searchResults && (
          <div style={{
            maxWidth: '1200px',
            margin: '0 auto',
            padding: '0 20px 80px 20px'
          }}>
            {/* Suggested Domains */}
            {searchResults.suggested_domains.length > 0 && (
              <div style={{ marginBottom: '60px' }}>
                <h2 style={{
                  fontSize: 'clamp(1.5rem, 3vw, 2rem)',
                  fontWeight: '600',
                  marginBottom: '32px',
                  color: '#ffffff',
                  textAlign: 'center',
                  letterSpacing: '-0.01em'
                }}>
                  Related Domains
                </h2>
                <div style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
                  gap: '20px'
                }}>
                  {searchResults.suggested_domains.map((domain) => (
                    <div
                      key={domain.id}
                      onClick={() => handleDomainClick(domain)}
                      style={{
                        background: 'rgba(255, 255, 255, 0.05)',
                        border: '1px solid rgba(255, 255, 255, 0.1)',
                        borderRadius: '16px',
                        padding: '24px',
                        cursor: 'pointer',
                        transition: 'all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                        backdropFilter: 'blur(20px)',
                        WebkitBackdropFilter: 'blur(20px)',
                        position: 'relative',
                        overflow: 'hidden'
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                        e.currentTarget.style.transform = 'translateY(-4px) scale(1.02)';
                        e.currentTarget.style.boxShadow = '0 20px 60px rgba(0, 122, 255, 0.2)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                        e.currentTarget.style.transform = 'translateY(0) scale(1)';
                        e.currentTarget.style.boxShadow = 'none';
                      }}
                    >
                      <h3 style={{
                        fontSize: '1.2rem',
                        fontWeight: '600',
                        color: '#ffffff',
                        marginBottom: '12px',
                        letterSpacing: '-0.01em'
                      }}>
                        {domain.name}
                      </h3>
                      <p style={{
                        fontSize: '0.9rem',
                        color: 'rgba(255, 255, 255, 0.6)',
                        marginBottom: '16px',
                        lineHeight: '1.4'
                      }}>
                        {domain.category}
                      </p>
                      <p style={{
                        fontSize: '0.95rem',
                        color: 'rgba(255, 255, 255, 0.8)',
                        lineHeight: '1.5',
                        marginBottom: '16px'
                      }}>
                        {domain.description}
                      </p>
                      <div style={{
                        display: 'flex',
                        flexWrap: 'wrap',
                        gap: '8px'
                      }}>
                        {domain.keywords.slice(0, 4).map((keyword, index) => (
                          <span
                            key={index}
                            style={{
                              fontSize: '0.8rem',
                              padding: '6px 12px',
                              background: 'rgba(255, 255, 255, 0.1)',
                              borderRadius: '20px',
                              color: 'rgba(255, 255, 255, 0.7)',
                              fontWeight: '500'
                            }}
                          >
                            {keyword}
                          </span>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Expert Profiles */}
            {searchResults.expert_profiles.length > 0 && (
              <div>
                <h2 style={{
                  fontSize: 'clamp(1.5rem, 3vw, 2rem)',
                  fontWeight: '600',
                  marginBottom: '32px',
                  color: '#ffffff',
                  textAlign: 'center',
                  letterSpacing: '-0.01em'
                }}>
                  Expert Profiles
                </h2>
                <div style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(350px, 1fr))',
                  gap: '32px'
                }}>
                  {searchResults.expert_profiles.map((expert) => (
                    <div
                      key={expert.id}
                      onClick={() => handleExpertClick(expert)}
                      style={{
                        background: 'rgba(255, 255, 255, 0.05)',
                        border: '1px solid rgba(255, 255, 255, 0.1)',
                        borderRadius: '20px',
                        overflow: 'hidden',
                        cursor: 'pointer',
                        transition: 'all 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94)',
                        backdropFilter: 'blur(20px)',
                        WebkitBackdropFilter: 'blur(20px)',
                        position: 'relative',
                        transform: 'translateY(0)',
                        boxShadow: '0 8px 32px rgba(0, 0, 0, 0.2)'
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.background = 'rgba(255, 255, 255, 0.08)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                        e.currentTarget.style.transform = 'translateY(-8px) scale(1.02)';
                        e.currentTarget.style.boxShadow = '0 24px 80px rgba(0, 122, 255, 0.15), 0 8px 32px rgba(0, 0, 0, 0.3)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
                        e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                        e.currentTarget.style.transform = 'translateY(0) scale(1)';
                        e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 0, 0, 0.2)';
                      }}
                    >
                      {/* Expert Image */}
                      <div style={{
                        height: '200px',
                        background: 'linear-gradient(135deg, #1a1a1a 0%, #000000 100%)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        position: 'relative',
                        overflow: 'hidden'
                      }}>
                        {expert.image_url ? (
                          <img
                            src={expert.image_url}
                            alt={expert.name}
                            style={{
                              width: '120px',
                              height: '120px',
                              borderRadius: '50%',
                              objectFit: 'cover',
                              border: '3px solid rgba(255, 255, 255, 0.2)',
                              boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)'
                            }}
                            onError={(e) => {
                              e.currentTarget.style.display = 'none';
                              const nextElement = e.currentTarget.nextElementSibling as HTMLElement;
                              if (nextElement) {
                                nextElement.style.display = 'flex';
                              }
                            }}
                          />
                        ) : null}
                        <div style={{
                          width: '120px',
                          height: '120px',
                          borderRadius: '50%',
                          background: 'rgba(255, 255, 255, 0.1)',
                          display: expert.image_url ? 'none' : 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                          fontSize: '32px',
                          fontWeight: '700',
                          color: '#ffffff',
                          border: '3px solid rgba(255, 255, 255, 0.2)',
                          boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)'
                        }}>
                          {expert.name.split(' ').map(n => n[0]).join('').toUpperCase()}
                        </div>
                        
                        {/* Availability Badge */}
                        <div style={{
                          position: 'absolute',
                          top: '16px',
                          right: '16px',
                          backgroundColor: expert.is_available 
                            ? 'rgba(0, 255, 0, 0.2)' 
                            : 'rgba(255, 0, 0, 0.2)',
                          color: expert.is_available ? '#51cf66' : '#ff6b6b',
                          padding: '8px 16px',
                          borderRadius: '20px',
                          fontSize: '12px',
                          fontWeight: '600',
                          letterSpacing: '0.5px',
                          backdropFilter: 'blur(20px)',
                          border: `1px solid ${expert.is_available ? 'rgba(0, 255, 0, 0.3)' : 'rgba(255, 0, 0, 0.3)'}`,
                          textTransform: 'uppercase'
                        }}>
                          {expert.is_available ? 'Available' : 'Busy'}
                        </div>
                      </div>

                      {/* Expert Info */}
                      <div style={{ padding: '32px' }}>
                        <h3 style={{
                          fontSize: '1.4rem',
                          fontWeight: '600',
                          color: '#ffffff',
                          marginBottom: '8px',
                          letterSpacing: '-0.01em',
                          lineHeight: '1.3'
                        }}>
                          {expert.name}
                        </h3>
                        
                        <div style={{
                          display: 'flex',
                          gap: '20px',
                          marginBottom: '20px',
                          fontSize: '0.9rem',
                          color: 'rgba(255, 255, 255, 0.6)'
                        }}>
                          <span>{expert.experience} years exp</span>
                          <span>{expert.publications} papers</span>
                          <span>{expert.patents} patents</span>
                        </div>
                        
                        <p style={{
                          fontSize: '0.95rem',
                          color: 'rgba(255, 255, 255, 0.8)',
                          lineHeight: '1.5',
                          marginBottom: '24px',
                          fontWeight: '300'
                        }}>
                          {expert.bio}
                        </p>
                        
                        <div style={{
                          display: 'flex',
                          flexWrap: 'wrap',
                          gap: '8px',
                          marginBottom: '24px'
                        }}>
                          {expert.domains.slice(0, 3).map((domain, index) => (
                            <span
                              key={index}
                              style={{
                                fontSize: '0.8rem',
                                padding: '6px 12px',
                                background: 'rgba(255, 255, 255, 0.1)',
                                borderRadius: '20px',
                                color: 'rgba(255, 255, 255, 0.7)',
                                fontWeight: '500',
                                border: '1px solid rgba(255, 255, 255, 0.1)'
                              }}
                            >
                              {domain}
                            </span>
                          ))}
                          {expert.domains.length > 3 && (
                            <span style={{
                              fontSize: '0.8rem',
                              padding: '6px 12px',
                              background: 'rgba(255, 255, 255, 0.05)',
                              borderRadius: '20px',
                              color: 'rgba(255, 255, 255, 0.5)',
                              fontWeight: '500'
                            }}>
                              +{expert.domains.length - 3} more
                            </span>
                          )}
                        </div>
                        
                        <button 
                          onClick={() => handleHireExpert(expert)}
                          style={{
                            width: '100%',
                            background: 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)',
                            border: 'none',
                            borderRadius: '12px',
                            padding: '16px 24px',
                            fontSize: '14px',
                            fontWeight: '600',
                            color: '#ffffff',
                            fontFamily: 'inherit',
                            letterSpacing: '-0.01em',
                            cursor: 'pointer',
                            transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                            textTransform: 'none',
                            position: 'relative',
                            overflow: 'hidden'
                          }}
                          onMouseEnter={(e) => {
                            e.currentTarget.style.transform = 'translateY(-2px)';
                            e.currentTarget.style.boxShadow = '0 8px 32px rgba(0, 122, 255, 0.3)';
                          }}
                          onMouseLeave={(e) => {
                            e.currentTarget.style.transform = 'translateY(0)';
                            e.currentTarget.style.boxShadow = 'none';
                          }}
                        >
                          Hire Expert
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* No Results */}
            {searchResults.expert_profiles.length === 0 && searchResults.suggested_domains.length === 0 && (
              <div style={{
                textAlign: 'center',
                padding: '80px 20px',
                color: 'rgba(255, 255, 255, 0.6)'
              }}>
                <div style={{
                  fontSize: '4rem',
                  marginBottom: '24px',
                  opacity: 0.5
                }}>
                  🔍
                </div>
                <h3 style={{
                  fontSize: '1.5rem',
                  fontWeight: '600',
                  color: '#ffffff',
                  marginBottom: '12px'
                }}>
                  Zero experts live for "{searchQuery}"
                </h3>
                <p style={{
                  fontSize: '1rem',
                  lineHeight: '1.5',
                  maxWidth: '400px',
                  margin: '0 auto'
                }}>
                  Try searching with different keywords or browse our suggested domains above.
                </p>
              </div>
            )}
          </div>
        )}
      </div>

      {/* CSS Animations */}
      <style>{`
        @keyframes float {
          0% { transform: translateY(0px); }
          50% { transform: translateY(-10px); }
          100% { transform: translateY(0px); }
        }
        
        @keyframes spin {
          0% { transform: rotate(0deg); }
          100% { transform: rotate(360deg); }
        }
      `}</style>

      {/* Hire Request Dialog */}
      {selectedExpert && (
        <HireRequestDialog
          expert={selectedExpert}
          isOpen={isDialogOpen}
          onClose={handleCloseDialog}
        />
      )}
    </div>
  );
};

export default ExpertSearchResultsPage;
