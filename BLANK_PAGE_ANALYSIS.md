# Deep Analysis: Why Blank Page on Production (Works Locally)

## Root Cause Chain

### 1. **Three.js/WebGL in Main Bundle**
- **LoginPage**, **SignupPage**, **EnhancedPremiumPage** (and others) use `@react-three/fiber` + `three`
- These are **statically imported** in App.tsx
- Webpack bundles them into the **main.js** chunk
- **Result:** When ANY page loads (including /final-orchestrator), the main bundle loads Three.js

### 2. **WebGL Context Lost**
- Three.js creates a WebGL context when the bundle loads
- On production: different GPU, mobile, incognito, or resource limits can cause "Context Lost"
- **Local works:** Your Mac has stable WebGL, no context limits
- **Production fails:** User's device, browser, or environment loses WebGL context → blank page

### 3. **0 Network Requests**
- Suggests aggressive caching (browser serving old broken HTML from cache)
- Or: page loaded from bfcache, Network tab cleared
- Old cached HTML might be the prerender-corrupted version (Chromium error page)

## Why Local ≠ Production

| Factor | Local | Production |
|--------|-------|------------|
| WebGL | Your Mac's GPU, stable | User's device, varies |
| Bundle | Same code, different env | Minified, different chunk hashes |
| Cache | No CDN, fresh | Vercel CDN, browser cache |
| URL | localhost:3000 | gaply.in → www.gaply.in |
| Incognito | N/A | Can restrict WebGL in some browsers |

## Fix: Route-Based Code Splitting

Lazy load **all** route components so /final-orchestrator never loads Three.js.
