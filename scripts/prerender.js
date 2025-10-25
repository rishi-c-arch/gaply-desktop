// scripts/prerender.js
// Usage: run after `npm run build` and with the build served at BASE (this script will try to start a local serve command).
const puppeteer = require('puppeteer');
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const ROUTES = ['/features', '/pricing', '/career', '/hire-expert']; // <-- adjust to your public marketing routes (excluding / to avoid overwriting main index.html)
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
    const proc = spawn('npx', ['serve', '-s', 'build', '-l', `${PORT}`], {
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
  const browser = await puppeteer.launch({ args: ['--no-sandbox', '--disable-setuid-sandbox'] });
  try {
    serverProc = await startStaticServer();
    const page = await browser.newPage();
    // optional: set a crawler-like UA for snapshotting
    await page.setUserAgent('Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)');
    for (const route of ROUTES) {
      const url = `${BASE}${route}`;
      console.log('Rendering', url);
      try {
        await page.goto(url, { waitUntil: 'networkidle2', timeout: 30000 });
        await page.waitForTimeout(500); // wait a bit for late fetches
        const html = await page.content();
        saveSnapshot(route, html);
      } catch (err) {
        console.error('Error rendering route', route, err && err.message ? err.message : err);
      }
    }
    console.log('Prerender complete.');
  } catch (err) {
    console.error('Prerender failure:', err);
    process.exitCode = 1;
  } finally {
    try { await browser.close(); } catch(e){}
    try { if (serverProc) serverProc.kill(); } catch(e){}
  }
})();
