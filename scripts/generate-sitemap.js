// scripts/generate-sitemap.js — public URLs for Google, Bing, and other crawlers
const fs = require('fs');
const path = require('path');

const baseUrl = 'https://www.gaply.in';
const ROOT = path.join(__dirname, '..');
const BUILD_DIR = path.join(ROOT, 'build');
const PUBLIC_DIR = path.join(ROOT, 'public');
const SITEMAP_PATH = path.join(BUILD_DIR, 'sitemap.xml');
const PUBLIC_SITEMAP_PATH = path.join(PUBLIC_DIR, 'sitemap.xml');
const currentDate = new Date().toISOString().split('T')[0];

/** @type {{ path: string, priority: string, changefreq: string }[]} */
const ENTRIES = [
  { path: '/', priority: '1.0', changefreq: 'weekly' },
  { path: '/citation-generator', priority: '0.95', changefreq: 'weekly' },
  { path: '/features', priority: '0.95', changefreq: 'weekly' },
  { path: '/journal-matching', priority: '0.9', changefreq: 'weekly' },
  { path: '/paper-search', priority: '0.9', changefreq: 'weekly' },
  { path: '/academic-ai-remover', priority: '0.9', changefreq: 'weekly' },
  { path: '/pricing', priority: '0.85', changefreq: 'weekly' },
  { path: '/download', priority: '0.85', changefreq: 'monthly' },
  { path: '/watch-demo', priority: '0.8', changefreq: 'monthly' },
  { path: '/support', priority: '0.8', changefreq: 'monthly' },
  { path: '/contact', priority: '0.75', changefreq: 'monthly' },
  { path: '/research-hub', priority: '0.8', changefreq: 'weekly' },
  { path: '/conferences-india', priority: '0.85', changefreq: 'weekly' },
  { path: '/statistical-research', priority: '0.8', changefreq: 'monthly' },
  { path: '/document-orchestrator', priority: '0.75', changefreq: 'monthly' },
  { path: '/final-orchestrator', priority: '0.75', changefreq: 'monthly' },
  { path: '/manuscript-upload', priority: '0.75', changefreq: 'monthly' },
  { path: '/career', priority: '0.7', changefreq: 'monthly' },
  { path: '/hire-expert', priority: '0.7', changefreq: 'monthly' },
  { path: '/blog', priority: '0.85', changefreq: 'weekly' },
  { path: '/blog/how-to-write-research-paper-step-by-step', priority: '0.75', changefreq: 'monthly' },
  { path: '/blog/phd-research-proposal-to-publication', priority: '0.75', changefreq: 'monthly' },
  { path: '/blog/turning-thesis-into-published-research', priority: '0.75', changefreq: 'monthly' },
  { path: '/blog/crafting-research-questions-objectives-hypotheses', priority: '0.75', changefreq: 'monthly' },
  { path: '/blog/reporting-findings-research-paper-best-practices', priority: '0.75', changefreq: 'monthly' },
  { path: '/privacy', priority: '0.5', changefreq: 'yearly' },
  { path: '/guides/scopus-indexed-journals-low-apc', priority: '0.72', changefreq: 'monthly' },
  { path: '/guides/fast-publication-scopus-journals-india', priority: '0.72', changefreq: 'monthly' },
  { path: '/guides/ethical-researcher-guide-ai-academic-writing', priority: '0.72', changefreq: 'monthly' },
  { path: '/terms', priority: '0.5', changefreq: 'yearly' },
  { path: '/login', priority: '0.55', changefreq: 'monthly' },
  { path: '/signup', priority: '0.55', changefreq: 'monthly' },
];

function generateSitemap() {
  const sitemap = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${ENTRIES.map(
  ({ path: route, priority, changefreq }) => `  <url>
    <loc>${baseUrl}${route}</loc>
    <lastmod>${currentDate}</lastmod>
    <changefreq>${changefreq}</changefreq>
    <priority>${priority}</priority>
  </url>`
).join('\n')}
</urlset>`;

  fs.writeFileSync(SITEMAP_PATH, sitemap, 'utf8');
  console.log('Generated sitemap:', SITEMAP_PATH);
  if (fs.existsSync(PUBLIC_DIR)) {
    fs.writeFileSync(PUBLIC_SITEMAP_PATH, sitemap, 'utf8');
    console.log('Generated sitemap:', PUBLIC_SITEMAP_PATH);
  }
}

generateSitemap();
