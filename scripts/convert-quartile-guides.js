#!/usr/bin/env node
/**
 * Converts quartile guide docx files to HTML with gaply.in watermark.
 * Run locally when .docx sources change: npm run convert-quartile-guides
 * Output: public/quartile-guides/q1-guide.html, q2-q3-guide.html, q4-guide.html
 * (Committed to git; production/Vercel build does not run this — avoids mammoth on CI.)
 */

const fs = require('fs');
const path = require('path');
const mammoth = require('mammoth');

const SOURCE_DIR = path.join(__dirname, '../public/quartile-guides/source');
const OUTPUT_DIR = path.join(__dirname, '../public/quartile-guides');

const WATERMARK_HTML = `
<style>
  .gaply-watermark {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    pointer-events: none;
    z-index: 9999;
    overflow: hidden;
  }
  .gaply-watermark::after {
    content: "gaply.in";
    position: absolute;
    font-size: 4rem;
    font-weight: 700;
    color: rgba(0, 0, 0, 0.04);
    white-space: nowrap;
    transform: rotate(-30deg);
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%) rotate(-30deg);
    font-family: system-ui, -apple-system, sans-serif;
    letter-spacing: 0.1em;
  }
  .guide-content {
    position: relative;
    z-index: 1;
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
    font-family: Georgia, serif;
    line-height: 1.6;
    color: #1a1a1a;
  }
  .guide-content h1, .guide-content h2, .guide-content h3 { margin-top: 1.5em; }
  .guide-content p { margin: 0.75em 0; }
  .guide-content table { border-collapse: collapse; width: 100%; margin: 1em 0; }
  .guide-content th, .guide-content td { border: 1px solid #ddd; padding: 8px; text-align: left; }
  .guide-content th { background: #f5f5f5; }
  .guide-header {
    text-align: center;
    padding: 1rem 0 2rem;
    border-bottom: 1px solid #eee;
    margin-bottom: 2rem;
  }
  .guide-header a { color: #6366f1; text-decoration: none; }
  .guide-header a:hover { text-decoration: underline; }
</style>
<div class="gaply-watermark" aria-hidden="true"></div>
`;

function wrapWithPage(html, title) {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${title} | Gaply</title>
  <link rel="icon" href="/favicon.ico">
</head>
<body>
  <div class="guide-header">
    <a href="/">← Back to Gaply</a>
  </div>
  <div class="guide-content">
    ${html}
  </div>
  ${WATERMARK_HTML}
</body>
</html>`;
}

async function convertDocx(inputPath, outputPath, title) {
  const buffer = fs.readFileSync(inputPath);
  const result = await mammoth.convertToHtml({ buffer });
  const wrapped = wrapWithPage(result.value, title);
  fs.writeFileSync(outputPath, wrapped, 'utf8');
  console.log(`Converted: ${path.basename(inputPath)} → ${path.basename(outputPath)}`);
}

async function main() {
  if (!fs.existsSync(OUTPUT_DIR)) fs.mkdirSync(OUTPUT_DIR, { recursive: true });

  await convertDocx(
    path.join(SOURCE_DIR, 'Q1_Journals_Complete_Guide.docx'),
    path.join(OUTPUT_DIR, 'q1-guide.html'),
    'Q1 Journals Complete Guide'
  );
  await convertDocx(
    path.join(SOURCE_DIR, 'Q2_Q3_Journals_Complete_Guide.docx'),
    path.join(OUTPUT_DIR, 'q2-q3-guide.html'),
    'Q2 & Q3 Journals Complete Guide'
  );
  await convertDocx(
    path.join(SOURCE_DIR, 'Q4_Journals_Complete_Guide.docx'),
    path.join(OUTPUT_DIR, 'q4-guide.html'),
    'Q4 Journals Complete Guide'
  );

  console.log('Done. HTML guides with watermark created.');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
