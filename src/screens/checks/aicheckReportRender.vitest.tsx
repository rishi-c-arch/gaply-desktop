// Gaply — the on-screen AI Check report EQUIVALENCE PIN.
//
// The honesty strings were extracted from AiCheckReport.tsx into the shared
// aicheckReportModel (so the HTML export can't drift from the screen). That
// refactor touched the WORKING on-screen report, so this locks its observable
// output: a full-DOM snapshot over both fixtures. It was seeded from output
// PROVEN byte-identical to the pre-refactor (HEAD) report (a one-time render-both
// -and-diff check over the same fixtures), so the baseline is the pre-refactor
// behavior, not the post-refactor behavior rubber-stamped. Any later edit to the
// model or the report that changes what a reader SEES breaks this snapshot — the
// intended tripwire.
import React from 'react';
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import AiCheckReport from './AiCheckReport';
import { AICHECK_FIXTURE, AICHECK_FIXTURE_SPANISH } from './aicheckFixture';

afterEach(cleanup);

describe('AiCheckReport — observable-output equivalence pin', () => {
  it('English fixture renders the locked DOM', () => {
    const { container } = render(<AiCheckReport result={AICHECK_FIXTURE} />);
    expect(container.innerHTML).toMatchSnapshot();
  });

  it('Spanish (language-downgrade) fixture renders the locked DOM', () => {
    const { container } = render(<AiCheckReport result={AICHECK_FIXTURE_SPANISH} />);
    expect(container.innerHTML).toMatchSnapshot();
  });
});
