import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { authService, User } from '../services/authService';
import { premiumService } from '../services/premiumService';

// Subscription interface
export interface Subscription {
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

// Auth context type
interface AuthContextType {
  user: User | null;
  subscription: Subscription | null;
  token: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: (credentials: { email: string; password: string }) => Promise<{ success: boolean; error?: string }>;
  register: (credentials: { email: string; password: string; first_name: string; last_name: string }) => Promise<{ success: boolean; error?: string }>;
  logout: () => Promise<void>;
  updateUser: (user: User) => void;
  refreshSubscription: () => Promise<void>;
  canUseFeature: (featureType: 'gap_finder' | 'deep_eval') => Promise<{ canUse: boolean; remainingUses: number; error?: string }>;
}

// Create context
const AuthContext = createContext<AuthContextType | undefined>(undefined);

interface AuthProviderProps {
  children: ReactNode;
}

export const AuthProvider: React.FC<AuthProviderProps> = ({ children }) => {
  const [user, setUser] = useState<User | null>(null);
  const [subscription, setSubscription] = useState<Subscription | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Initialize auth state from localStorage and verify token
  useEffect(() => {
    const initializeAuth = async () => {
      try {
        // Check if user is already logged in
        if (authService.isAuthenticated()) {
          const currentUser = authService.getCurrentUser();
          const currentToken = authService.getToken();
          
          if (currentUser && currentToken) {
            setUser(currentUser);
            setToken(currentToken);
            
            // Verify token and refresh user data
            const verificationResult = await authService.verifyToken();
            if (verificationResult.success && verificationResult.data) {
              setUser(verificationResult.data.user);
              setToken(verificationResult.data.token);
              
              // Load subscription data
              await refreshSubscription();
            } else {
              // Token is invalid, clear auth
              await logout();
            }
          }
        }
      } catch (error) {
        console.error('Auth initialization error:', error);
        await logout();
      } finally {
        setIsLoading(false);
      }
    };

    initializeAuth();
  }, []);

  const login = async (credentials: { email: string; password: string }): Promise<{ success: boolean; error?: string }> => {
    try {
      setIsLoading(true);
      const result = await authService.login(credentials);
      
      if (result.success && result.data) {
        setUser(result.data.user);
        setToken(result.data.token);
        
        // Load subscription data
        await refreshSubscription();
        
        return { success: true };
      } else {
        return { success: false, error: result.error || 'Login failed' };
      }
    } catch (error) {
      console.error('Login error:', error);
      return { 
        success: false, 
        error: error instanceof Error ? error.message : 'Login failed' 
      };
    } finally {
      setIsLoading(false);
    }
  };

  const register = async (credentials: { email: string; password: string; first_name: string; last_name: string }): Promise<{ success: boolean; error?: string }> => {
    try {
      setIsLoading(true);
      const result = await authService.register(credentials);
      
      if (result.success && result.data) {
        setUser(result.data.user);
        setToken(result.data.token);
        
        return { success: true };
      } else {
        return { success: false, error: result.error || 'Registration failed' };
      }
    } catch (error) {
      console.error('Registration error:', error);
      return { 
        success: false, 
        error: error instanceof Error ? error.message : 'Registration failed' 
      };
    } finally {
      setIsLoading(false);
    }
  };

  const logout = async () => {
    try {
      await authService.logout();
    } catch (error) {
      console.error('Logout error:', error);
    } finally {
      setUser(null);
      setToken(null);
      setSubscription(null);
    }
  };

  const updateUser = (updatedUser: User) => {
    setUser(updatedUser);
    localStorage.setItem('user', JSON.stringify(updatedUser));
  };

  const refreshSubscription = async () => {
    try {
      if (user) {
        const subscriptionData = await premiumService.getUserSubscription();
        setSubscription(subscriptionData);
      }
    } catch (error) {
      console.error('Refresh subscription error:', error);
    }
  };

  const canUseFeature = async (featureType: 'gap_finder' | 'deep_eval'): Promise<{ canUse: boolean; remainingUses: number; error?: string }> => {
    try {
      return await premiumService.canUseFeature(featureType);
    } catch (error) {
      console.error('Check feature access error:', error);
      return {
        canUse: false,
        remainingUses: 0,
        error: error instanceof Error ? error.message : 'Failed to check feature access',
      };
    }
  };

  const value: AuthContextType = {
    user,
    subscription,
    token,
    isAuthenticated: !!user && !!token,
    isLoading,
    login,
    register,
    logout,
    updateUser,
    refreshSubscription,
    canUseFeature,
  };

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  );
};

export const useAuth = (): AuthContextType => {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
};