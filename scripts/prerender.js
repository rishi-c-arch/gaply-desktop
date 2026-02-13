// scripts/prerender.js
// Usage: run after `npm run build` and with the build served at BASE (this script will try to start a local serve command).
const puppeteer = require('puppeteer');
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const ROUTES = ['/', '/features', '/pricing', '/career', '/hire-expert']; // Test homepage first to see if it renders
const BUILD_DIR = path.join(__dirname, '..', 'build');
const PORT = process.env.PRERENDER_PORT || 5000;
const BASE = `http://127.0.0.1:${PORT}`;

function saveSnapshot(route, html) {
  const clean = route === '/' ? '' : route.replace(/^\//, '');
  const outDir = path.join(BUILD_DIR, clean);
  if (!fs.existsSync(outDir)) fs.mkdirSync(outDir, { recursive: true });
  const outFile = path.join(outDir, 'index.html');
  fs.writeFileSync(outFile, html, 'utf8');
  console.log('Saved snapshot:', outFile);
}

// Start a local static server using 'serve' (npx) so we don't add another dependency
function startStaticServer() {
  return new Promise((resolve, reject) => {
    console.log('Starting static server on port', PORT);
    const cwd = path.join(__dirname, '..');
    const proc = spawn('npx', ['serve', '-s', 'build', '-l', `${PORT}`], {
      cwd,
      stdio: ['ignore', 'pipe', 'pipe'],
      shell: process.platform === 'win32'
    });
    proc.stdout.on('data', d => {
      const s = d.toString();
      process.stdout.write(s);
      if (s.toLowerCase().includes('accepting connections') || s.toLowerCase().includes('listening on')) {
        resolve(proc);
      }
    });
    proc.stderr.on('data', d => process.stderr.write(d.toString()));
    proc.on('error', err => reject(err));
    // fallback timeout if the serve output doesn't match
    setTimeout(() => resolve(proc), 3000);
  });
}

(async () => {
  let serverProc;
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox']
  });
  try {
    serverProc = await startStaticServer();
    const page = await browser.newPage();

    // Block API/backend requests during prerender - they cause 403 and aren't needed for HTML snapshot
    await page.setRequestInterception(true);
    page.on('request', (req) => {
      const u = req.url();
      const isApi =
        u.includes(':8080') ||
        u.includes('gaply-backend') ||
        u.includes('backend.gaply') ||
        u.includes('/api/');
      if (isApi) {
        req.abort();
      } else {
        req.continue();
      }
    });

    // Log console messages and errors from the page (skip 403 noise from blocked API)
    page.on('console', msg => {
      const t = msg.text();
      if (!t.includes('403') && !t.includes('Failed to load resource')) console.log('PAGE LOG:', t);
    });
    page.on('pageerror', error => console.log('PAGE ERROR:', error.message));

    // Use normal Chrome UA - serve blocks Googlebot/crawlers with 403
    await page.setUserAgent('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36');
    for (const route of ROUTES) {
      const url = `${BASE}${route}`;
      console.log('Rendering', url);
      try {
        await page.goto(url, { waitUntil: 'networkidle2', timeout: 30000 });
        
        // Wait for React to render - wait for root div to have content
        await page.waitForFunction(
          () => {
            const root = document.getElementById('root');
            return root && root.innerHTML.length > 100;
          },
          { timeout: 15000 }
        ).catch(() => console.log('Warning: React content may not be fully loaded for', route));
        
        // Wait for SEOHead useEffect to run and update meta tags
        await page.waitForTimeout(3000);
        
        // Double-check that title has been updated (indicates SEOHead ran)
        const pageTitle = await page.title();
        console.log(`Page title for ${route}: "${pageTitle}"`);
        
        const html = await page.content();
        
        // Verify we have actual content, not just the shell
        if (html.includes('<div id="root"></div>') || html.length < 1000) {
          console.warn(`Warning: Route ${route} may not have rendered properly (content length: ${html.length})`);
        } else {
          console.log(`✓ Route ${route} rendered successfully (content length: ${html.length})`);
        }
        
        saveSnapshot(route, html);
      } catch (err) {
        console.error('Error rendering route', route, err && err.message ? err.message : err);
      }
    }
    console.log('Prerender complete.');
  } catch (err) {
    console.error('Prerender failure:', err);
    // Don't fail build - deploy can proceed with base build output
  } finally {
    try { await browser.close(); } catch(e){}
    try { if (serverProc) serverProc.kill(); } catch(e){}
  }
})();
