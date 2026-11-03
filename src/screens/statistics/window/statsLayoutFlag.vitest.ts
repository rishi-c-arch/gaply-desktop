// The Statistics window's flag (Research OS, Phase 8P, decision 449): off by
// default, read from the module's map, never from an .env file (decision 167).
import { describe, expect, it } from 'vitest';

import { FEATURE_FLAGS } from '../../../config/featureFlags';

describe('the statsLayout flag', () => {
  it('is off by default', () => {
    expect(FEATURE_FLAGS.statsLayout).toBe(false);
  });
});
