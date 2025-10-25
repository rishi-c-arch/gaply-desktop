// scripts/generate-sitemap.js
const fs = require('fs');
const path = require('path');

const ROUTES = ['/', '/features', '/pricing', '/career', '/hire-expert'];
const BUILD_DIR = path.join(__dirname, '..', 'build');
const SITEMAP_PATH = path.join(BUILD_DIR, 'sitemap.xml');

const baseUrl = 'https://www.gaply.in';
const currentDate = new Date().toISOString().split('T')[0];

function generateSitemap() {
  const sitemap = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${ROUTES.map(route => {
  const priority = route === '/' ? '1.0' : '0.8';
  const changefreq = route === '/' ? 'weekly' : 'monthly';
  return `  <url>
    <loc>${baseUrl}${route}</loc>
    <lastmod>${currentDate}</lastmod>
    <changefreq>${changefreq}</changefreq>
    <priority>${priority}</priority>
  </url>`;
}).join('\n')}
</urlset>`;

  fs.writeFileSync(SITEMAP_PATH, sitemap, 'utf8');
  console.log('Generated sitemap:', SITEMAP_PATH);
}

generateSitemap();
