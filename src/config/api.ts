// API Configuration for GAPLY Frontend
// Production-ready configuration for Go backend integration

export const API_CONFIG = {
  BASE_URL: process.env.REACT_APP_API_BASE_URL || 'https://your-go-backend.com',
  TIMEOUT: 10000, // 10 seconds
  ENDPOINTS: {
    AUTH: {
      LOGIN: '/api/v1/auth/login',
      REGISTER: '/api/v1/auth/register',
      REFRESH: '/api/v1/auth/refresh',
      LOGOUT: '/api/v1/auth/logout',
      VERIFY_TOKEN: '/api/v1/auth/verify'
    },
    USER: {
      PROFILE: '/api/v1/user/profile',
      ACCOUNT: '/api/v1/user/account',
      USAGE: '/api/v1/user/usage',
      UPDATE_PROFILE: '/api/v1/user/profile',
      UPDATE_PACKAGE: '/api/v1/user/package'
    },
    PAYMENT: {
      CREATE_ORDER: '/api/v1/payment/create-order',
      VERIFY: '/api/v1/payment/verify',
      WEBHOOK: '/api/v1/payment/webhook',
      HISTORY: '/api/v1/payment/history'
    },
    PACKAGES: {
      LIST: '/api/v1/packages',
      DETAILS: '/api/v1/packages/:id'
    }
  }
};

// API Response Types
export interface ApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
  message?: string;
}

export interface LoginCredentials {
  email: string;
  password: string;
}

export interface RegisterCredentials {
  email: string;
  password: string;
  name: string;
}

export interface User {
  id: string;
  email: string;
  name: string;
  package: string;
  validUntil: string;
  createdAt: string;
  updatedAt: string;
}

export interface UserAccount {
  package: string;
  gapFinderUses: number;
  deepAnalysisUses: number;
  teamSupportSessions: number;
  whatsappSupport: boolean;
  validUntil: string;
}

export interface PaymentOrder {
  id: string;
  razorpay_order_id: string;
  amount: number;
  currency: string;
  package_type: string;
  status: 'pending' | 'completed' | 'failed';
  created_at: string;
}

export interface Package {
  id: string;
  name: string;
  price: number;
  description: string;
  features: string[];
  popular: boolean;
  valid_days: number;
}
