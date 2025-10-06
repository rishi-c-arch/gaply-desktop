// config/api.js
// Centralized API Configuration
export const CONFIG = {
  API_BASE_URL: (typeof process !== 'undefined' && process.env && (process.env.REACT_APP_API_BASE_URL || process.env.VITE_API_BASE_URL)) || 'https://backend.gaply.in',
  VERSION: 'v1',
  TIMEOUT: 10000,
  RETRY_ATTEMPTS: 3
};

// API Endpoints
export const API_ENDPOINTS = {
  // Authentication
  LOGIN: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/auth/login`,
  REGISTER: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/auth/register`,
  LOGOUT: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/auth/logout`,
  
  // Research Analysis
  ANALYZE: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/analyze`,
  UPLOAD: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/upload`,
  RESULTS: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/results`,
  
  // User Management
  PROFILE: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/user/profile`,
  HISTORY: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/user/history`,
  
  // Free Features
  FREE_ANALYSIS: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/free/analyze`,
  FREE_RESULTS: `${CONFIG.API_BASE_URL}/api/${CONFIG.VERSION}/free/results`
};

// Error Handling Utility
export const handleApiError = (error, context) => {
  console.error(`${context}:`, error);
  
  let userMessage = 'An error occurred. Please try again.';
  
  if (error.response) {
    // Server responded with error status
    const status = error.response.status;
    switch (status) {
      case 400:
        userMessage = 'Invalid request. Please check your input.';
        break;
      case 401:
        userMessage = 'Please log in to continue.';
        break;
      case 403:
        userMessage = 'You do not have permission to perform this action.';
        break;
      case 404:
        userMessage = 'The requested resource was not found.';
        break;
      case 500:
        userMessage = 'Server error. Please try again later.';
        break;
      default:
        userMessage = `Error ${status}: ${error.response.data?.message || 'Unknown error'}`;
    }
  } else if (error.request) {
    // Network error
    userMessage = 'Network error. Please check your connection.';
  }
  
  // Show user notification
  showUserNotification(userMessage, 'error');
  
  return userMessage;
};

// User Notification System
export const showUserNotification = (message, type = 'info') => {
  // Remove existing notifications
  const existing = document.querySelector('.api-notification');
  if (existing) existing.remove();
  
  // Create notification element
  const notification = document.createElement('div');
  notification.className = `api-notification api-notification--${type}`;
  notification.innerHTML = `
    <div class="notification-content">
      <span class="notification-icon">${getNotificationIcon(type)}</span>
      <span class="notification-message">${message}</span>
      <button class="notification-close" onclick="this.parentElement.parentElement.remove()">×</button>
    </div>
  `;
  
  // Add styles
  notification.style.cssText = `
    position: fixed;
    top: 20px;
    right: 20px;
    z-index: 10000;
    background: ${getNotificationColor(type)};
    color: white;
    padding: 1rem 1.5rem;
    border-radius: 8px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    max-width: 400px;
    animation: slideIn 0.3s ease;
  `;
  
  // Add to DOM
  document.body.appendChild(notification);
  
  // Auto remove after 5 seconds
  setTimeout(() => {
    if (notification.parentElement) {
      notification.style.animation = 'slideOut 0.3s ease';
      setTimeout(() => notification.remove(), 300);
    }
  }, 5000);
};

const getNotificationIcon = (type) => {
  switch (type) {
    case 'success': return '✅';
    case 'error': return '❌';
    case 'warning': return '⚠️';
    default: return 'ℹ️';
  }
};

const getNotificationColor = (type) => {
  switch (type) {
    case 'success': return '#10b981';
    case 'error': return '#ef4444';
    case 'warning': return '#f59e0b';
    default: return '#3b82f6';
  }
};

// Loading State Utility
export const showLoading = (element, message = 'Loading...') => {
  if (!element) return;
  
  element.innerHTML = `
    <div class="loading-spinner">
      <div class="spinner"></div>
      <p>${message}</p>
    </div>
  `;
  
  // Add spinner styles if not already added
  if (!document.querySelector('#spinner-styles')) {
    const style = document.createElement('style');
    style.id = 'spinner-styles';
    style.textContent = `
      .loading-spinner {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: 2rem;
        color: var(--text-muted);
      }
      
      .spinner {
        width: 40px;
        height: 40px;
        border: 3px solid rgba(245, 158, 11, 0.3);
        border-top: 3px solid #f59e0b;
        border-radius: 50%;
        animation: spin 1s linear infinite;
        margin-bottom: 1rem;
      }
      
      @keyframes spin {
        0% { transform: rotate(0deg); }
        100% { transform: rotate(360deg); }
      }
      
      @keyframes slideIn {
        from { transform: translateX(100%); opacity: 0; }
        to { transform: translateX(0); opacity: 1; }
      }
      
      @keyframes slideOut {
        from { transform: translateX(0); opacity: 1; }
        to { transform: translateX(100%); opacity: 0; }
      }
    `;
    document.head.appendChild(style);
  }
};

// API Request Wrapper with Error Handling
export const apiRequest = async (url, options = {}) => {
  const defaultOptions = {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
    timeout: CONFIG.TIMEOUT,
    ...options
  };
  
  try {
    const response = await fetch(url, defaultOptions);
    
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
    
    const data = await response.json();
    return { success: true, data };
    
  } catch (error) {
    return { success: false, error };
  }
};

export default CONFIG;
