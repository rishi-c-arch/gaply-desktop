#!/usr/bin/env bash
# Deploy this frontend to Vercel (production). Run from this directory.
# Usage: ./deploy.sh

set -e
cd "$(dirname "$0")"

echo "==> Building and deploying to Vercel (production)..."
npm run deploy
echo ""
echo "==> Frontend deployed. Production URL shown above."
