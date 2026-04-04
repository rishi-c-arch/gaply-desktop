#!/usr/bin/env node
const fs = require('fs');
const path = require('path');
const os = require('os');

const root = path.join(__dirname, '..');
const destDir = path.join(root, 'public', 'guides');
const dest = path.join(destDir, 'ethical-researcher-guide-ai-2026.pdf');
const home = os.homedir();
const defaultCandidates = [
  path.join(home, 'Desktop', "The Ethical Researcher's Guide to AI (2).pdf"),
  path.join(home, 'Downloads', "The Ethical Researcher's Guide to AI (2).pdf"),
  path.join(home, 'Downloads', "The Ethical Researcher's Guide to AI (1).pdf"),
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
  console.error('Source PDF not found.');
  if (argSrc) console.error('Path given:', argSrc);
  console.error('Tried defaults:', defaultCandidates.join(', '));
  console.error('Export from PowerPoint: File → Export → PDF, then:');
  console.error('  node scripts/copy-ethical-pdf-to-public.js /path/to/deck.pdf');
  process.exit(1);
}

fs.mkdirSync(destDir, { recursive: true });
fs.copyFileSync(src, dest);
const stat = fs.statSync(dest);
console.log('From:', src);
console.log('OK →', dest, `(${Math.round(stat.size / 1024 / 1024)} MB)`);
