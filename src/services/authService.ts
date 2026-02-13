import { apiFetch } from '../api/config';

export interface User {
  id: string;
  email: string;
  first_name?: string;
  last_name?: string;
  role?: string;
  created_at: string;
  last_login_at?: string;
  is_premium?: boolean;
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

  // Clear authentication data (full logout - clears all auth keys for consistency)
  clearAuth() {
    this.token = null;
    localStorage.removeItem('authToken');
    localStorage.removeItem('token');
    localStorage.removeItem('user');
    localStorage.removeItem('user_email');
  }

  // Get authorization headers
  private getAuthHeaders(): HeadersInit {
    const token = this.getToken();
    return {
      'Content-Type': 'application/json',
      ...(token && { Authorization: `Bearer ${token}` }),
    };
  }

  // Safely parse JSON from response; handles empty or invalid bodies
  private async safeParseJson<T = unknown>(response: Response): Promise<{ data: T | null; error: string | null }> {
    const text = await response.text();
    if (!text || text.trim() === '') {
      return { data: null, error: 'Server returned empty response. Check backend URL and CORS.' };
    }
    try {
      return { data: JSON.parse(text) as T, error: null };
    } catch {
      return { data: null, error: `Invalid response from server: ${text.slice(0, 80)}${text.length > 80 ? '…' : ''}` };
    }
  }

  // Register new user (retries on 503 for cold-start / transient backend issues)
  async register(credentials: RegisterCredentials): Promise<AuthResponse> {
    const maxAttempts = 3;
    const retryDelayMs = 1500;

    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      try {
        const response = await apiFetch('/v1/auth/signup', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            email: credentials.email,
            password: credentials.password,
            first_name: credentials.first_name,
            last_name: credentials.last_name,
          }),
        });

        const { data, error: parseError } = await this.safeParseJson<{ access_token?: string; error?: string }>(response);
        if (parseError) {
          return { success: false, error: parseError };
        }
        if (!data) {
          return { success: false, error: 'Invalid server response' };
        }
        if (!response.ok) {
          if (response.status === 503 && attempt < maxAttempts) {
            await new Promise((r) => setTimeout(r, retryDelayMs));
            continue;
          }
          return {
            success: false,
            error: data.error || 'Registration failed',
          };
        }

        const token = data.access_token;
        if (!token) {
          return { success: false, error: 'Missing access token' };
        }

        this.setToken(token);
        const user = await this.fetchMe();
        if (user) {
          localStorage.setItem('user', JSON.stringify(user));
          return { success: true, data: { token, user } };
        }

        return { success: false, error: 'Failed to load user profile' };
      } catch (error) {
        const isTransient =
          error instanceof Error &&
          (error.message.includes('fetch') ||
            error.message.includes('network') ||
            error.message.includes('Failed to fetch'));
        if (isTransient && attempt < maxAttempts) {
          await new Promise((r) => setTimeout(r, retryDelayMs));
          continue;
        }
        console.error('Registration error:', error);
        return {
          success: false,
          error: error instanceof Error ? error.message : 'Registration failed',
        };
      }
    }

    return { success: false, error: 'Registration failed. Please try again.' };
  }

  // Login user (retries on 503 for cold-start / transient backend issues)
  async login(credentials: LoginCredentials): Promise<AuthResponse> {
    const maxAttempts = 3;
    const retryDelayMs = 1500;

    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      try {
        const response = await apiFetch('/v1/auth/login', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            email: credentials.email,
            password: credentials.password,
          }),
        });

        const { data, error: parseError } = await this.safeParseJson<{ access_token?: string; error?: string }>(response);
        if (parseError) {
          return { success: false, error: parseError };
        }
        if (!data) {
          return { success: false, error: 'Invalid server response' };
        }
        if (!response.ok) {
          if (response.status === 503 && attempt < maxAttempts) {
            await new Promise((r) => setTimeout(r, retryDelayMs));
            continue;
          }
          return { success: false, error: data.error || 'Login failed' };
        }

        const token = data.access_token;
        if (!token) {
          return { success: false, error: 'Missing access token' };
        }

        this.setToken(token);
        const user = await this.fetchMe();
        if (user) {
          localStorage.setItem('user', JSON.stringify(user));
          return { success: true, data: { token, user } };
        }

        return { success: false, error: 'Failed to load user profile' };
      } catch (error) {
        const isTransient =
          error instanceof Error &&
          (error.message.includes('fetch') ||
            error.message.includes('network') ||
            error.message.includes('Failed to fetch'));
        if (isTransient && attempt < maxAttempts) {
          await new Promise((r) => setTimeout(r, retryDelayMs));
          continue;
        }
        console.error('Login error:', error);
        return {
          success: false,
          error: error instanceof Error ? error.message : 'Login failed',
        };
      }
    }

    return { success: false, error: 'Login failed. Please try again.' };
  }

  // Logout user
  async logout(): Promise<void> {
    try {
      const token = this.getToken();
      if (token) {
        await apiFetch('/v1/auth/logout', {
          method: 'DELETE',
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

      const user = await this.fetchMe();
      if (user) {
        localStorage.setItem('user', JSON.stringify(user));
        return {
          success: true,
          data: { token, user },
        };
      }

      this.clearAuth();
      return {
        success: false,
        error: 'Token verification failed',
      };
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
      const response = await apiFetch(`/api/premium-features/usage`, {
        method: 'GET',
        headers: this.getAuthHeaders(),
      });

      const data = await response.json();
      if (response.ok && data.success) {
        return {
          package_id: data.plan?.code || '',
          package_name: data.plan?.name || '',
          purchased_at: data.plan?.purchased_at || '',
          remaining_uses: data.usage || {},
          status: data.plan ? 'active' : 'none',
        };
      }
      return null;
    } catch (error) {
      console.error('Get subscription error:', error);
      return null;
    }
  }

  // Check feature access
  async checkFeatureAccess(userId: string, featureType: string): Promise<FeatureAccess> {
    try {
      const response = await apiFetch(`/v1/entitlements`, {
        method: 'GET',
        headers: this.getAuthHeaders(),
      });

      const data = await response.json();
      if (!response.ok || !Array.isArray(data)) {
        return { has_access: false, remaining_uses: 0 };
      }

      const found = data.find((item: any) => item.feature_name === featureType);
      const remaining = found?.remaining_uses ?? 0;
      return {
        has_access: remaining > 0,
        remaining_uses: remaining,
      };
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
    return true;
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
    return this.isAuthenticated();
  }

  private async fetchMe(): Promise<User | null> {
    try {
      const response = await apiFetch('/v1/auth/me', {
        method: 'GET',
        headers: this.getAuthHeaders(),
      });
      if (!response.ok) {
        return null;
      }
      const data = await response.json();
      return data;
    } catch (error) {
      console.error('Fetch user profile error:', error);
      return null;
    }
  }
}

// Create singleton instance
export const authService = new AuthService();
export default authService;
