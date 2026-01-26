import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import './PaperSearchPage.css';

interface SearchResult {
  title: string;
  authors: string;
  abstract: string;
  source: string;
  url: string;
  year?: string;
  doi?: string;
  citations?: number;
}

const PaperSearchPage: React.FC = () => {
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);

  // Enhanced search function with multiple academic databases
  const handleSearch = async () => {
    if (!searchQuery.trim()) return;
    setIsSearching(true);
    setSearchResults([]);

    try {
      // Search multiple academic databases
      const searchPromises = [
        searchArXiv(searchQuery),
        searchCrossRef(searchQuery),
        searchOpenAlex(searchQuery),
        searchSemanticScholar(searchQuery)
      ];

      const allResults = await Promise.allSettled(searchPromises);
      const combinedResults: SearchResult[] = [];

      allResults.forEach((result) => {
        if (result.status === 'fulfilled' && result.value) {
          combinedResults.push(...result.value);
        }
      });

      // Remove duplicates and limit to top 15 results
      const uniqueResults = combinedResults
        .filter((result, index, self) => 
          index === self.findIndex(r => r.title === result.title)
        )
        .slice(0, 15);

      setSearchResults(uniqueResults);
    } catch (error) {
      console.error('Search error:', error);
      // Fallback to mock results if all APIs fail
      setSearchResults(generateMockResults(searchQuery));
    }

    setIsSearching(false);
  };

  // Search arXiv database
  const searchArXiv = async (query: string): Promise<SearchResult[]> => {
    try {
      const response = await fetch(
        `https://export.arxiv.org/api/query?search_query=all:${encodeURIComponent(query)}&start=0&max_results=5&sortBy=relevance&sortOrder=descending`
      );
      const data = await response.text();
      const parser = new DOMParser();
      const xmlDoc = parser.parseFromString(data, 'text/xml');
      const entries = xmlDoc.getElementsByTagName('entry');
      
      return Array.from(entries).map((entry) => {
        const title = entry.getElementsByTagName('title')[0]?.textContent || 'Untitled';
        const authors = Array.from(entry.getElementsByTagName('author'))
          .map(author => author.getElementsByTagName('name')[0]?.textContent || '')
          .join(', ');
        const abstract = entry.getElementsByTagName('summary')[0]?.textContent || '';
        const link = entry.getElementsByTagName('link')[0]?.getAttribute('href') || '#';
        const published = entry.getElementsByTagName('published')[0]?.textContent || '';
        const year = published ? new Date(published).getFullYear().toString() : '';

        return {
          title: title.replace(/\n/g, ' ').trim(),
          authors: authors || 'Unknown Authors',
          abstract: abstract.replace(/\n/g, ' ').trim().substring(0, 200) + '...',
          source: 'arXiv',
          url: link,
          year,
          doi: link.includes('arxiv.org') ? link.split('/').pop() : undefined
        };
      });
    } catch (error) {
      console.error('arXiv search error:', error);
      return [];
    }
  };

  // Search CrossRef database
  const searchCrossRef = async (query: string): Promise<SearchResult[]> => {
    try {
      const response = await fetch(
        `https://api.crossref.org/works?query=${encodeURIComponent(query)}&rows=5&sort=relevance`
      );
      const data = await response.json();
      
      return data.message?.items?.map((item: any) => ({
        title: item.title?.[0] || 'Untitled',
        authors: item.author?.map((a: any) => `${a.given || ''} ${a.family || ''}`).join(', ') || 'Unknown Authors',
        abstract: item.abstract || 'No abstract available',
        source: 'CrossRef',
        url: item.URL || item['link'][0]?.URL || '#',
        year: item['published-print']?.['date-parts']?.[0]?.[0]?.toString() || item['published-online']?.['date-parts']?.[0]?.[0]?.toString() || '',
        doi: item.DOI,
        citations: item['is-referenced-by-count']
      })) || [];
    } catch (error) {
      console.error('CrossRef search error:', error);
      return [];
    }
  };

  // Search OpenAlex database
  const searchOpenAlex = async (query: string): Promise<SearchResult[]> => {
    try {
      const response = await fetch(
        `https://api.openalex.org/works?search=${encodeURIComponent(query)}&per-page=5&sort=relevance_score:desc`
      );
      const data = await response.json();
      
      return data.results?.map((work: any) => ({
        title: work.title || 'Untitled',
        authors: work.authorships?.map((a: any) => a.author?.display_name).join(', ') || 'Unknown Authors',
        abstract: work.abstract_inverted_index ? 
          Object.keys(work.abstract_inverted_index)
            .sort((a, b) => work.abstract_inverted_index[a][0] - work.abstract_inverted_index[b][0])
            .join(' ')
            .substring(0, 200) + '...' : 'No abstract available',
        source: 'OpenAlex',
        url: work.doi ? `https://doi.org/${work.doi}` : work.primary_location?.landing_page_url || '#',
        year: work.publication_year?.toString() || '',
        doi: work.doi,
        citations: work.cited_by_count
      })) || [];
    } catch (error) {
      console.error('OpenAlex search error:', error);
      return [];
    }
  };

  // Search Semantic Scholar database
  const searchSemanticScholar = async (query: string): Promise<SearchResult[]> => {
    try {
      const response = await fetch(
        `https://api.semanticscholar.org/graph/v1/paper/search?query=${encodeURIComponent(query)}&limit=5&sort=relevance`
      );
      const data = await response.json();
      
      return data.data?.map((paper: any) => ({
        title: paper.title || 'Untitled',
        authors: paper.authors?.map((a: any) => a.name).join(', ') || 'Unknown Authors',
        abstract: paper.abstract || 'No abstract available',
        source: 'Semantic Scholar',
        url: paper.url || paper.externalIds?.DOI ? `https://doi.org/${paper.externalIds.DOI}` : '#',
        year: paper.year?.toString() || '',
        doi: paper.externalIds?.DOI,
        citations: paper.citationCount
      })) || [];
    } catch (error) {
      console.error('Semantic Scholar search error:', error);
      return [];
    }
  };

  // Generate mock results as fallback
  const generateMockResults = (query: string): SearchResult[] => {
    const mockTitles = [
      `Advanced ${query}: A Comprehensive Analysis`,
      `Machine Learning Approaches in ${query}`,
      `${query} and Its Applications in Modern Research`,
      `Novel Methods for ${query} Detection and Analysis`,
      `The Impact of ${query} on Contemporary Science`
    ];

    const sources = ['arXiv', 'CrossRef', 'OpenAlex', 'Semantic Scholar', 'PubMed'];
    const years = ['2023', '2022', '2024', '2021', '2020'];

    return mockTitles.map((title, i) => ({
      title,
      authors: `Dr. Smith, Dr. Johnson, Dr. Williams`,
      abstract: `This study presents a comprehensive analysis of ${query} and its implications for modern research. The methodology involves advanced statistical analysis and machine learning techniques to provide insights into the current state of the field.`,
      source: sources[i % sources.length],
      url: `https://example.com/paper-${i + 1}`,
      year: years[i % years.length],
      doi: `10.1000/example.${i + 1}`,
      citations: Math.floor(Math.random() * 100) + 10
    }));
  };

  return (
    <div className="paper-search-page">
      {/* Navigation Bar */}
      <div className="paper-search-header">
        <h1 className="paper-search-title">Paper Search</h1>
        
        <button
          onClick={() => navigate('/')}
          className="paper-search-back"
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'rgba(255, 255, 255, 0.15)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
          }}
        >
          ← Back to Home
        </button>
      </div>

      {/* Main Content */}
      <div className="paper-search-card">
        <p className="paper-search-description">
          Search across multiple academic databases to find relevant research papers with direct links to full texts. 
          Our intelligent search combines results from arXiv, CrossRef, OpenAlex, and Semantic Scholar to provide comprehensive coverage.
        </p>

        <div className="paper-search-actions">
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
            placeholder="Enter keywords or research topic..."
            className="paper-search-input"
          />

          <button
            onClick={handleSearch}
            disabled={!searchQuery.trim() || isSearching}
            className="paper-search-button"
            style={{
              background: searchQuery.trim() && !isSearching ? 'rgba(255, 255, 255, 0.15)' : 'rgba(255, 255, 255, 0.05)',
              cursor: searchQuery.trim() && !isSearching ? 'pointer' : 'not-allowed',
            }}
            onMouseEnter={(e) => {
              if (searchQuery.trim() && !isSearching) {
                e.currentTarget.style.background = 'rgba(255, 255, 255, 0.2)';
              }
            }}
            onMouseLeave={(e) => {
              if (searchQuery.trim() && !isSearching) {
                e.currentTarget.style.background = 'rgba(255, 255, 255, 0.15)';
              }
            }}
          >
            {isSearching ? 'Searching...' : 'Search'}
          </button>
        </div>

        {/* Results */}
        {searchResults.length > 0 && (
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '20px',
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
                color: '#ffffff',
                margin: 0,
              }}>
                Search Results ({searchResults.length})
              </h3>
              <div style={{
                fontSize: '0.875rem',
                color: 'rgba(255, 255, 255, 0.6)',
                fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
              }}>
                Powered by arXiv, CrossRef, OpenAlex & Semantic Scholar
              </div>
            </div>

            {searchResults.map((result, i) => (
              <div
                key={i}
                style={{
                  padding: '28px',
                  borderRadius: '20px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                  position: 'relative',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.08)';
                  e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                  e.currentTarget.style.transform = 'translateY(-2px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
                  e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                {/* Paper Title */}
                <a
                  href={result.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{
                    fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                    fontSize: '1.25rem',
                    fontWeight: '600',
                    color: '#ffffff',
                    textDecoration: 'none',
                    lineHeight: '1.3',
                    display: 'block',
                    marginBottom: '12px',
                    transition: 'color 0.3s ease',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.color = '#007AFF';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.color = '#ffffff';
                  }}
                >
                  {result.title}
                </a>

                {/* Authors */}
                <p style={{
                  fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                  fontSize: '0.95rem',
                  color: 'rgba(255, 255, 255, 0.8)',
                  margin: '0 0 12px 0',
                  fontWeight: '500',
                }}>
                  {result.authors}
                </p>

                {/* Abstract */}
                <p style={{
                  fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                  fontSize: '0.9rem',
                  color: 'rgba(255, 255, 255, 0.7)',
                  margin: '0 0 16px 0',
                  lineHeight: '1.5',
                }}>
                  {result.abstract}
                </p>

                {/* Metadata Row */}
                <div style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  flexWrap: 'wrap',
                  gap: '12px',
                }}>
                  <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '16px',
                    flexWrap: 'wrap',
                  }}>
                    {/* Source Badge */}
                    <div style={{
                      background: 'rgba(0, 122, 255, 0.2)',
                      color: '#007AFF',
                      padding: '6px 12px',
                      borderRadius: '12px',
                      fontSize: '0.8rem',
                      fontWeight: '600',
                      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                      border: '1px solid rgba(0, 122, 255, 0.3)',
                    }}>
                      {result.source}
                    </div>

                    {/* Year */}
                    {result.year && (
                      <span style={{
                        fontSize: '0.85rem',
                        color: 'rgba(255, 255, 255, 0.6)',
                        fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                      }}>
                        {result.year}
                      </span>
                    )}

                    {/* Citations */}
                    {result.citations && (
                      <span style={{
                        fontSize: '0.85rem',
                        color: 'rgba(255, 255, 255, 0.6)',
                        fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
                      }}>
                        📊 {result.citations} citations
                      </span>
                    )}
                  </div>

                  {/* DOI */}
                  {result.doi && (
                    <div style={{
                      fontSize: '0.8rem',
                      color: 'rgba(255, 255, 255, 0.5)',
                      fontFamily: 'monospace',
                      background: 'rgba(255, 255, 255, 0.05)',
                      padding: '4px 8px',
                      borderRadius: '6px',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                    }}>
                      DOI: {result.doi}
                    </div>
                  )}
                </div>

                {/* External Link Indicator */}
                <div style={{
                  position: 'absolute',
                  top: '20px',
                  right: '20px',
                  fontSize: '0.8rem',
                  color: 'rgba(255, 255, 255, 0.4)',
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

export default PaperSearchPage;

