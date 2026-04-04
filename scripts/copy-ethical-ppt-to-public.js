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
const home = os.homedir();
const defaultCandidates = [
  path.join(home, 'Desktop', "The Ethical Researcher's Guide to AI (2).pptx"),
  path.join(home, 'Downloads', "The Ethical Researcher's Guide to AI (2).pptx"),
  path.join(home, 'Downloads', "The Ethical Researcher's Guide to AI (1).pptx"),
];
const argSrc = process.argv[2];
const src =
  argSrc ||
  defaultCandidates.find((p) => {
    try {
      return fs.existsSync(p);
    } catch {
      return false;
    }
  });

if (!src || !fs.existsSync(src)) {
  console.error('Source not found.');
  if (argSrc) console.error('Path given:', argSrc);
  console.error('Tried defaults:', defaultCandidates.join(', '));
  console.error('Usage: node scripts/copy-ethical-ppt-to-public.js [path-to-file.pptx]');
  console.error('Tip: run from the frontend folder, e.g. cd ~/Desktop/GAPLY/gaply-react-frontend');
  process.exit(1);
}

fs.mkdirSync(destDir, { recursive: true });
fs.copyFileSync(src, dest);
const stat = fs.statSync(dest);
console.log('From:', src);
console.log('OK →', dest, `(${Math.round(stat.size / 1024 / 1024)} MB)`);
