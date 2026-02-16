#!/usr/bin/env node
// Copy index.html to final-orchestrator/index.html so /final-orchestrator serves the SPA
const fs = require('fs');
const path = require('path');

const buildDir = path.join(__dirname, '..', 'build');
const indexPath = path.join(buildDir, 'index.html');
const targetDir = path.join(buildDir, 'final-orchestrator');
const targetPath = path.join(targetDir, 'index.html');

if (!fs.existsSync(indexPath)) {
  console.warn('copy-final-orchestrator: build/index.html not found, skipping');
  process.exit(0);
}

if (!fs.existsSync(targetDir)) {
  fs.mkdirSync(targetDir, { recursive: true });
}
fs.copyFileSync(indexPath, targetPath);
console.log('copy-final-orchestrator: copied index.html to final-orchestrator/index.html');
