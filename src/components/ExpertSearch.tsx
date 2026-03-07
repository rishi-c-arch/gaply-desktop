import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { expertSearchAPI, ExpertDomain, ExpertProfile } from '../services/expertSearchAPI';

interface ExpertSearchProps {
  onExpertSelect?: (expert: ExpertProfile) => void;
  onDomainSelect?: (domain: ExpertDomain) => void;
}

const ExpertSearch: React.FC<ExpertSearchProps> = ({ onExpertSelect, onDomainSelect }) => {
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<ExpertDomain[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);

  // Fetch domain suggestions on component mount
  useEffect(() => {
    fetchDomainSuggestions();
  }, []);

  const fetchDomainSuggestions = async () => {
    try {
      const suggestions = await expertSearchAPI.getDomainSuggestions();
      setSuggestions(suggestions);
    } catch (err) {
      console.error('Failed to fetch domain suggestions:', err);
      // Fallback to mock data
      setSuggestions([]);
    }
  };

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!searchQuery.trim()) return;

    setIsLoading(true);
    setError(null);

    // Navigate to search results page
    navigate(`/search-results?q=${encodeURIComponent(searchQuery)}`);
    
    setIsLoading(false);
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setSearchQuery(value);
    
    // Show suggestions if input is not empty
    if (value.trim()) {
      setShowSuggestions(true);
    } else {
      setShowSuggestions(false);
    }
  };

  const handleSuggestionClick = (domain: ExpertDomain) => {
    setSearchQuery(domain.name);
    setShowSuggestions(false);
    // Navigate to search results page with domain name
    navigate(`/search-results?q=${encodeURIComponent(domain.name)}`);
  };

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const handleExpertClick = (expert: ExpertProfile) => {
    if (onExpertSelect) {
      onExpertSelect(expert);
    }
  };

  const filteredSuggestions = suggestions.filter(domain =>
    domain.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    domain.keywords.some(keyword => 
      keyword.toLowerCase().includes(searchQuery.toLowerCase())
    )
  ).slice(0, 5);

  return (
    <div style={{ width: '100%', maxWidth: '1000px', margin: '0 auto' }}>
      {/* Search Form */}
      <form onSubmit={handleSearch} style={{ position: 'relative' }}>
        <div style={{
          position: 'relative',
          background: 'rgba(255, 255, 255, 0.03)',
          borderRadius: '8px',
          border: '1px solid rgba(255, 255, 255, 0.08)',
          backdropFilter: 'blur(20px)',
          WebkitBackdropFilter: 'blur(20px)',
          overflow: 'hidden',
          transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
          boxShadow: '0 4px 16px rgba(0, 0, 0, 0.1)',
          height: '40px'
        }}>
          <input
            type="text"
            value={searchQuery}
            onChange={handleInputChange}
            placeholder="Search experts by name, expertise, or domain..."
            style={{
              width: '100%',
              height: '100%',
              padding: '0 100px 0 16px',
              fontSize: '14px',
              fontWeight: '400',
              background: 'transparent',
              border: 'none',
              borderRadius: '8px',
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
                parentElement.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                parentElement.style.boxShadow = '0 8px 24px rgba(255, 255, 255, 0.1), 0 4px 16px rgba(0, 0, 0, 0.15)';
              }
            }}
            onBlur={(e) => {
              const parentElement = e.target.parentElement;
              if (parentElement) {
                parentElement.style.borderColor = 'rgba(255, 255, 255, 0.08)';
                parentElement.style.boxShadow = '0 4px 16px rgba(0, 0, 0, 0.1)';
              }
            }}
          />
          
          <button
            type="submit"
            disabled={!searchQuery.trim() || isLoading}
            style={{
              position: 'absolute',
              right: '4px',
              top: '50%',
              transform: 'translateY(-50%)',
              background: searchQuery.trim() && !isLoading
                ? 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)'
                : 'rgba(255, 255, 255, 0.05)',
              border: 'none',
              borderRadius: '6px',
              padding: '6px 16px',
              fontSize: '12px',
              fontWeight: '600',
              color: searchQuery.trim() && !isLoading ? '#ffffff' : 'rgba(255, 255, 255, 0.3)',
              fontFamily: 'inherit',
              letterSpacing: '-0.01em',
              cursor: searchQuery.trim() && !isLoading ? 'pointer' : 'not-allowed',
              transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
              textTransform: 'none',
              opacity: searchQuery.trim() && !isLoading ? '1' : '0.5',
              height: '32px'
            }}
          >
            {isLoading ? 'Searching...' : 'Search'}
          </button>
        </div>

        {/* Suggestions Dropdown */}
        {showSuggestions && filteredSuggestions.length > 0 && (
          <div style={{
            position: 'absolute',
            top: '100%',
            left: 0,
            right: 0,
            background: 'rgba(0, 0, 0, 0.9)',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            borderRadius: '8px',
            marginTop: '4px',
            zIndex: 1000,
            maxHeight: '200px',
            overflowY: 'auto',
            backdropFilter: 'blur(20px)',
            WebkitBackdropFilter: 'blur(20px)'
          }}>
            {filteredSuggestions.map((domain) => (
              <div
                key={domain.id}
                onClick={() => handleSuggestionClick(domain)}
                style={{
                  padding: '12px 16px',
                  cursor: 'pointer',
                  borderBottom: '1px solid rgba(255, 255, 255, 0.05)',
                  transition: 'background-color 0.2s ease'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.1)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'transparent';
                }}
              >
                <div style={{
                  fontSize: '14px',
                  fontWeight: '600',
                  color: '#ffffff',
                  marginBottom: '4px'
                }}>
                  {domain.name}
                </div>
                <div style={{
                  fontSize: '12px',
                  color: 'rgba(255, 255, 255, 0.6)',
                  marginBottom: '2px'
                }}>
                  {domain.category}
                </div>
                <div style={{
                  fontSize: '11px',
                  color: 'rgba(255, 255, 255, 0.4)'
                }}>
                  {domain.keywords.slice(0, 3).join(', ')}
                </div>
              </div>
            ))}
          </div>
        )}
      </form>

      {/* Error Message */}
      {error && (
        <div style={{
          marginTop: '16px',
          padding: '12px 16px',
          background: 'rgba(255, 0, 0, 0.1)',
          border: '1px solid rgba(255, 0, 0, 0.3)',
          borderRadius: '8px',
          color: '#ff6b6b',
          fontSize: '14px'
        }}>
          {error}
        </div>
      )}

    </div>
  );
};

export default ExpertSearch;
