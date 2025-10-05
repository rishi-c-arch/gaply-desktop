// HeroR3F Helper Functions

/**
 * Check if WebGL is supported in the current browser
 * @returns {boolean} True if WebGL is supported
 */
export function isWebGLSupported() {
  try {
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
    return !!gl;
  } catch (e) {
    return false;
  }
}

/**
 * Get optimal device pixel ratio for performance
 * @param {number} maxRatio - Maximum allowed DPR (default: 1.5)
 * @returns {number} Clamped device pixel ratio
 */
export function getOptimalDPR(maxRatio = 1.5) {
  return Math.min(window.devicePixelRatio || 1, maxRatio);
}

/**
 * Generate random word instances for falling animation
 * @param {Array<string>} words - Array of words to use
 * @param {number} count - Number of instances to generate
 * @returns {Array<Object>} Array of word instance objects
 */
export function generateWordInstances(words, count) {
  return Array.from({ length: count }, (_, i) => ({
    id: i,
    word: words[i % words.length],
    position: [
      Math.random() * 8 - 4, // X: -4 to 4
      Math.random() * 4 + 2, // Y: 2 to 6 (spawn above)
      Math.random() * 0.9 - 0.3 // Z: -0.3 to 0.6 (depth)
    ],
    rotation: [
      Math.random() * 0.2 - 0.1,
      Math.random() * 0.2 - 0.1,
      Math.random() * 0.2 - 0.1
    ],
    scale: [
      0.8 + Math.random() * 0.4,
      0.8 + Math.random() * 0.4,
      0.8 + Math.random() * 0.4
    ],
    speed: 0.15 + Math.random() * 0.45,
    opacity: 0.6 + Math.random() * 0.4,
    rotationSpeed: [
      Math.random() * 0.02 - 0.01,
      Math.random() * 0.02 - 0.01,
      Math.random() * 0.02 - 0.01
    ],
    fadeIn: 0.2,
    fadeOut: 1.8,
    age: Math.random() * 10
  }));
}

/**
 * Update word instance properties for animation frame
 * @param {Array<Object>} instances - Array of word instances
 * @param {number} delta - Time delta since last frame
 * @returns {Array<Object>} Updated instances array
 */
export function updateWordInstances(instances, delta) {
  return instances.map(instance => {
    const updated = { ...instance };
    
    // Update age
    updated.age += delta;
    
    // Update position (falling)
    updated.position[1] -= updated.speed * delta;
    
    // Update rotation
    updated.rotation[0] += updated.rotationSpeed[0];
    updated.rotation[1] += updated.rotationSpeed[1];
    updated.rotation[2] += updated.rotationSpeed[2];
    
    // Fade in/out logic
    if (updated.age < updated.fadeIn) {
      updated.opacity = updated.age / updated.fadeIn * (0.6 + Math.random() * 0.4);
    } else if (updated.position[1] < -updated.fadeOut) {
      updated.opacity = Math.max(0, (updated.position[1] + updated.fadeOut + 2) / 2);
    }
    
    // Respawn when fallen too far
    if (updated.position[1] < -2.5) {
      updated.position = [
        Math.random() * 8 - 4,
        3,
        Math.random() * 0.9 - 0.3
      ];
      updated.age = 0;
      updated.opacity = 0.6 + Math.random() * 0.4;
      updated.speed = 0.15 + Math.random() * 0.45;
      updated.rotationSpeed = [
        Math.random() * 0.02 - 0.01,
        Math.random() * 0.02 - 0.01,
        Math.random() * 0.02 - 0.01
      ];
    }
    
    return updated;
  });
}

/**
 * Create a simple 3D logo geometry for fallback
 * @param {Object} props - Logo configuration
 * @returns {Object} Three.js geometry and material
 */
export function createFallbackLogo({ color = '#4F46E5', size = 0.6 } = {}) {
  return {
    geometry: {
      main: { type: 'boxGeometry', args: [1.5 * size, 0.3 * size, 0.3 * size] },
      accent: { type: 'boxGeometry', args: [1.8 * size, 0.1 * size, 0.1 * size] },
      letter: { type: 'cylinderGeometry', args: [0.2 * size, 0.2 * size, 0.1 * size, 8] }
    },
    materials: {
      main: { color },
      accent: { color: '#6366F1' },
      letter: { color: '#FFFFFF' }
    }
  };
}

/**
 * Debounce function for performance optimization
 * @param {Function} func - Function to debounce
 * @param {number} wait - Wait time in milliseconds
 * @returns {Function} Debounced function
 */
export function debounce(func, wait) {
  let timeout;
  return function executedFunction(...args) {
    const later = () => {
      clearTimeout(timeout);
      func(...args);
    };
    clearTimeout(timeout);
    timeout = setTimeout(later, wait);
  };
}

/**
 * Throttle function for performance optimization
 * @param {Function} func - Function to throttle
 * @param {number} limit - Time limit in milliseconds
 * @returns {Function} Throttled function
 */
export function throttle(func, limit) {
  let inThrottle;
  return function(...args) {
    if (!inThrottle) {
      func.apply(this, args);
      inThrottle = true;
      setTimeout(() => inThrottle = false, limit);
    }
  };
}

/**
 * Preload GLB model for better performance
 * @param {string} url - URL of the GLB model
 * @returns {Promise<Object>} Promise that resolves with loaded model
 */
export function preloadGLB(url) {
  return new Promise((resolve, reject) => {
    const loader = new window.THREE.GLTFLoader();
    loader.load(
      url,
      (gltf) => resolve(gltf),
      (progress) => console.log('Loading progress:', progress),
      (error) => reject(error)
    );
  });
}

/**
 * Get responsive canvas dimensions
 * @param {string} breakpoint - Current breakpoint ('mobile', 'tablet', 'desktop')
 * @returns {Object} Width and height dimensions
 */
export function getCanvasDimensions(breakpoint = 'desktop') {
  const dimensions = {
    mobile: { width: 420, height: 420 },
    tablet: { width: 520, height: 520 },
    desktop: { width: 620, height: 620 }
  };
  
  return dimensions[breakpoint] || dimensions.desktop;
}

/**
 * Calculate optimal camera position for logo
 * @param {Object} logoSize - Logo dimensions
 * @param {number} canvasAspect - Canvas aspect ratio
 * @returns {Array<number>} Camera position [x, y, z]
 */
export function calculateCameraPosition(logoSize = { width: 1.5, height: 0.3 }, canvasAspect = 1) {
  const distance = Math.max(logoSize.width, logoSize.height) * 3;
  return [0, 0, distance];
}

/**
 * Performance monitoring utilities
 */
export const performanceMonitor = {
  frameCount: 0,
  lastTime: performance.now(),
  fps: 0,
  
  update() {
    this.frameCount++;
    const currentTime = performance.now();
    if (currentTime - this.lastTime >= 1000) {
      this.fps = this.frameCount;
      this.frameCount = 0;
      this.lastTime = currentTime;
    }
    return this.fps;
  },
  
  isPerformanceGood() {
    return this.fps >= 30; // Minimum acceptable FPS
  }
};

/**
 * Default configuration for HeroR3F
 */
export const defaultConfig = {
  spinSpeed: 0.18,
  maxWords: 24,
  words: ['IDEA', 'PAPER', 'MENTOR', 'FUND', 'REVIEW', 'PUBLISH', 'GAPLY'],
  enablePoster: true,
  camera: {
    position: [0, 0, 4.2],
    fov: 45
  },
  lighting: {
    ambient: { intensity: 0.4 },
    directional: { position: [5, 5, 5], intensity: 0.8 },
    fill: { position: [-5, 5, -5], intensity: 0.3, color: '#6366F1' },
    rim: { position: [0, 5, 0], intensity: 0.5, color: '#8B5CF6' }
  },
  shadows: {
    position: [0, -1.2, 0],
    opacity: 0.6,
    blur: 2,
    far: 4.5,
    resolution: 256,
    color: '#4F46E5'
  }
};
