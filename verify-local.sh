#!/bin/bash
# Verify local backend + frontend setup (like production)
set -e
echo "=== Verifying Local Setup ==="

# Check backend on 8080
echo ""
echo "1. Backend (localhost:8080):"
if curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/health 2>/dev/null | grep -qE "200|404"; then
  echo "   ✅ Backend responding"
else
  echo "   ❌ Backend not responding. Start it first (e.g. ./out or go run .)"
  exit 1
fi

# Check frontend build
echo ""
echo "2. Frontend build:"
cd "$(dirname "$0")"
if npm run build 2>&1 | tail -3 | grep -q "build folder"; then
  echo "   ✅ Build succeeded"
else
  echo "   ❌ Build failed"
  exit 1
fi

# Serve and test
echo ""
echo "3. Serving build on port 3000..."
echo "   Open http://localhost:3000/final-orchestrator in browser"
echo "   (Backend must be at localhost:8080)"
npx serve -s build -l 3000
