// API service for expert search functionality
import prakharImage from '../assets/images/P.jpeg';
import anuragImage from '../assets/images/anu.png';
import piyushImage from '../assets/images/ch.png';

const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:8080';

export interface ExpertDomain {
  id: number;
  name: string;
  category: string;
  keywords: string[];
  description: string;
  created_at: string;
}

export interface ExpertProfile {
  id: number;
  name: string;
  email: string;
  domains: string[];
  experience: number;
  publications: number;
  patents: number;
  bio: string;
  image_url: string;
  is_available: boolean;
  created_at: string;
}

export interface SearchResponse {
  query: string;
  suggested_domains: ExpertDomain[];
  expert_profiles: ExpertProfile[];
  total_results: number;
}

export interface ContactExpertRequest {
  expert_id: number;
  subject: string;
  message: string;
  user_email: string;
  user_name: string;
}

class ExpertSearchAPI {
  private baseURL: string;

  constructor(baseURL: string = API_BASE_URL) {
    this.baseURL = baseURL;
  }

  // Search experts and domains
  async searchExperts(query: string): Promise<SearchResponse> {
    const response = await fetch(`${this.baseURL}/api/expert/search`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ query }),
    });

    if (!response.ok) {
      throw new Error(`Search failed: ${response.statusText}`);
    }

    return response.json();
  }

  // Get all domains
  async getAllDomains(): Promise<ExpertDomain[]> {
    const response = await fetch(`${this.baseURL}/api/expert/domains`);
    
    if (!response.ok) {
      throw new Error(`Failed to get domains: ${response.statusText}`);
    }

    const data = await response.json();
    return data.domains || [];
  }

  // Get domain categories
  async getDomainCategories(): Promise<string[]> {
    const response = await fetch(`${this.baseURL}/api/expert/categories`);
    
    if (!response.ok) {
      throw new Error(`Failed to get categories: ${response.statusText}`);
    }

    const data = await response.json();
    return data.categories || [];
  }

  // Get experts by domain
  async getExpertsByDomain(domain: string): Promise<ExpertProfile[]> {
    const response = await fetch(`${this.baseURL}/api/expert/domain/${encodeURIComponent(domain)}`);
    
    if (!response.ok) {
      throw new Error(`Failed to get experts by domain: ${response.statusText}`);
    }

    const data = await response.json();
    return data.experts || [];
  }

  // Get expert profile by ID
  async getExpertProfile(id: number): Promise<ExpertProfile> {
    const response = await fetch(`${this.baseURL}/api/expert/profile/${id}`);
    
    if (!response.ok) {
      throw new Error(`Failed to get expert profile: ${response.statusText}`);
    }

    const data = await response.json();
    return data.expert;
  }

  // Contact expert
  async contactExpert(request: ContactExpertRequest, token: string): Promise<void> {
    const response = await fetch(`${this.baseURL}/api/expert/contact`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`,
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error(`Failed to contact expert: ${response.statusText}`);
    }
  }

  // Public search (limited results)
  async publicSearchExperts(query: string): Promise<SearchResponse> {
    const response = await fetch(`${this.baseURL}/api/public/expert/search`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ query }),
    });

    if (!response.ok) {
      throw new Error(`Public search failed: ${response.statusText}`);
    }

    return response.json();
  }

  // Get featured experts
  async getFeaturedExperts(): Promise<ExpertProfile[]> {
    const response = await fetch(`${this.baseURL}/api/public/expert/featured`);
    
    if (!response.ok) {
      throw new Error(`Failed to get featured experts: ${response.statusText}`);
    }

    const data = await response.json();
    return data.featured_experts || [];
  }

  // Get domain suggestions
  async getDomainSuggestions(): Promise<ExpertDomain[]> {
    const response = await fetch(`${this.baseURL}/api/public/expert/suggestions`);
    
    if (!response.ok) {
      throw new Error(`Failed to get domain suggestions: ${response.statusText}`);
    }

    const data = await response.json();
    return data.suggestions || [];
  }

  // Enhanced search with fuzzy matching
  async searchExpertsFuzzy(query: string): Promise<SearchResponse> {
    const response = await fetch(`${this.baseURL}/api/admin/expert/search/fuzzy?q=${encodeURIComponent(query)}`, {
      headers: {
        'Authorization': `Bearer ${localStorage.getItem('admin_token') || 'demo_token'}`
      }
    });

    if (!response.ok) {
      throw new Error(`Fuzzy search failed: ${response.statusText}`);
    }

    const data = await response.json();
    return {
      query: data.query,
      suggested_domains: [], // Fuzzy search focuses on experts
      expert_profiles: data.experts || [],
      total_results: data.total || 0
    };
  }

  // Search experts by domain match
  async searchExpertsByDomain(domain: string): Promise<ExpertProfile[]> {
    const response = await fetch(`${this.baseURL}/api/admin/expert/search/domain-match?domain=${encodeURIComponent(domain)}`, {
      headers: {
        'Authorization': `Bearer ${localStorage.getItem('admin_token') || 'demo_token'}`
      }
    });

    if (!response.ok) {
      throw new Error(`Domain search failed: ${response.statusText}`);
    }

    const data = await response.json();
    return data.experts || [];
  }

  // Get recently created experts
  async getRecentlyCreatedExperts(): Promise<ExpertProfile[]> {
    const response = await fetch(`${this.baseURL}/api/admin/expert/recent`, {
      headers: {
        'Authorization': `Bearer ${localStorage.getItem('admin_token') || 'demo_token'}`
      }
    });

    if (!response.ok) {
      throw new Error(`Failed to get recent experts: ${response.statusText}`);
    }

    const data = await response.json();
    return data.recent_experts || [];
  }

  // Refresh search index
  async refreshSearchIndex(): Promise<void> {
    const response = await fetch(`${this.baseURL}/api/admin/expert/refresh-index`, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${localStorage.getItem('admin_token') || 'demo_token'}`
      }
    });

    if (!response.ok) {
      throw new Error(`Failed to refresh search index: ${response.statusText}`);
    }
  }
}

// Create singleton instance
export const expertSearchAPI = new ExpertSearchAPI();

// Utility functions for common operations
export const searchExpertsByKeyword = async (keyword: string): Promise<SearchResponse> => {
  return expertSearchAPI.searchExperts(keyword);
};

export const getPopularDomains = async (): Promise<ExpertDomain[]> => {
  return expertSearchAPI.getDomainSuggestions();
};

export const getFeaturedExpertProfiles = async (): Promise<ExpertProfile[]> => {
  return expertSearchAPI.getFeaturedExperts();
};

// Mock data for development/testing
export const mockExpertDomains: ExpertDomain[] = [
  {
    id: 1,
    name: 'SPSS Analyst',
    category: 'Data Analysis & Statistics',
    keywords: ['spss', 'statistical analysis', 'data analysis', 'statistics', 'quantitative research'],
    description: 'Expert in SPSS software for statistical analysis and data interpretation',
    created_at: new Date().toISOString()
  },
  {
    id: 2,
    name: 'Machine Learning / AI Engineer',
    category: 'Technology & Engineering',
    keywords: ['machine learning', 'artificial intelligence', 'deep learning', 'neural networks', 'ai algorithms'],
    description: 'Specialist in machine learning and artificial intelligence',
    created_at: new Date().toISOString()
  },
  {
    id: 3,
    name: 'Medical / Scientific Writer',
    category: 'Research & Writing',
    keywords: ['medical writing', 'scientific writing', 'research papers', 'manuscripts', 'publications'],
    description: 'Professional medical and scientific writing specialist',
    created_at: new Date().toISOString()
  }
];

// Admin-created experts (only appear in search results, not on front page)
export const mockAdminExpertProfiles: ExpertProfile[] = [
  {
    id: 101,
    name: 'Dr. Sarah Johnson',
    email: 'sarah.johnson@example.com',
    domains: ['Data Analysis & Statistics', 'SPSS Analyst', 'Statistical Modeling'],
    experience: 8,
    publications: 25,
    patents: 2,
    bio: 'Expert statistician with 8 years of experience in SPSS analysis and statistical modeling for medical research.',
    image_url: 'sarah.jpg',
    is_available: true,
    created_at: new Date().toISOString()
  },
  {
    id: 102,
    name: 'Dr. Michael Chen',
    email: 'michael.chen@example.com',
    domains: ['Technology & Engineering', 'Machine Learning / AI Engineer', 'Data Science'],
    experience: 10,
    publications: 18,
    patents: 5,
    bio: 'AI researcher and machine learning engineer specializing in deep learning and neural networks.',
    image_url: 'michael.jpg',
    is_available: true,
    created_at: new Date().toISOString()
  },
  {
    id: 103,
    name: 'Dr. Emily Rodriguez',
    email: 'emily.rodriguez@example.com',
    domains: ['Research & Writing', 'Medical / Scientific Writer', 'Academic Editing'],
    experience: 6,
    publications: 30,
    patents: 0,
    bio: 'Professional medical writer with extensive experience in academic publications and manuscript editing.',
    image_url: 'emily.jpg',
    is_available: true,
    created_at: new Date().toISOString()
  },
  {
    id: 104,
    name: 'Dr. James Wilson',
    email: 'james.wilson@example.com',
    domains: ['Health & Medicine', 'Public Health & Epidemiology', 'Clinical Research'],
    experience: 12,
    publications: 35,
    patents: 1,
    bio: 'Public health expert with specialization in epidemiology and clinical trial design.',
    image_url: 'james.jpg',
    is_available: true,
    created_at: new Date().toISOString()
  },
  {
    id: 105,
    name: 'Dr. Lisa Thompson',
    email: 'lisa.thompson@example.com',
    domains: ['Business & Economics', 'Economics & Finance', 'Data Analysis'],
    experience: 9,
    publications: 22,
    patents: 3,
    bio: 'Economist and financial analyst with expertise in economic modeling and policy analysis.',
    image_url: 'lisa.jpg',
    is_available: true,
    created_at: new Date().toISOString()
  }
];

// Front page experts (hardcoded, don't appear in search)
export const frontPageExpertProfiles: ExpertProfile[] = [
  {
    id: 1,
    name: 'Prakhar Singh',
    email: 'prakhar@example.com',
    domains: ['Finance & Research Expert', 'Econometrician'],
    experience: 5,
    publications: 5,
    patents: 0,
    bio: 'Finance & Research Expert. PhD Finance | 5 yrs | 5 papers.',
    image_url: prakharImage,
    is_available: true,
    created_at: new Date().toISOString()
  },
  {
    id: 2,
    name: 'Anurag Singh',
    email: 'anurag@example.com',
    domains: ['Computer Scientist', 'Machine Learning Engineer'],
    experience: 10,
    publications: 2,
    patents: 1,
    bio: 'Computer Scientist. PhD CS | 10 yrs | 2 papers | 1 patent.',
    image_url: anuragImage,
    is_available: true,
    created_at: new Date().toISOString()
  },
  {
    id: 3,
    name: 'Piyush Anand',
    email: 'piyush@example.com',
    domains: ['ML & XAI Expert', 'Data Analyst / EDA Specialist'],
    experience: 10,
    publications: 6,
    patents: 0,
    bio: 'ML & XAI Expert. PhD ML | 10 yrs | 6 papers.',
    image_url: piyushImage,
    is_available: true,
    created_at: new Date().toISOString()
  }
];

// Legacy export for backward compatibility (admin-created experts only)
export const mockExpertProfiles = mockAdminExpertProfiles;

// Fallback function for when backend is not available
export const getMockSearchResults = (query: string): SearchResponse => {
  const filteredDomains = mockExpertDomains.filter(domain =>
    domain.name.toLowerCase().includes(query.toLowerCase()) ||
    domain.keywords.some(keyword => keyword.toLowerCase().includes(query.toLowerCase()))
  );

  // Only use admin-created experts for search results (not front page experts)
  const filteredExperts = mockAdminExpertProfiles.filter(expert =>
    expert.name.toLowerCase().includes(query.toLowerCase()) ||
    expert.bio.toLowerCase().includes(query.toLowerCase()) ||
    expert.domains.some(domain => domain.toLowerCase().includes(query.toLowerCase()))
  );

  return {
    query,
    suggested_domains: filteredDomains,
    expert_profiles: filteredExperts,
    total_results: filteredExperts.length
  };
};

export default expertSearchAPI;
