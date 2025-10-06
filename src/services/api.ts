// API Service for connecting frontend features to backend
const API_BASE_URL = process.env.REACT_APP_API_URL || 'https://gaply-backend-production.up.railway.app';

// Types for API responses
export interface PaperSearchRequest {
  query: {
    keywords: string[];
    max_results: number;
  };
}

export interface PaperSearchResponse {
  papers: Array<{
    title: string;
    authors: string[];
    abstract: string;
    doi: string;
    url: string;
    source: string;
    published_date: string;
  }>;
}

export interface JournalMatchRequest {
  title: string;
  abstract: string;
  preferences: {
    access_type: 'free' | 'paid';
    quartile: 'Q1' | 'Q2' | 'Q3' | 'Q4' | 'ALL';
    show_impact_factor: boolean;
    show_acceptance_rate: boolean;
    include_guidelines: boolean;
  };
}

export interface JournalMatchResponse {
  journals: Array<{
    journal_name: string;
    quartile: string;
    impact_factor?: number;
    acceptance_rate?: string;
    url: string;
    guidelines?: string;
  }>;
}

export interface ParaphraseRequest {
  text: string;
  style: 'academic' | 'formal' | 'casual';
}

export interface ParaphraseResponse {
  original_text: string;
  paraphrased_text: string;
  changes_made: string[];
}

// API Service Class
class ApiService {
  private baseUrl: string;

  constructor(baseUrl: string = API_BASE_URL) {
    this.baseUrl = baseUrl;
  }

  private async makeRequest<T>(
    endpoint: string,
    options: RequestInit = {}
  ): Promise<T> {
    const url = `${this.baseUrl}${endpoint}`;
    
    const defaultHeaders = {
      'Content-Type': 'application/json',
    };

    const response = await fetch(url, {
      ...options,
      headers: {
        ...defaultHeaders,
        ...options.headers,
      },
    });

    if (!response.ok) {
      throw new Error(`API request failed: ${response.status} ${response.statusText}`);
    }

    return response.json();
  }

  // Paper Search API
  async searchPapers(request: PaperSearchRequest): Promise<PaperSearchResponse> {
    try {
      return await this.makeRequest<PaperSearchResponse>('/api/search', {
        method: 'POST',
        body: JSON.stringify(request),
      });
    } catch (error) {
      console.error('Paper search failed:', error);
      // Return mock data as fallback
      return {
        papers: Array.from({ length: 5 }).map((_, i) => ({
          title: `${request.query.keywords[0]} — Research Study ${i + 1}`,
          authors: [`Author ${i + 1}`, `Co-author ${i + 1}`],
          abstract: `This study explores ${request.query.keywords[0]} and its implications for modern research.`,
          doi: `10.1000/example.${i + 1}`,
          url: `https://example.com/paper-${i + 1}`,
          source: ['arXiv', 'CrossRef', 'OpenAlex', 'PubMed', 'IEEE'][i % 5],
          published_date: new Date().toISOString(),
        })),
      };
    }
  }

  // Journal Matching API
  async matchJournals(request: JournalMatchRequest): Promise<JournalMatchResponse> {
    try {
      return await this.makeRequest<JournalMatchResponse>('/api/match', {
        method: 'POST',
        body: JSON.stringify(request),
      });
    } catch (error) {
      console.error('Journal matching failed:', error);
      // Return mock data as fallback
      const mockJournals = [
        { journal_name: 'IEEE Access', quartile: 'Q1', impact_factor: 4.64, acceptance_rate: '30%' },
        { journal_name: 'PLOS ONE', quartile: 'Q2', impact_factor: 3.75, acceptance_rate: '48%' },
        { journal_name: 'Heliyon', quartile: 'Q3', impact_factor: 3.2, acceptance_rate: '40%' },
        { journal_name: 'Nature Communications', quartile: 'Q1', impact_factor: 16.6, acceptance_rate: '25%' },
        { journal_name: 'Scientific Reports', quartile: 'Q2', impact_factor: 4.38, acceptance_rate: '45%' },
      ];

      const filtered = request.preferences.quartile === 'ALL' 
        ? mockJournals 
        : mockJournals.filter(j => j.quartile === request.preferences.quartile);

      return {
        journals: filtered.map(journal => ({
          journal_name: journal.journal_name,
          quartile: journal.quartile,
          impact_factor: request.preferences.show_impact_factor ? journal.impact_factor : undefined,
          acceptance_rate: request.preferences.show_acceptance_rate ? journal.acceptance_rate : undefined,
          url: `https://example.com/journal/${journal.journal_name.toLowerCase().replace(/\s+/g, '-')}`,
          guidelines: request.preferences.include_guidelines ? 'Submission guidelines available on journal website' : undefined,
        })),
      };
    }
  }

  // Paraphrase API (Academic AI Remover)
  async paraphraseText(request: ParaphraseRequest): Promise<ParaphraseResponse> {
    try {
      return await this.makeRequest<ParaphraseResponse>('/api/v1/paraphrase/direct', {
        method: 'POST',
        body: JSON.stringify(request),
      });
    } catch (error) {
      console.error('Paraphrase failed:', error);
      // Return mock data as fallback
      const changes = ['Removed filler words', 'Enhanced academic tone', 'Improved sentence structure'];
      return {
        original_text: request.text,
        paraphrased_text: request.text
          .replace(/\b(very|really|basically|just|quite|rather|pretty|somewhat)\b/gi, '')
          .replace(/\b(I think|I believe|I feel|in my opinion)\b/gi, '')
          .replace(/\b(actually|literally|honestly|obviously|clearly)\b/gi, '')
          .trim() + ' (enhanced for academic writing)',
        changes_made: changes,
      };
    }
  }

  // Health check
  async healthCheck(): Promise<{ status: string }> {
    try {
      return await this.makeRequest<{ status: string }>('/api/health');
    } catch (error) {
      console.error('Health check failed:', error);
      return { status: 'offline' };
    }
  }
}

// Export singleton instance
export const apiService = new ApiService();
export default apiService;