// REACT_APP_FEATURE_STORED_DATA and the one helper that decides whether the
// Stored data section shows (Research OS, Phase 5D-1a; decision 206): off by
// default, and on only in the desktop app.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it, vi } from 'vitest';

import { storedDataShown } from './storedDataGate';

const flags = vi.hoisted(() => ({ storedData: false }));
vi.mock('../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'storedData' && flags.storedData,
}));

describe('REACT_APP_FEATURE_STORED_DATA', () => {
  it('is off by default', async () => {
    const real = await vi.importActual<typeof import('../../config/featureFlags')>('../../config/featureFlags');
    expect(process.env.REACT_APP_FEATURE_STORED_DATA).toBeUndefined();
    expect(real.FEATURE_FLAGS.storedData).toBe(false);
    expect(real.isFeatureEnabled('storedData')).toBe(false);
  });

  it('is set by no committed .env file', () => {
    const root = path.join(__dirname, '../../..');
    const envs = fs.readdirSync(root).filter((f) => f.startsWith('.env'));
    for (const f of envs) {
      expect(fs.readFileSync(path.join(root, f), 'utf8'), f).not.toMatch(/REACT_APP_FEATURE_STORED_DATA\s*=\s*(true|1)/);
    }
  });
});

describe('storedDataShown', () => {
  it.each([
    [false, false, false],
    [true, false, false],
    [false, true, false],
    [true, true, true],
  ])('storedData=%s desktop=%s gives %s', (storedData, desktop, shown) => {
    flags.storedData = storedData;
    expect(storedDataShown(desktop)).toBe(shown);
  });
});
