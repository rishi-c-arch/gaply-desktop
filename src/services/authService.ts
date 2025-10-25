import { API_BASE_URL, apiFetch } from '../api/config';

export interface User {
  id: string;
  email: string;
  first_name?: string;
  last_name?: string;
  is_premium: boolean;
  premium_expires_at?: string;
  created_at: string;
  last_login_at?: string;
}

export interface LoginCredentials {
  email: string;
  password: string;
}

export interface RegisterCredentials {
  email: string;
  password: string;
  first_name: string;
  last_name: string;
}

export interface AuthResponse {
  success: boolean;
  data?: {
    token: string;
    user: User;
  };
  error?: string;
}

export interface SubscriptionSummary {
  package_id: string;
  package_name: string;
  purchased_at: string;
  expires_at?: string;
  remaining_uses: {
    gap_finder?: number;
    deep_eval?: number;
    support?: boolean;
  };
  status: string;
}

export interface FeatureAccess {
  has_access: boolean;
  remaining_uses: number;
  subscription_id?: string;
  expires_at?: string;
}

class AuthService {
  private token: string | null = null;

  constructor() {
    // Initialize token from localStorage
    this.token = localStorage.getItem('authToken');
  }

  // Set authentication token
  setToken(token: string) {
    this.token = token;
    localStorage.setItem('authToken', token);
  }

  // Get current token
  getToken(): string | null {
    return this.token || localStorage.getItem('authToken');
  }

  // Clear authentication data
  clearAuth() {
    this.token = null;
    localStorage.removeItem('authToken');
    localStorage.removeItem('user');
  }

  // Get authorization headers
  private getAuthHeaders(): HeadersInit {
    const token = this.getToken();
    return {
      'Content-Type': 'application/json',
      ...(token && { Authorization: `Bearer ${token}` }),
    };
  }

  // Register new user
  async register(credentials: RegisterCredentials): Promise<AuthResponse> {
    try {
      const response = await apiFetch('/api/premium/signup', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: credentials.email,
          password: credentials.password,
          first_name: credentials.first_name,
          last_name: credentials.last_name,
        }),
      });

      const data = await response.json();
      console.log('Registration API response:', data); // Debug log

      if (response.ok && data.success && data.token && data.user) {
        this.setToken(data.token);
        localStorage.setItem('user', JSON.stringify(data.user));
        localStorage.setItem('user_email', data.user.email); // Store email for test account detection
        return {
          success: true,
          data: { token: data.token, user: data.user },
        };
      } else {
        return {
          success: false,
          error: data.error || data.message || 'Registration failed',
        };
      }
    } catch (error) {
      console.error('Registration error:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Registration failed',
      };
    }
  }

  // Login user
  async login(credentials: LoginCredentials): Promise<AuthResponse> {
    try {
      const response = await apiFetch('/api/premium/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: credentials.email,
          password: credentials.password,
        }),
      });

      const data = await response.json();
      console.log('Login API response:', data); // Debug log

      if (response.ok && data.success && data.token && data.user) {
        this.setToken(data.token);
        localStorage.setItem('user', JSON.stringify(data.user));
        localStorage.setItem('user_email', data.user.email); // Store email for test account detection
        return {
          success: true,
          data: { token: data.token, user: data.user },
        };
      } else {
        return {
          success: false,
          error: data.error || data.message || 'Login failed',
        };
      }
    } catch (error) {
      console.error('Login error:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Login failed',
      };
    }
  }

  // Logout user
  async logout(): Promise<void> {
    try {
      const token = this.getToken();
      if (token) {
        await apiFetch('/api/premium/logout', {
          method: 'POST',
          headers: this.getAuthHeaders(),
        });
      }
    } catch (error) {
      console.error('Logout error:', error);
    } finally {
      this.clearAuth();
    }
  }

  // Verify token and get user data
  async verifyToken(): Promise<AuthResponse> {
    try {
      const token = this.getToken();
      if (!token) {
        return { success: false, error: 'No token found' };
      }

      // Extract email from token (format: token_email_timestamp)
      const parts = token.split('_');
      if (parts.length < 3) {
        return { success: false, error: 'Invalid token format' };
      }
      const email = parts[1];

      const response = await apiFetch(`/api/premium/status?email=${encodeURIComponent(email)}`, {
        method: 'GET',
        headers: { 'Content-Type': 'application/json' },
      });

      const data = await response.json();

      if (response.ok && data.success) {
        localStorage.setItem('user', JSON.stringify(data));
        return {
          success: true,
          data: { token, user: data },
        };
      } else {
        this.clearAuth();
        return {
          success: false,
          error: data.error || 'Token verification failed',
        };
      }
    } catch (error) {
      console.error('Token verification error:', error);
      this.clearAuth();
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Token verification failed',
      };
    }
  }

  // Get user subscription summary
  async getUserSubscription(userId: string): Promise<SubscriptionSummary | null> {
    try {
      const response = await apiFetch(`/api/premium/subscription/${userId}`, {
        method: 'GET',
        headers: this.getAuthHeaders(),
      });

      const data = await response.json();

      if (response.ok && data.success) {
        return data.data;
      } else {
        return null;
      }
    } catch (error) {
      console.error('Get subscription error:', error);
      return null;
    }
  }

  // Check feature access
  async checkFeatureAccess(userId: string, featureType: string): Promise<FeatureAccess> {
    try {
      const response = await apiFetch(`/api/premium/check-access/${userId}/${featureType}`, {
        method: 'GET',
        headers: this.getAuthHeaders(),
      });

      const data = await response.json();

      if (response.ok && data.success) {
        return data.data;
      } else {
        return {
          has_access: false,
          remaining_uses: 0,
        };
      }
    } catch (error) {
      console.error('Check feature access error:', error);
      return {
        has_access: false,
        remaining_uses: 0,
      };
    }
  }

  // Use a feature (decrement usage)
  async useFeature(userId: string, featureType: string, jobId?: string): Promise<boolean> {
    try {
      const response = await apiFetch(`/api/premium/use-feature/${userId}/${featureType}`, {
        method: 'POST',
        headers: this.getAuthHeaders(),
        body: JSON.stringify({ job_id: jobId }),
      });

      const data = await response.json();
      return response.ok && data.success;
    } catch (error) {
      console.error('Use feature error:', error);
      return false;
    }
  }

  // Get current user from localStorage
  getCurrentUser(): User | null {
    try {
      const userStr = localStorage.getItem('user');
      return userStr ? JSON.parse(userStr) : null;
    } catch (error) {
      console.error('Error parsing user data:', error);
      return null;
    }
  }

  // Check if user is authenticated
  isAuthenticated(): boolean {
    return !!(this.getToken() && this.getCurrentUser());
  }

  // Check if user has premium access
  isPremium(): boolean {
    const user = this.getCurrentUser();
    if (!user || !user.is_premium) return false;
    
    if (user.premium_expires_at) {
      return new Date(user.premium_expires_at) > new Date();
    }
    
    return true;
  }
}

// Create singleton instance
export const authService = new AuthService();
export default authService;
