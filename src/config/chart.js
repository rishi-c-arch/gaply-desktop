// config/chart.js
// Chart.js Configuration and Fallback Handling

// Chart.js CDN Configuration
export const CHART_CONFIG = {
  CDN_URL: 'https://cdn.jsdelivr.net/npm/chart.js@3.9.1/dist/chart.min.js',
  FALLBACK_URL: '/static/vendor/chart.min.js',
  VERSION: '3.9.1'
};

// Load Chart.js with fallback
export const loadChartJS = () => {
  return new Promise((resolve, reject) => {
    // Check if Chart.js is already loaded
    if (window.Chart) {
      resolve(window.Chart);
      return;
    }
    
    // Try CDN first
    const script = document.createElement('script');
    script.src = CHART_CONFIG.CDN_URL;
    script.async = true;
    
    script.onload = () => {
      if (window.Chart) {
        console.log('Chart.js loaded from CDN');
        resolve(window.Chart);
      } else {
        console.warn('Chart.js CDN failed, trying fallback');
        loadFallbackChartJS().then(resolve).catch(reject);
      }
    };
    
    script.onerror = () => {
      console.warn('Chart.js CDN failed, trying fallback');
      loadFallbackChartJS().then(resolve).catch(reject);
    };
    
    document.head.appendChild(script);
  });
};

// Load Chart.js from local fallback
const loadFallbackChartJS = () => {
  return new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = CHART_CONFIG.FALLBACK_URL;
    script.async = true;
    
    script.onload = () => {
      if (window.Chart) {
        console.log('Chart.js loaded from fallback');
        resolve(window.Chart);
      } else {
        reject(new Error('Chart.js failed to load from both CDN and fallback'));
      }
    };
    
    script.onerror = () => {
      reject(new Error('Chart.js failed to load from both CDN and fallback'));
    };
    
    document.head.appendChild(script);
  });
};

// Chart Configuration Templates
export const CHART_TEMPLATES = {
  // Research Analysis Chart
  RESEARCH_ANALYSIS: {
    type: 'doughnut',
    data: {
      labels: ['Completed', 'In Progress', 'Pending'],
      datasets: [{
        data: [0, 0, 0],
        backgroundColor: [
          '#10b981',
          '#f59e0b',
          '#6b7280'
        ],
        borderWidth: 0
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          position: 'bottom',
          labels: {
            usePointStyle: true,
            padding: 20,
            font: {
              family: 'Inter',
              size: 14
            }
          }
        }
      }
    }
  },
  
  // Trend Analysis Chart
  TREND_ANALYSIS: {
    type: 'line',
    data: {
      labels: [],
      datasets: [{
        label: 'Research Trends',
        data: [],
        borderColor: '#3b82f6',
        backgroundColor: 'rgba(59, 130, 246, 0.1)',
        borderWidth: 2,
        fill: true,
        tension: 0.4
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          display: false
        }
      },
      scales: {
        y: {
          beginAtZero: true,
          grid: {
            color: 'rgba(0, 0, 0, 0.1)'
          }
        },
        x: {
          grid: {
            color: 'rgba(0, 0, 0, 0.1)'
          }
        }
      }
    }
  }
};

// Create Chart with Error Handling
export const createChart = async (ctx, config, template = 'RESEARCH_ANALYSIS') => {
  try {
    const Chart = await loadChartJS();
    const chartConfig = { ...CHART_TEMPLATES[template], ...config };
    return new Chart(ctx, chartConfig);
  } catch (error) {
    console.error('Failed to create chart:', error);
    showChartError(ctx, 'Chart failed to load');
    return null;
  }
};

// Show Chart Error Fallback
const showChartError = (ctx, message) => {
  const canvas = ctx.canvas;
  const parent = canvas.parentElement;
  
  parent.innerHTML = `
    <div class="chart-error">
      <div class="error-icon">📊</div>
      <p>${message}</p>
      <button onclick="location.reload()" class="retry-button">Retry</button>
    </div>
  `;
  
  // Add error styles
  const style = document.createElement('style');
  style.textContent = `
    .chart-error {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      height: 300px;
      background: rgba(255, 255, 255, 0.05);
      border-radius: 12px;
      color: var(--text-muted);
      text-align: center;
    }
    
    .error-icon {
      font-size: 3rem;
      margin-bottom: 1rem;
      opacity: 0.5;
    }
    
    .retry-button {
      margin-top: 1rem;
      padding: 0.5rem 1rem;
      background: #f59e0b;
      color: white;
      border: none;
      border-radius: 6px;
      cursor: pointer;
      font-weight: 600;
    }
    
    .retry-button:hover {
      background: #d97706;
    }
  `;
  
  if (!document.querySelector('#chart-error-styles')) {
    style.id = 'chart-error-styles';
    document.head.appendChild(style);
  }
};

export default CHART_CONFIG;
