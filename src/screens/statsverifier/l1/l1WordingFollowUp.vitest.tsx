// Phase 2 evaluation fixes, part C follow-up: the minor wording part C
// proposed, one test each, and the sentences for the two codes this step
// added (a printed p the printed statistic rules out; a workbook that unpacks
// to more than it declares).
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { L1ResultCard, storedRange } from './L1ResultCard';
import { NOT_REPRODUCED_TEXT, level0Phrase, refusalSentence, sentenceFor } from './l1Wording';
import { RECIPIENT_TEXT, WEB_RETENTION_TEXT } from './L1AiPanel';
import { KEY_TEXT } from './L1Settings';
import { L0ResultCard } from '../../checks/StatsConsistencySection';
import type { L0ResultView } from '../../checks/statsConsistencyTypes';
import type { StatsL1Bridge } from '../statsL1Bridge';
import type { L1ClaimReproductionView, L1ClaimView, L1NumberView, WireCode } from '../statsL1Types';

afterEach(cleanup);

const claim: L1ClaimView = {
  claimId: 0, revision: 'rev0', sectionIndex: 2, section: 'results', paragraph: 1, sentenceIndex: 0,
  sentence: 'Scores rose, t(38) = 2.10, p = .042.', family: 't', statistic: '2.10', df: ['38'], n: null, p: '= .042', level0: { code: 'consistent' },
};
const NUMBER: L1NumberView = { kind: { code: 'statistic' }, printed: '2.10', relation: '=', computed: 2.1034, acceptedLo: 2.095, acceptedHi: 2.105, status: { code: 'matched' }, precisionLo: null, precisionHi: null };

function card(variant: WireCode, setBy: WireCode, provenance: L1ClaimReproductionView['provenance'] = null): string {
  const r: L1ClaimReproductionView = {
    claimId: 0, sectionIndex: 2, paragraph: 1, sentence: 0, method: 'independent_t', variant, variantSetBy: setBy,
    verdict: { kind: 'reproduced', notes: [] }, notes: [],
    candidates: [{ variant: { code: 'student_pooled' }, storedDecimals: null, numbers: [NUMBER] }],
    nUsed: 40, rowsRead: 40, exclusions: [], provenance, wholeNumbers: [],
  };
  render(
    <L1ResultCard bridge={{} as StatsL1Bridge} run={7} claim={claim} result={r} alpha={{ printed: ['.05'], stated: false }} ctx={{ column: (a) => `C${a}` }} catalogue={[]} aiHelp={false} webContext={false} onUsage={() => undefined} />
  );
  return screen.getByTestId('l1-result-0').textContent ?? '';
}

describe('part C follow-up: minor wording', () => {
  it('"Variant: not stated" is said once; who set it is said only for a stated variant', () => {
    expect(screen.queryByText(/not stated, not stated/)).toBeNull();
    const notStated = card({ code: 'not_stated' }, { code: 'not_stated' });
    expect(notStated).toContain('Variant: not stated.');
    expect(notStated).not.toMatch(/not stated, /);
    cleanup();
    const stated = card({ code: 'welch' }, { code: 'user_ticked' });
    expect(stated).toMatch(/Variant: [^.]+, [^.]+\./);
  });

  it('granularity_unknown says what is needed and that Gaply never assumes it', () => {
    expect(sentenceFor({ code: 'granularity_unknown' })).toBe(
      'This check needs to know the data are whole numbers; Gaply never assumes that, so it was not run.'
    );
  });

  it("Level 0's conclusion line says what the printed statistic implies, and when alpha was assumed", () => {
    const r: L0ResultView = {
      id: 1, location: { section: 'results', paragraph: 1, section_index: 2 }, raw: 't(28) = 1.20, p = .001', reportedP: '.001', reportedPRelation: '=',
      recomputedP: { lo: 0.24, hi: 0.25 }, verdict: { kind: 'inconsistent', conclusionChanges: true },
      check: { methodId: 'p_consistency', methodName: 'p-value consistency', engineVersion: 'e', inputsDigest: 'd', toleranceText: 't' } as unknown as L0ResultView['check'],
    };
    render(<L0ResultCard result={r} alpha=".05" alphaAssumed />);
    expect(screen.getByTestId('l0-conclusion').textContent).toBe(
      'If the statistic and df are as printed, their p falls on the other side of α = .05 (assumed; the paper states none) from the printed p.'
    );
  });

  it('Not reproduced names a binding that differs from the paper\'s analysis as a cause', () => {
    expect(NOT_REPRODUCED_TEXT).toContain("a binding that differs from the paper's analysis");
    expect(NOT_REPRODUCED_TEXT).not.toMatch(/wrong|error/i);
  });

  it('too_large states the sizes in MB', () => {
    expect(sentenceFor({ code: 'too_large', bytes: 12_582_912, limit: 10_485_760 })).toBe('The data file is 12.0 MB, over the 10.0 MB limit.');
    expect(sentenceFor({ code: 'too_large', bytes: 10_485_761, limit: 10_485_760 })).toBe('The data file is just over the 10.0 MB limit.');
  });

  it("the stored range reads as what the file's rounding allows", () => {
    const n = { ...NUMBER, precisionLo: 2.071, precisionHi: 2.1358 };
    expect(storedRange(n, 2)).toBe(" (with the file's 2-decimal rounding it could be anywhere from 2.071 to 2.1358)");
    expect(storedRange(n, 0)).toBe(" (with the file's rounding to whole numbers it could be anywhere from 2.071 to 2.1358)");
  });

  it('a refusal shows its own message or a plain sentence, never "Something went wrong" or a raw "Refused: code"', () => {
    expect(refusalSentence(new Error('The run is closed'))).toBe('This could not be done: The run is closed.');
    expect(refusalSentence(undefined)).toBe('This could not be done.');
    const unknown = refusalSentence(new Error('{"code":"some_new_code"}'));
    expect(unknown).toBe('This could not be done (some new code).');
    expect(unknown).not.toMatch(/Refused:|some_new_code/);
  });

  it('one term each: "could not check" and "inconsistent"; the method is named, never its id', () => {
    expect(sentenceFor({ code: 'p_could_not_check', reason: 'adjusted_p' })).toMatch(/^Could not check the p: /);
    expect(sentenceFor({ code: 'p_not_checked' })).toBe('The p was not decided, so a change in conclusion was not assessed.');
    expect(level0Phrase({ code: 'inconsistent', conclusion_changes: 'no' })).toBe('Level 0: inconsistent (these numbers cannot all be correct as printed)');
    const text = card({ code: 'not_stated' }, { code: 'not_stated' }, {
      methodId: 'independent_t', methodVersion: '1', engineVersion: 'e', reference: null, bindingSource: { code: 'manual' },
      variantSetBy: { code: 'not_stated' }, inputsDigest: 'abc', check: null,
    } as unknown as L1ClaimReproductionView['provenance']);
    expect(text).not.toContain('independent_t');
  });

  it('the web and billing lines say only what the code makes true', () => {
    // stats_ai.rs: the only host is OPENAI_BASE, called from this device with the user's key.
    expect(RECIPIENT_TEXT).toBe('Sent from this device straight to OpenAI over HTTPS, with your own API key. No Gaply server is in between, so none receives it.');
    expect(RECIPIENT_TEXT).not.toMatch(/sees nothing/);
    // outbound.rs: one hosted web_search tool, filtered to the allowed domains; what OpenAI does next is not Gaply's code.
    expect(WEB_RETENTION_TEXT).not.toMatch(/sends the query on to the sites/);
    expect(WEB_RETENTION_TEXT).toContain('limited to a fixed list of sites');
    expect(KEY_TEXT).toContain('Every request is made with your key, so OpenAI bills your account for it');
    expect(KEY_TEXT).not.toMatch(/for what you send\./);
  });

  it('r printed as ±1.00 is explained as a rounding, not called impossible', () => {
    expect(sentenceFor({ code: 'printed_out_of_range' })).toContain('A correlation printed as ±1.00 can be a rounding of a value just inside ±1.');
  });

  it('the two codes this step added read as plain sentences', () => {
    expect(sentenceFor({ code: 'not_matched', why: 'p_ruled_out_by_printed_statistic' })).toBe(
      "The printed p cannot follow from the printed statistic and its degrees of freedom, whatever the data's rounding."
    );
    expect(sentenceFor({ code: 'not_matched' })).toBe('Differs from the value computed from the data under this binding.');
    expect(sentenceFor({ code: 'conclusion_not_assessed', gap: 'p_judged_on_printed_numbers' })).toMatch(/^The p was judged against the printed statistic/);
    expect(sentenceFor({ code: 'archive_too_large', declared: 4097, limit: 67_108_864 })).toBe(
      'This file unpacks to more than its own contents list says, so it was not opened.'
    );
    expect(sentenceFor({ code: 'archive_too_large', declared: 1_572_865_485, limit: 67_108_864 })).toMatch(/^This file would take at least 1500 MB once unpacked, more than the 64 MB/);
  });
});
