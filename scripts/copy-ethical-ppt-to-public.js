#!/usr/bin/env node
/**
 * Copy the ethical AI deck into public/ so embed + download work.
 * Run this in YOUR terminal (Cursor sidebar → Terminal, or Terminal.app), not via the AI agent,
 * because agent shells often cannot read ~/Downloads.
 */
const fs = require('fs');
const path = require('path');
const os = require('os');

const root = path.join(__dirname, '..');
const destDir = path.join(root, 'public', 'guides');
const dest = path.join(destDir, 'ethical-researcher-guide-ai-2026.pptx');
const defaultSrc = path.join(
  os.homedir(),
  'Downloads',
  "The Ethical Researcher's Guide to AI (1).pptx"
);
const src = process.argv[2] || defaultSrc;

if (!fs.existsSync(src)) {
  console.error('Source not found:', src);
  console.error('Usage: node scripts/copy-ethical-ppt-to-public.js [path-to-file.pptx]');
  console.error('Tip: run from the frontend folder, e.g. cd ~/Desktop/GAPLY/gaply-react-frontend');
  console.error('Or use the full path to this script (see Ethical AI guide page on the site).');
  process.exit(1);
}

fs.mkdirSync(destDir, { recursive: true });
fs.copyFileSync(src, dest);
const stat = fs.statSync(dest);
console.log('OK →', dest, `(${Math.round(stat.size / 1024 / 1024)} MB)`);
