import { API_BASE_URL, apiFetch } from '../api/config';
import { authService } from './authService';

export interface PlanConfig {
  id: string;
  name: string;
  description: string;
  price: number;
  currency: string;
  features: {
    gap_finder: {
      name: string;
      description: string;
      uses: number;
      included: boolean;
    };
    deep_eval: {
      name: string;
      description: string;
      uses: number;
      included: boolean;
    };
    support: {
      included: boolean;
    };
  };
}

export interface RazorpayOrder {
  id: string;
  amount: number;
  currency: string;
  receipt: string;
  status: string;
}

export interface PaymentResponse {
  success: boolean;
  data?: {
    order: RazorpayOrder;
    key_id?: string;
  };
  error?: string;
}

export interface PremiumFeatureResponse {
  success: boolean;
  data?: {
    job_id: string;
    status: string;
    message: string;
  };
  error?: string;
}

class PremiumService {
  // Get all available plans
  async getPlans(): Promise<PlanConfig[]> {
    try {
      const response = await apiFetch('/api/premium/packages', {
        method: 'GET',
        headers: { 'Content-Type': 'application/json' },
      });

      const data = await response.json();

      if (response.ok && data.success) {
        if (Array.isArray(data.packages)) {
          return data.packages;
        }
        if (Array.isArray(data.data)) {
          return data.data;
        }
      } else {
        // Fallback to hardcoded plans if API fails
        return this.getDefaultPlans();
      }
    } catch (error) {
      console.error('Get plans error:', error);
      return this.getDefaultPlans();
    }
  }

  // Default plans as fallback
  private getDefaultPlans(): PlanConfig[] {
    return [
      {
        id: "PLAN-471",
        name: "Gaply Basic",
        description: "Perfect for individual researchers",
        price: 471,
        currency: "INR",
        features: {
          gap_finder: {
            name: "Advanced Literature Analysis",
            description: "Upload up to 5 research papers and get comprehensive literature gap analysis",
            uses: 1,
            included: true,
          },
          deep_eval: {
            name: "Deep Paper Analysis",
            description: "70-parameter evaluation engine with professional reports",
            uses: 0,
            included: false,
          },
          support: {
            included: false,
          },
        },
      },
      {
        id: "PLAN-1061",
        name: "Gaply Plus",
        description: "Great for research teams",
        price: 1061,
        currency: "INR",
        features: {
          gap_finder: {
            name: "Advanced Literature Analysis",
            description: "Upload up to 5 research papers and get comprehensive literature gap analysis",
            uses: 2,
            included: true,
          },
          deep_eval: {
            name: "Deep Paper Analysis",
            description: "70-parameter evaluation engine with professional reports",
            uses: 1,
            included: true,
          },
          support: {
            included: false,
          },
        },
      },
      {
        id: "PLAN-2241",
        name: "Gaply Pro",
        description: "Perfect for institutions and labs",
        price: 2241,
        currency: "INR",
        features: {
          gap_finder: {
            name: "Advanced Literature Analysis",
            description: "Upload up to 5 research papers and get comprehensive literature gap analysis",
            uses: 5,
            included: true,
          },
          deep_eval: {
            name: "Deep Paper Analysis",
            description: "70-parameter evaluation engine with professional reports",
            uses: 5,
            included: true,
          },
          support: {
            included: true,
          },
        },
      },
    ];
  }

  // Create Razorpay order for payment
  async createPaymentOrder(planId: string): Promise<PaymentResponse> {
    try {
      const user = authService.getCurrentUser();
      if (!user) {
        return {
          success: false,
          error: 'User not authenticated',
        };
      }

      const response = await apiFetch('/v1/payments/razorpay/create-order', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': crypto.randomUUID(),
          ...(authService.getToken() && { Authorization: `Bearer ${authService.getToken()}` }),
        },
        body: JSON.stringify({
          plan_code: planId,
        }),
      });

      const data = await response.json();

      if (response.ok && data.order_id) {
        return {
          success: true,
          data: {
            order: {
              id: data.order_id,
              amount: data.amount,
              currency: data.currency,
              receipt: data.order_id,
              status: data.status,
            },
            key_id: data.key_id,
          },
        };
      } else {
        return {
          success: false,
          error: data.error || 'Failed to create payment order',
        };
      }
    } catch (error) {
      console.error('Create payment order error:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Failed to create payment order',
      };
    }
  }

  // Verify payment and create subscription
  async verifyPayment(orderId: string, paymentId: string, signature: string): Promise<PaymentResponse> {
    try {
      const user = authService.getCurrentUser();
      if (!user) {
        return {
          success: false,
          error: 'User not authenticated',
        };
      }

      const response = await apiFetch('/v1/payments/razorpay/verify', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(authService.getToken() && { Authorization: `Bearer ${authService.getToken()}` }),
        },
        body: JSON.stringify({
          razorpay_order_id: orderId,
          razorpay_payment_id: paymentId,
          razorpay_signature: signature,
        }),
      });

      const data = await response.json();

      if (response.ok && data.success) {
        // Refresh user data to get updated premium status
        await authService.verifyToken();
        
        return {
          success: true,
          data: data.data,
        };
      } else {
        return {
          success: false,
          error: data.message || 'Payment verification failed',
        };
      }
    } catch (error) {
      console.error('Verify payment error:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Payment verification failed',
      };
    }
  }

  // Use Advanced Literature Analysis feature
  async useLiteratureAnalysis(papers: File[]): Promise<PremiumFeatureResponse> {
    try {
      const user = authService.getCurrentUser();
      if (!user) {
        return {
          success: false,
          error: 'User not authenticated',
        };
      }

      // Check feature access first
      const access = await authService.checkFeatureAccess(user.id, 'gap_finder');
      if (!access.has_access) {
        return {
          success: false,
          error: 'You do not have access to this feature. Please purchase a plan.',
        };
      }

      // Create FormData for file upload
      const formData = new FormData();
      papers.forEach((paper, index) => {
        formData.append(`paper_${index}`, paper);
      });

      const response = await apiFetch('/api/premium-features/literature-analysis', {
        method: 'POST',
        headers: {
          ...(authService.getToken() && { Authorization: `Bearer ${authService.getToken()}` }),
        },
        body: formData,
      });

      const data = await response.json();

      if (response.ok && data.success) {
        await authService.useFeature(user.id, 'gap_finder', data.job_id);
        return {
          success: true,
          data: {
            job_id: data.job_id,
            status: 'queued',
            message: data.message || 'Request accepted',
          },
        };
      } else {
        return {
          success: false,
          error: data.message || 'Literature analysis failed',
        };
      }
    } catch (error) {
      console.error('Literature analysis error:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Literature analysis failed',
      };
    }
  }

  // Use Deep Paper Analysis feature
  async useDeepPaperAnalysis(pdfFile: File): Promise<PremiumFeatureResponse> {
    try {
      const user = authService.getCurrentUser();
      if (!user) {
        return {
          success: false,
          error: 'User not authenticated',
        };
      }

      // Check feature access first
      const access = await authService.checkFeatureAccess(user.id, 'deep_eval');
      if (!access.has_access) {
        return {
          success: false,
          error: 'You do not have access to this feature. Please purchase a plan.',
        };
      }

      // Create FormData for file upload
      const formData = new FormData();
      formData.append('pdf_file', pdfFile);

      const response = await apiFetch('/api/premium-features/deep-paper-analysis', {
        method: 'POST',
        headers: {
          ...(authService.getToken() && { Authorization: `Bearer ${authService.getToken()}` }),
        },
        body: formData,
      });

      const data = await response.json();

      if (response.ok && data.success) {
        await authService.useFeature(user.id, 'deep_eval', data.job_id);
        return {
          success: true,
          data: {
            job_id: data.job_id,
            status: 'queued',
            message: data.message || 'Request accepted',
          },
        };
      } else {
        return {
          success: false,
          error: data.message || 'Deep paper analysis failed',
        };
      }
    } catch (error) {
      console.error('Deep paper analysis error:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Deep paper analysis failed',
      };
    }
  }

  // Check job status
  async checkJobStatus(jobId: string): Promise<{
    success: boolean;
    data?: {
      status: string;
      result?: any;
      error?: string;
      download_url?: string;
    };
    error?: string;
  }> {
    try {
      const response = await apiFetch(`/api/premium-features/status/${jobId}`, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
          ...(authService.getToken() && { Authorization: `Bearer ${authService.getToken()}` }),
        },
      });

      const data = await response.json();

      if (response.ok && data.success) {
        return {
          success: true,
          data: {
            status: data.message,
            result: data.data,
            download_url: data.download_url,
          },
        };
      } else {
        return {
          success: false,
          error: data.message || 'Failed to check job status',
        };
      }
    } catch (error) {
      console.error('Check job status error:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Failed to check job status',
      };
    }
  }

  // Get user's subscription details
  async getUserSubscription(): Promise<any> {
    try {
      const user = authService.getCurrentUser();
      if (!user) {
        return null;
      }

      return await authService.getUserSubscription(user.id);
    } catch (error) {
      console.error('Get user subscription error:', error);
      return null;
    }
  }

  // Check if user can use a specific feature
  async canUseFeature(featureType: 'gap_finder' | 'deep_eval'): Promise<{
    canUse: boolean;
    remainingUses: number;
    error?: string;
  }> {
    try {
      const user = authService.getCurrentUser();
      if (!user) {
        return {
          canUse: false,
          remainingUses: 0,
          error: 'User not authenticated',
        };
      }

      const access = await authService.checkFeatureAccess(user.id, featureType);
      return {
        canUse: access.has_access,
        remainingUses: access.remaining_uses,
        error: access.has_access ? undefined : 'No remaining uses or subscription expired',
      };
    } catch (error) {
      console.error('Check feature access error:', error);
      return {
        canUse: false,
        remainingUses: 0,
        error: error instanceof Error ? error.message : 'Failed to check feature access',
      };
    }
  }
}

// Create singleton instance
export const premiumService = new PremiumService();
export default premiumService;
