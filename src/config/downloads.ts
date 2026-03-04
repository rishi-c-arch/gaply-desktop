/**
 * Desktop app download base URL.
 * Uses R2 for direct downloads (no sign-in required).
 */
const GITHUB_REPO = 'RishiSTARP/gaply-frontend-production';
const R2_DOWNLOAD_BASE = 'https://pub-1789da295ffa40a5be8ad337f5d02859.r2.dev';
const DOWNLOAD_BASE = R2_DOWNLOAD_BASE;

export const RELEASE_BASE = `https://github.com/${GITHUB_REPO}/releases`;
export const DOWNLOAD_LINKS = {
  macArm64: `${DOWNLOAD_BASE}/Gaply-mac-arm64.dmg`,
  windows: `${DOWNLOAD_BASE}/Gaply-windows.exe`,
} as const;
