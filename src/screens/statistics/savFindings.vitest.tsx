// What the SPSS reader noted about a file (Research OS, Phase 8I, decision
// 365): each closed code's sentence, word for word, and where it shows,
// under the profile of the five methods' flows (ProfileTable) and of the
// descriptive summary's own. A view with no finding shows no list.
import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import DescribeFlow from './DescribeFlow';
import { makeMockDescribeBridge } from './describeBridge';
import type { DatasetView, SavFinding } from './describeTypes';
import { ProfileTable } from './flowParts';
import { savFindingSentence } from './savFindingWording';

vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

afterEach(cleanup);

const finding = (code: string, params: Record<string, number>): SavFinding => ({ code, params } as SavFinding);

const view = (fileFindings?: SavFinding[]): DatasetView => ({
  slot: 1,
  displayName: 'zero.sav',
  profile: {
    rows: 20,
    columns: [{ alias: 'C1', header: 'ITEM1', kind: 'numeric', n: 20, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0 }],
  },
  inputRef: { contentHash: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef', rows: 20, columns: 1, semanticRole: 'table' },
  ...(fileFindings ? { fileFindings } : {}),
});

describe('the SPSS reader findings', () => {
  it('says each code in its own words, with its numbers', () => {
    const cases: [SavFinding, string][] = [
      [
        finding('sav.header_rows_mismatch', { declared: 0, read: 20 }),
        "The file's header says it holds 0 cases, but 20 were read. Gaply uses the 20 it read.",
      ],
      [
        finding('sav.case_size_mismatch', { declared: 3, used: 2 }),
        "The file's header gives 3 values per case, but its variables define 2. Gaply reads 2.",
      ],
      [
        finding('sav.case_size_missing', { used: 2 }),
        "The file's header does not give its values per case. Gaply reads the 2 its variables define.",
      ],
      [
        finding('sav.dates_out_of_range', { count: 2 }),
        '2 date or time values were outside the years 1 to 9999, and were read as missing.',
      ],
      [
        finding('sav.type_continuation', { record: 2 }),
        "The file's variable record 2 is marked as part of the variable before it. Gaply reads it that way.",
      ],
      [
        finding('sav.compression_bias', { bias: 99.5 }),
        "The file's header gives an offset of 99.5 for its compressed whole numbers, not the usual 100. Gaply reads them with that offset; SPSS reads such a file assuming 100, so these numbers may differ from SPSS's.",
      ],
      [
        finding('sav.zsav_trailer_bias', { bias: 7 }),
        "The file's compressed-data trailer gives an offset of 7, not the usual 100. Gaply reads the numbers with the header's offset, so this changes nothing Gaply reads.",
      ],
      [
        finding('sav.very_long_string_ignored', { count: 2 }),
        '2 long text variables declare a length the file cannot hold. Gaply reads each at the length of its own record, at most 255 bytes, so longer text in them may be cut.',
      ],
      [
        finding('sav.unrecognised_finding', { count: 3 }),
        'The SPSS reader noted 3 other things about this file that Gaply cannot describe yet.',
      ],
    ];
    for (const [f, want] of cases) expect(savFindingSentence(f)).toBe(want);
  });

  // Decision 376: a count of exactly 1 takes the singular; every other
  // number, 0 included, keeps the plural.
  it('says a count of 1 in the singular and 0 or 2 in the plural', () => {
    const cases: [SavFinding, string][] = [
      [
        finding('sav.header_rows_mismatch', { declared: 1, read: 2 }),
        "The file's header says it holds 1 case, but 2 were read. Gaply uses the 2 it read.",
      ],
      [
        finding('sav.header_rows_mismatch', { declared: 0, read: 1 }),
        "The file's header says it holds 0 cases, but 1 was read. Gaply uses the 1 it read.",
      ],
      [
        finding('sav.header_rows_mismatch', { declared: 2, read: 1 }),
        "The file's header says it holds 2 cases, but 1 was read. Gaply uses the 1 it read.",
      ],
      [
        finding('sav.case_size_mismatch', { declared: 1, used: 2 }),
        "The file's header gives 1 value per case, but its variables define 2. Gaply reads 2.",
      ],
      [
        finding('sav.case_size_mismatch', { declared: 2, used: 3 }),
        "The file's header gives 2 values per case, but its variables define 3. Gaply reads 3.",
      ],
      [
        finding('sav.dates_out_of_range', { count: 1 }),
        '1 date or time value was outside the years 1 to 9999, and was read as missing.',
      ],
      [
        finding('sav.dates_out_of_range', { count: 0 }),
        '0 date or time values were outside the years 1 to 9999, and were read as missing.',
      ],
      [
        finding('sav.very_long_string_ignored', { count: 1 }),
        '1 long text variable declares a length the file cannot hold. Gaply reads it at the length of its own record, at most 255 bytes, so longer text in it may be cut.',
      ],
      [
        finding('sav.very_long_string_ignored', { count: 2 }),
        '2 long text variables declare a length the file cannot hold. Gaply reads each at the length of its own record, at most 255 bytes, so longer text in them may be cut.',
      ],
      [
        finding('sav.unrecognised_finding', { count: 1 }),
        'The SPSS reader noted 1 other thing about this file that Gaply cannot describe yet.',
      ],
      [
        finding('sav.unrecognised_finding', { count: 2 }),
        'The SPSS reader noted 2 other things about this file that Gaply cannot describe yet.',
      ],
    ];
    for (const [f, want] of cases) expect(savFindingSentence(f)).toBe(want);
    // A code this screen does not know is said as the one thing noted.
    expect(savFindingSentence(finding('sav.a_later_code', {}))).toBe(
      'The SPSS reader noted 1 other thing about this file that Gaply cannot describe yet.',
    );
  });

  it('shows the sentences under the profile of a method, and nothing when there are none', () => {
    render(<ProfileTable prefix="cr" view={view([finding('sav.header_rows_mismatch', { declared: 0, read: 20 })])} />);
    const list = screen.getByTestId('cr-file-findings');
    expect(list.textContent).toBe("The file's header says it holds 0 cases, but 20 were read. Gaply uses the 20 it read.");
    expect(screen.getByTestId('cr-profile')).toBeTruthy();
    cleanup();
    render(<ProfileTable prefix="cr" view={view()} />);
    expect(screen.queryByTestId('cr-file-findings')).toBeNull();
    cleanup();
    render(<ProfileTable prefix="cr" view={view([])} />);
    expect(screen.queryByTestId('cr-file-findings')).toBeNull();
  });

  it("shows them under the descriptive summary's own profile", async () => {
    const m = makeMockDescribeBridge({
      stats_desc_open: view([
        finding('sav.case_size_missing', { used: 2 }),
        finding('sav.dates_out_of_range', { count: 1 }),
      ]),
      stats_desc_close: true,
    });
    render(<DescribeFlow bridge={m} pickPath={() => Promise.resolve('/x/zero.sav')} pollMs={1} />);
    fireEvent.click(screen.getByTestId('desc-choose'));
    await screen.findByTestId('desc-profile');
    const items = Array.from(screen.getByTestId('desc-file-findings').querySelectorAll('li')).map((li) => li.textContent);
    expect(items).toEqual([
      "The file's header does not give its values per case. Gaply reads the 2 its variables define.",
      '1 date or time value was outside the years 1 to 9999, and was read as missing.',
    ]);
  });
});
