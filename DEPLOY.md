# Deploy Gaply React to Vercel

## Quick Deploy (Copy & Paste)

```bash
cd /Users/rishi/Desktop/GAPLY
git add gaply-react/
git commit -m "Add PublishReady & DataMaestro to sidebar, light mode toggle"
git push origin main
```

**Vercel deploys from:** `RishiSTARP/gaply-react-frontend` — ensure your code is in that repo, or reconnect Vercel to `gaply-frontend-production` with Root Directory `gaply-react`.

---

## Option 1: Deploy via Git Push

If Vercel is connected to your GitHub repo, pushing triggers automatic deployment.

```bash
cd /Users/rishi/Desktop/GAPLY
git add gaply-react/
git commit -m "Add PublishReady & DataMaestro to sidebar"
git push origin main
```

---

## Option 2: Deploy via Vercel CLI

```bash
cd /Users/rishi/Desktop/GAPLY/gaply-react
npm run build
npx vercel --prod
```

---

## Option 3: npm deploy script

```bash
cd /Users/rishi/Desktop/GAPLY/gaply-react
npm run deploy
```

---

## Troubleshooting

- **Build fails:** Run `npm run build` in gaply-react folder
- **Wrong repo:** Vercel → Settings → Git → connect `gaply-react-frontend` or `gaply-frontend-production`
- **Root directory:** Set to `gaply-react` if using monorepo
