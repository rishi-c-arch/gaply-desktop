// API Service for GAPLY Frontend
// Production-ready API service with error handling and authentication

import { API_CONFIG, ApiResponse, LoginCredentials, RegisterCredentials, User, UserAccount, PaymentOrder, Package } from '../config/api';

class ApiService {
  private baseURL: string;
  private timeout: number;

  constructor() {
    this.baseURL = API_CONFIG.BASE_URL;
    this.timeout = API_CONFIG.TIMEOUT;
  }

  // Get auth token from localStorage
  private getAuthToken(): string | null {
    return localStorage.getItem('token');
  }

  // Get default headers
  private getHeaders(includeAuth: boolean = true): HeadersInit {
    const headers: HeadersInit = {
      'Content-Type': 'application/json',
    };

    if (includeAuth) {
      const token = this.getAuthToken();
      if (token) {
        headers['Authorization'] = `Bearer ${token}`;
      }
    }

    return headers;
  }

  // Make HTTP request with error handling
  private async makeRequest<T>(
    endpoint: string,
    options: RequestInit = {},
    includeAuth: boolean = true
  ): Promise<ApiResponse<T>> {
    try {
      const url = `${this.baseURL}${endpoint}`;
      const config: RequestInit = {
        ...options,
        headers: {
          ...this.getHeaders(includeAuth),
          ...options.headers,
        },
        signal: AbortSignal.timeout(this.timeout),
      };

      const response = await fetch(url, config);
      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.error || `HTTP ${response.status}: ${response.statusText}`);
      }

      return {
        success: true,
        data,
        message: data.message,
      };
    } catch (error) {
      console.error('API Request failed:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'An unexpected error occurred',
      };
    }
  }

  // Authentication methods
  async login(credentials: LoginCredentials): Promise<ApiResponse<{ token: string; user: User }>> {
    return this.makeRequest(
      API_CONFIG.ENDPOINTS.AUTH.LOGIN,
      {
        method: 'POST',
        body: JSON.stringify(credentials),
      },
      false
    );
  }

  async register(credentials: RegisterCredentials): Promise<ApiResponse<{ token: string; user: User }>> {
    return this.makeRequest(
      API_CONFIG.ENDPOINTS.AUTH.REGISTER,
      {
        method: 'POST',
        body: JSON.stringify(credentials),
      },
      false
    );
  }

  async logout(): Promise<ApiResponse> {
    const result = await this.makeRequest(API_CONFIG.ENDPOINTS.AUTH.LOGOUT, {
      method: 'POST',
    });

    // Clear local storage regardless of API response
    localStorage.removeItem('token');
    localStorage.removeItem('user');

    return result;
  }

  async verifyToken(): Promise<ApiResponse<User>> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.AUTH.VERIFY_TOKEN);
  }

  // User methods
  async getUserProfile(): Promise<ApiResponse<User>> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.USER.PROFILE);
  }

  async getUserAccount(): Promise<ApiResponse<UserAccount>> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.USER.ACCOUNT);
  }

  async updateUserProfile(profileData: Partial<User>): Promise<ApiResponse<User>> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.USER.UPDATE_PROFILE, {
      method: 'PUT',
      body: JSON.stringify(profileData),
    });
  }

  // Package methods
  async getPackages(): Promise<ApiResponse<Package[]>> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.PACKAGES.LIST);
  }

  async getPackageDetails(packageId: string): Promise<ApiResponse<Package>> {
    return this.makeRequest(
      API_CONFIG.ENDPOINTS.PACKAGES.DETAILS.replace(':id', packageId)
    );
  }

  // Payment methods
  async createPaymentOrder(packageId: string): Promise<ApiResponse<PaymentOrder>> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.PAYMENT.CREATE_ORDER, {
      method: 'POST',
      body: JSON.stringify({ package_id: packageId }),
    });
  }

  async verifyPayment(paymentData: {
    razorpay_order_id: string;
    razorpay_payment_id: string;
    razorpay_signature: string;
  }): Promise<ApiResponse<{ success: boolean; package?: string }>> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.PAYMENT.VERIFY, {
      method: 'POST',
      body: JSON.stringify(paymentData),
    });
  }

  async getPaymentHistory(): Promise<ApiResponse<PaymentOrder[]>> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.PAYMENT.HISTORY);
  }

  // Usage tracking
  async updateUsage(feature: string, usesRemaining: number): Promise<ApiResponse> {
    return this.makeRequest(API_CONFIG.ENDPOINTS.USER.USAGE, {
      method: 'PUT',
      body: JSON.stringify({
        feature,
        uses_remaining: usesRemaining,
      }),
    });
  }
}

// Export singleton instance
export const apiService = new ApiService();
export default apiService;
