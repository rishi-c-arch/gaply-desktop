// project_recipes through the mock bridge (Research OS, Phase 8U-3, decision
// 553): the screens' tests answer it by its command name, with the project's
// id as its one argument, and a mock with no answer for it refuses, as for
// every other command.
import { describe, expect, it } from 'vitest';

import { makeMockProjectBridge, PROJECT_COMMANDS } from './projectBridge';
import type { RecipeView } from './projectTypes';

const ID = '0123456789abcdef0123456789abcdef';

const ONE: RecipeView[] = [
  { usedBy: [2], sourceColumns: 4, scores: [{ position: 5, name: 'Average', statistic: 'mean', items: 3 }] },
];

describe('the mock bridge and project_recipes', () => {
  it('answers recipes(projectId) from its command name, with the id as its argument', async () => {
    const b = makeMockProjectBridge({ [PROJECT_COMMANDS.recipes]: ONE });
    await expect(b.recipes(ID)).resolves.toEqual(ONE);
    expect(b.calls).toEqual([[PROJECT_COMMANDS.recipes, { projectId: ID }]]);
  });

  it('refuses it when it has no answer, as every command', async () => {
    const b = makeMockProjectBridge({});
    await expect(b.recipes(ID)).rejects.toThrow(`no mock response for ${PROJECT_COMMANDS.recipes}`);
  });

  it('is listed between runs and runResult, in the order the commands are registered', () => {
    const keys = Object.keys(PROJECT_COMMANDS);
    expect(keys.indexOf('recipes')).toBe(keys.indexOf('runs') + 1);
    expect(keys.indexOf('runResult')).toBe(keys.indexOf('recipes') + 1);
  });
});
