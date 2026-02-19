# Deploy Gaply frontend to Vercel

The app is already built and configured for Vercel. You only need to log in and deploy.

## Option A: Deploy from your machine (CLI)

1. **Log in to Vercel** (fixes "token is not valid"):
   ```bash
   cd /Users/rishi/Desktop/GAPLY/gaply-react-frontend
   npx vercel login
   ```
   Follow the prompts (email or GitHub).

2. **Deploy to production**:
   ```bash
   npx vercel --prod --yes
   ```
   Or use the npm script:
   ```bash
   npm run deploy
   ```

Your live URL will be printed after the deploy (e.g. `https://gaply-react-xxx.vercel.app`).

---

## Option B: Deploy from GitHub (recommended for ongoing deploys)

1. Push this repo to GitHub (if you haven’t already).
2. Go to [vercel.com](https://vercel.com) → **Add New** → **Project**.
3. Import your repo.
4. Set **Root Directory** to `gaply-react-frontend`.
5. Vercel will use the existing `vercel.json` (build command, output directory, rewrites). Click **Deploy**.

Future pushes to the connected branch will trigger automatic deploys.

---

## Existing Vercel config (`vercel.json`)

- **Build:** `npm run build:prod`
- **Output:** `build`
- **SPA rewrites:** all routes → `index.html`
- **Env:** `REACT_APP_API_BASE_URL`, `REACT_APP_API_URL`, `REACT_APP_ENVIRONMENT` (production backend)

Custom domain (e.g. gaply.in) can be added in the Vercel project **Settings → Domains**.
