import { buildApiUrl } from './config';

// ============================================
// FREE FEATURES API
// ============================================

/**
 * Search research papers (FREE FEATURE)
 * @param query Search query
 * @param filters Optional filters (year, domain, etc.)
 */
export async function searchPapers(query: string, filters?: any) {
  const response = await fetch(buildApiUrl('/api/search'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      query,
      ...filters,
    }),
  });

  if (!response.ok) {
    throw new Error('Failed to search papers');
  }

  return await response.json();
}

/**
 * Match journals for a paper (FREE FEATURE)
 * @param paperTitle Paper title
 * @param paperAbstract Paper abstract
 * @param keywords Optional keywords
 */
export async function matchJournals(
  paperTitle: string,
  paperAbstract: string,
  keywords?: string[]
) {
  const response = await fetch(buildApiUrl('/api/match'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      title: paperTitle,
      abstract: paperAbstract,
      keywords,
    }),
  });

  if (!response.ok) {
    throw new Error('Failed to match journals');
  }

  return await response.json();
}

/**
 * Paraphrase text / AI Remover (FREE FEATURE)
 * @param text Text to paraphrase
 */
export async function paraphraseText(text: string) {
  const response = await fetch(buildApiUrl('/api/v1/paraphrase/direct'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      text,
    }),
  });

  if (!response.ok) {
    throw new Error('Failed to paraphrase text');
  }

  return await response.json();
}

// ============================================
// AUTHENTICATION API
// ============================================

/**
 * User signup
 */
export async function signup(email: string, password: string, firstName: string, lastName: string) {
  const response = await fetch(buildApiUrl('/api/premium/signup'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      email,
      password,
      first_name: firstName,
      last_name: lastName,
    }),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Signup failed');
  }

  return await response.json();
}

/**
 * User login
 */
export async function login(email: string, password: string) {
  const response = await fetch(buildApiUrl('/api/premium/login'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      email,
      password,
    }),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Login failed');
  }

  return await response.json();
}

// ============================================
// PAYMENT API
// ============================================

/**
 * Create Razorpay order
 */
export async function createOrder(packageId: string, token: string) {
  const response = await fetch(buildApiUrl('/api/premium/create-order'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
    body: JSON.stringify({
      package_id: packageId,
    }),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Failed to create order');
  }

  return await response.json();
}

/**
 * Verify Razorpay payment
 */
export async function verifyPayment(
  orderId: string,
  paymentId: string,
  signature: string,
  token: string
) {
  const response = await fetch(buildApiUrl('/api/premium/verify-payment'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
    body: JSON.stringify({
      razorpay_order_id: orderId,
      razorpay_payment_id: paymentId,
      razorpay_signature: signature,
    }),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Payment verification failed');
  }

  return await response.json();
}

/**
 * Get user plan details
 */
export async function getUserPlan(token: string) {
  const response = await fetch(buildApiUrl('/api/premium/user-plan'), {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    throw new Error('Failed to get user plan');
  }

  return await response.json();
}

// ============================================
// PREMIUM FEATURES API
// ============================================

/**
 * Start gap analysis (PREMIUM FEATURE)
 * @param papers Array of paper files or URLs
 * @param token User authentication token
 */
export async function startGapAnalysis(papers: File[], token: string) {
  const formData = new FormData();
  papers.forEach((paper, index) => {
    formData.append(`paper_${index}`, paper);
  });

  const response = await fetch(buildApiUrl('/api/premium/gap-analysis'), {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${token}`,
    },
    body: formData,
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Failed to start gap analysis');
  }

  return await response.json();
}

/**
 * Get gap analysis status
 */
export async function getGapAnalysisStatus(jobId: string, token: string) {
  const response = await fetch(buildApiUrl(`/api/premium/gap-analysis/status/${jobId}`), {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    throw new Error('Failed to get analysis status');
  }

  return await response.json();
}

/**
 * Get gap analysis results
 */
export async function getGapAnalysisResults(jobId: string, token: string) {
  const response = await fetch(buildApiUrl(`/api/premium/gap-analysis/results/${jobId}`), {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    throw new Error('Failed to get analysis results');
  }

  return await response.json();
}

/**
 * Start deep paper evaluation (PREMIUM FEATURE)
 */
export async function startDeepEvaluation(paper: File, token: string) {
  const formData = new FormData();
  formData.append('paper', paper);

  const response = await fetch(buildApiUrl('/api/v3/premium/deep-evaluation/submit'), {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${token}`,
    },
    body: formData,
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Failed to start deep evaluation');
  }

  return await response.json();
}

/**
 * Get deep evaluation status
 */
export async function getDeepEvaluationStatus(jobId: string, token: string) {
  const response = await fetch(buildApiUrl(`/api/v3/premium/deep-evaluation/status/${jobId}`), {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    throw new Error('Failed to get evaluation status');
  }

  return await response.json();
}

/**
 * Download deep evaluation report
 */
export async function downloadDeepEvaluationReport(jobId: string, token: string) {
  const response = await fetch(buildApiUrl(`/api/v3/premium/deep-evaluation/report/${jobId}`), {
    method: 'GET',
    headers: {
      'Authorization': `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    throw new Error('Failed to download report');
  }

  return await response.blob();
}

// ============================================
// JOURNAL QUARTILE ANALYSIS
// ============================================

/**
 * Get journal quartile data (Q1, Q2, Q3, Q4)
 */
export async function getQuartileData(quartile: 'q1' | 'q2' | 'q3' | 'q4') {
  const response = await fetch(buildApiUrl(`/api/journals/quartile/${quartile}`), {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to get ${quartile.toUpperCase()} data`);
  }

  return await response.json();
}

/**
 * Download quartile report
 */
export async function downloadQuartileReport(quartile: 'q1' | 'q2' | 'q3' | 'q4') {
  const response = await fetch(buildApiUrl(`/api/journals/quartile/${quartile}/download`), {
    method: 'GET',
  });

  if (!response.ok) {
    throw new Error(`Failed to download ${quartile.toUpperCase()} report`);
  }

  return await response.blob();
}

// ============================================
// HEALTH CHECK
// ============================================

/**
 * Check backend health
 */
export async function checkHealth() {
  const response = await fetch(buildApiUrl('/api/health'), {
    method: 'GET',
  });

  if (!response.ok) {
    throw new Error('Backend health check failed');
  }

  return await response.json();
}

