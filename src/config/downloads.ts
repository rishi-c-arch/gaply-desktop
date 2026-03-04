/**
 * GitHub release base URL for desktop app downloads.
 * Update GITHUB_REPO if your releases are in a different repo.
 */
const GITHUB_REPO = 'RishiSTARP/gaply-frontend-production';
export const RELEASE_BASE = `https://github.com/${GITHUB_REPO}/releases`;
export const DOWNLOAD_BASE = `${RELEASE_BASE}/latest/download`;

export const DOWNLOAD_LINKS = {
  macArm64: `${DOWNLOAD_BASE}/Gaply-mac-arm64.dmg`,
  windows: `${DOWNLOAD_BASE}/Gaply-windows.exe`,
} as const;
