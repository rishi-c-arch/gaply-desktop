# Deployment Audit – Blank Page & 503 Fix

**Date:** Feb 16, 2026  
**Issue:** Blank page at `gaply.in/final-orchestrator` + 503 from backend

---

## Root Causes Identified

### 1. **Wrong backend URL (503)**
- **Problem:** App was calling `gaply-backend-production.up.railway.app`, which returns 503.
- **Cause:** `config.ts` used `FALLBACK_URLS[1]` (gaply-backend-production) when env vars were empty.
- **Fix:** Default fallback changed to `gaply-backend-gaply.up.railway.app`.

### 2. **Vercel `vercel.json` env is deprecated**
- **Problem:** `vercel.json` `env` block may not be applied at build time.
- **Fix:** Add env vars in **Vercel Dashboard → Project → Settings → Environment Variables**:
  - `REACT_APP_API_BASE_URL` = `https://gaply-backend-gaply.up.railway.app`
  - `REACT_APP_API_URL` = `https://gaply-backend-gaply.up.railway.app`
  - Scope: Production

### 3. **Prerender was overwriting index.html**
- **Problem:** Prerender script replaced `build/index.html` with a Chromium error page.
- **Fix:** Prerender removed from `postbuild` in `package.json`.

### 4. **Domain: gaply.in vs www.gaply.in**
- Vercel alias is `www.gaply.in`. Use `https://www.gaply.in/final-orchestrator` for testing.
- Ensure `gaply.in` redirects to `www.gaply.in` in DNS/Vercel.

---

## Changes Made

| File | Change |
|------|--------|
| `src/api/config.ts` | Default fallback → `gaply-backend-gaply` |
| `package.json` | Removed prerender from postbuild |
| `.env.production` | Set to `gaply-backend-gaply` |

---

## Checklist Before Next Deploy

- [ ] Add env vars in Vercel Dashboard (see above)
- [ ] Confirm Railway backend `gaply-backend-gaply` is running
- [ ] Push changes and let Vercel auto-deploy, or run `npm run deploy`
- [ ] Test `https://www.gaply.in/final-orchestrator` (not gaply.in)

---

## Backend (Railway)

If 503 persists:

1. Open [Railway Dashboard](https://railway.app)
2. Select project `gaply-backend-gaply`
3. Check service status and logs
4. Ensure `OPENAI_API_KEY` and other env vars are set
5. Check that `/api/v1/upload` is handled by the backend
