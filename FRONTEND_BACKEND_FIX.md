# Frontend + Backend Fix Guide

## Current Status

- **Frontend**: Page may show blank due to cache or deployment lag
- **Backend**: Analysis fails when form is submitted
- **Local**: Works (gaply-react at localhost:3000 + backend at localhost:8080)
- **Production**: gaply.in uses gaply-react-frontend + Railway backend

---

## 1. FRONTEND FIX (Blank Page)

### A. Deploy Latest Changes

```bash
cd /Users/rishi/Desktop/GAPLY/gaply-react-frontend
git status
git add -A
git commit -m "Fix blank page: app-content-below-header, loading overlay, error handlers"
git push origin main
```

Vercel will auto-deploy. Wait 2–3 minutes.

### B. Clear Cache (User-Side)

If still blank after deploy:
- **Hard refresh**: `Cmd+Shift+R` (Mac) or `Ctrl+Shift+R` (Windows)
- **Incognito**: Open `https://www.gaply.in/final-orchestrator` in a new incognito window
- **Clear site data**: DevTools → Application → Storage → Clear site data

### C. Use Correct URL

- ✅ `https://www.gaply.in/final-orchestrator` (with www)
- ❌ `gaply.in/final-orchestrator` (redirects; may cause issues)

---

## 2. BACKEND FIX (Analysis Fails)

### Required Backend Endpoints

The frontend calls these on `https://gaply-backend-gaply.up.railway.app`:

| Endpoint | Purpose |
|----------|---------|
| `POST /api/v1/upload` | File upload |
| `GET /api/ai/guidelines` | Journal guidelines |
| `GET /api/ai/retrieved-docs` | Retrieved documents |
| `POST /api/ai/document-orchestrator` | Main analysis |
| `POST /api/ai/publication-chance` | Publication chance |
| `POST /api/ai/referee-review` | Referee review |
| `POST /api/ai/line-review` | Line review |
| `POST /api/ai/chat` | Chat concierge |

### Railway Backend Checklist

1. **ORCHESTRATOR_URL** – **CRITICAL** – Set to your PublishReady orchestrator URL (e.g. `https://gaply-orchestrator.up.railway.app`). Without this, all `/api/ai/*` calls return 503 "PublishReady orchestrator not configured".

2. **CORS** – Allow `https://www.gaply.in` and `https://gaply.in`:
   ```
   Access-Control-Allow-Origin: https://www.gaply.in
   ```

3. **Upload response** – `gaply-production-backend` `/api/v1/upload` must return `extracted_text` in the JSON response (fixed in paraphrasing.go).

4. **Proxy** – `gaply-production-backend` proxies these to ORCHESTRATOR_URL:
   - `/api/ai/guidelines`, `/api/ai/retrieved-docs`, `/api/ai/document-orchestrator`, etc.
   - `/api/v1/upload` is handled by the backend (paraphrasingHandler) and must return `extracted_text`.

5. **Two services on Railway**:
   - `gaply-backend` – main API (must have ORCHESTRATOR_URL)
   - `gaply-orchestrator` – PublishReady orchestrator (standalone-orchestrator or publishready-orchestrator)

6. **Logs** – In Railway dashboard, check both backend and orchestrator logs when analysis is submitted

### Test Backend Directly

```bash
# Health check
curl -s https://gaply-backend-gaply.up.railway.app/health

# Test upload (replace with real file)
curl -X POST https://gaply-backend-gaply.up.railway.app/api/v1/upload \
  -F "file=@test.pdf"
```

---

## 3. LOCAL vs PRODUCTION

| | Local | Production |
|---|-------|------------|
| **Frontend** | gaply-react (port 3000) | gaply-react-frontend (Vercel) |
| **Backend** | localhost:8080 | gaply-backend-gaply.up.railway.app |
| **Codebase** | Different! | Must deploy gaply-react-frontend |

Local uses `gaply-react`; production uses `gaply-react-frontend`. They are different folders. Changes in gaply-react do **not** affect production until applied to gaply-react-frontend and pushed.

---

## 4. Quick Verification

1. Open `https://www.gaply.in/final-orchestrator` in incognito
2. You should see: "Document Analysis Orchestrator" title, upload area, journal inputs, Generate Report button
3. If blank: hard refresh, check DevTools Console for errors
4. If form shows but analysis fails: check Railway backend logs and ORCHESTRATOR_URL
