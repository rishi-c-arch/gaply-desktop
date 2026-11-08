// Run every saved result again…'s words (Research OS, Phase 8U-2, decisions
// 537 and 546): each word for word, and every new sentence and label read for
// the words a verdict would use, with three lists copied from where they live.
import { describe, expect, it } from 'vitest';

import { windowMenus } from './windowMenus';
import {
  RERUN_ALL_FAILED,
  RERUN_ALL_GOING_NOTE,
  RERUN_ALL_LEFT_OUT,
  RERUN_ALL_MEMORY,
  RERUN_ALL_NOT_RUN,
  RERUN_ALL_PREPARED,
  RERUN_ALL_REST,
  RERUN_ALL_STOP,
  RERUN_ALL_STOPPED,
  RERUN_ALL_TITLE,
  rerunAllCount,
  rerunAllSentence,
  RERUNS_GOING,
} from './windowWording';

/** alphaWording.vitest.ts:20, copied: the words a verdict on alpha would use (decision 384). */
const VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

/** PairedFlow.vitest.tsx:135, copied: decision 66's words. */
const VERDICT_66 = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;

/** projectWording.vitest.ts:47, copied: words that would pass a verdict on the data, or celebrate. */
const VERDICT_PROJECT = /\b(significant|correct|incorrect|accurate|inaccurate|reliable|unreliable|trustworthy|wrong|invalid|suspicious|fraud)\b|★|✓/i;

const ALL_COUNTS = { identical: 1, different: 1, notCompared: 1, failed: 1, savedNotRead: 1, notSaved: 1, notKnown: 1, notRun: 1, notStarted: 1 };

describe("the list's words, word for word (decision 537)", () => {
  it('the title, the sentences and the labels', () => {
    expect(RERUN_ALL_TITLE).toBe('Run every saved result again');
    expect(rerunAllSentence(1)).toBe(
      'Run the 1 saved result again? Gaply runs it, with the same settings, on the copy of the data file kept in this project. The new run is saved beside the one it repeats, which does not change.'
    );
    expect(rerunAllSentence(3)).toBe(
      'Run all 3 saved results again? Gaply runs each one, in the order they were saved, with the same settings, on the copy of the data file kept in this project. Each new run is saved beside the one it repeats, which does not change.'
    );
    expect(RERUN_ALL_PREPARED).toBe('For a result made on data with scale scores, Gaply first makes the scores again from the recipe kept in this project.');
    expect(RERUN_ALL_LEFT_OUT).toBe('Runs of an earlier result, and results that did not finish, are not included.');
    expect(RERUN_ALL_MEMORY).toBe(
      "A data file is open in this window, and each run holds its own copy of its data in memory beside it. Closing the file first uses less of this computer's memory."
    );
    expect(RERUN_ALL_GOING_NOTE).toBe('The runs go on if you close this. Stop cancels the one running and starts no more.');
    expect(RERUN_ALL_STOP).toBe('Stop');
    expect(RERUN_ALL_NOT_RUN).toBe('It was not run again.');
    expect(RERUN_ALL_FAILED).toBe('The new run failed. Its reason is saved with it.');
    expect(RERUN_ALL_STOPPED).toBe('Stopped.');
    expect(RERUN_ALL_REST).toBe('The rest were not run again.');
    expect(RERUNS_GOING).toBe('Saved results are being run again. Wait for them, or stop them from the Project menu.');
  });

  it('the count: the total, then each count above zero in its order, with the singular', () => {
    const zero = { identical: 0, different: 0, notCompared: 0, failed: 0, savedNotRead: 0, notSaved: 0, notKnown: 0, notRun: 0, notStarted: 0 };
    expect(rerunAllCount(3, { ...zero, identical: 3 })).toBe('3 saved results: 3 identical.');
    expect(rerunAllCount(1, { ...zero, notStarted: 1 })).toBe('1 saved result: 1 not started.');
    expect(rerunAllCount(3, { ...zero, identical: 1, notSaved: 1, notStarted: 1 })).toBe('3 saved results: 1 identical, 1 not saved, 1 not started.');
  });
});

describe('no new word carries a verdict (decision 540)', () => {
  it("none matches alpha's list, decision 66's or the project screens'", () => {
    const label = windowMenus({
      fileOpen: true,
      runGoing: false,
      busy: false,
      capabilities: [],
      projects: true,
      statsCheck: false,
      workspaces: false,
      projectOpen: true,
      actions: { open: () => undefined, close: () => undefined, go: () => undefined, switchWorkspace: () => undefined, help: () => undefined },
    })
      .flatMap((m) => m.groups.flatMap((g) => g.items))
      .find((i) => i.id === 'project-rerun-all')?.label;
    expect(label).toBe('Run every saved result again…');
    const all = [
      RERUN_ALL_TITLE,
      label as string,
      rerunAllSentence(1),
      rerunAllSentence(7),
      RERUN_ALL_PREPARED,
      RERUN_ALL_LEFT_OUT,
      RERUN_ALL_MEMORY,
      RERUN_ALL_GOING_NOTE,
      RERUN_ALL_STOP,
      RERUN_ALL_NOT_RUN,
      RERUN_ALL_FAILED,
      RERUN_ALL_STOPPED,
      RERUN_ALL_REST,
      RERUNS_GOING,
      rerunAllCount(9, ALL_COUNTS),
      rerunAllCount(1, { ...ALL_COUNTS, different: 0, notCompared: 0, failed: 0, savedNotRead: 0, notSaved: 0, notKnown: 0, notRun: 0, notStarted: 0 }),
    ];
    expect(all).toHaveLength(16);
    for (const s of all) {
      expect(s, s).not.toMatch(VERDICT);
      expect(s, s).not.toMatch(VERDICT_66);
      expect(s, s).not.toMatch(VERDICT_PROJECT);
    }
  });
});
