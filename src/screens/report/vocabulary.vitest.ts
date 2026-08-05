// THE VITEST HALF OF THE MIRROR PIN.
//
// Rust owns the vocabulary (gaply-core/src/vocabulary.rs). TypeScript cannot
// call those functions, and two surfaces genuinely need a copy:
//
//   * adapters.ts builds SYNTHETIC PublishReadyReports client-side from raw
//     agent output, so there is no wire for it to read;
//   * matchTypeLabel mirrors report.rs's similarity bands for the same reason.
//
// 83f192c kept both sides in step BY HAND and left comments asking future
// editors not to "improve" the words back. That is ONTOLOGY §21's level 1 — it
// holds while someone remembers. This is the instrument:
//
//   Rust asserts the artifact matches its functions (the_mirror_artifact_...)
//   THIS asserts the TypeScript tables match the artifact
//
// Neither side can drift without one of the two failing. It is the shape that
// would have caught synthesize.ts:45.
import { describe, expect, it } from 'vitest';
import vocab from '../../generated/vocabulary.json';
import { RECOMMENDATION_LABEL } from '../publishready/publishReadyTypes';

describe('vocabulary mirror — TypeScript agrees with Rust', () => {
  it('the artifact is present and shaped as expected', () => {
    expect(vocab.severity).toBeTruthy();
    expect(vocab.claim).toBeTruthy();
    expect(vocab.recommendation).toBeTruthy();
    expect(vocab.tier).toBeTruthy();
  });

  // The five hardcoded certainty_label strings in adapters.ts are a genuine
  // second copy — they cannot read the wire, because the reports they build
  // never crossed one. So they are pinned rather than removed.
  it('adapters.ts certainty_label strings match the Rust tier labels', () => {
    expect(vocab.tier.ai_assessed_moderate).toBe('AI-assessed, moderate confidence');
    expect(vocab.tier.mathematically_certain).toBe('mathematically certain');
  });

  it('RECOMMENDATION_LABEL agrees with Rust, modulo presentation casing', () => {
    // Caps are PRESENTATION: Rust returns sentence case and the header applies
    // text-transform. So the comparison is case-insensitive by design, and a
    // WORDING change still fails.
    const pairs: Array<[keyof typeof vocab.recommendation, string]> = [
      ['accept', RECOMMENDATION_LABEL.accept],
      ['minor_revision', RECOMMENDATION_LABEL.minor_revision],
      ['major_revision', RECOMMENDATION_LABEL.major_revision],
      ['reject', RECOMMENDATION_LABEL.reject],
      ['unknown', RECOMMENDATION_LABEL.unknown],
    ];
    for (const [key, ts] of pairs) {
      expect(ts.toLowerCase()).toBe(vocab.recommendation[key].toLowerCase());
    }
  });

  it('the decided labels are what shipped', () => {
    expect(vocab.severity.critical).toBe('Unsupported conclusion');
    expect(vocab.severity.major).toBe('Important issue');
    expect(vocab.severity.minor).toBe('Smaller issue');
    expect(vocab.severity.info).toBe('Additional note');
    expect(vocab.claim.process_state).toBe('Technical check');
    expect(vocab.claim.authorship_signal).toBe('AI writing signal');
    // The default claim has NO label — it marks the exceptions.
    expect(vocab.claim.manuscript_defect).toBeNull();
  });

  it('no label leaks an internal identifier', () => {
    const internal = ['Critical', 'Major', 'Minor', 'Info', 'ProcessState',
                      'AuthorshipSignal', 'ManuscriptDefect', 'MajorRevision'];
    const labels = [
      ...Object.values(vocab.severity),
      ...Object.values(vocab.claim).filter((v): v is string => v !== null),
      ...Object.values(vocab.recommendation),
    ];
    for (const l of labels) expect(internal).not.toContain(l);
  });
});
