// Help (Research OS, Phase 8P, decision 453): the home's content reused, never
// copied, and two fixed texts, in ProjectDialog, which keeps the focus inside
// and returns it.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { HOME_TEXT } from '../capabilityWording';
import { limitsSentence } from '../flowParts';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import type { MethodLimitsView } from '../describeTypes';
import type { CapabilityView, Lifecycle } from '../ttestTypes';
import { HelpDialog } from './HelpDialog';

const flags = vi.hoisted(() => ({ statsCheck: true }));
vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'statsCheck' && flags.statsCheck }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const cap = (id: string, lifecycle: Lifecycle, runnable: boolean, knownLimitations: string[] = []): CapabilityView => ({
  id,
  version: 1,
  lifecycle,
  titleCode: 'stats.capability.describe.title',
  descriptionCode: 'stats.capability.describe.description',
  licenseNotes: [],
  knownLimitations,
  runnable,
});
const LIST: CapabilityView[] = [
  cap('stats.check', 'stable', true),
  cap('stats.describe', 'beta', true, ['stats.capability.limit.table.text_files_utf8_only']),
  cap('stats.verify.pearson', 'stable', true),
  cap('stats.analyze.future', 'planned', false),
  cap('stats.analyze.old', 'deprecated', false),
];

const noop = () => undefined;

describe('Methods and limits', () => {
  it("draws the home's places: limitations, the roadmap and the retired list, from Rust's list", () => {
    render(<HelpDialog topic="methods" capabilities={LIST} unread={false} limits={limits8 as MethodLimitsView} onClose={noop} />);
    const d = screen.getByTestId('sw-help-methods');
    expect(within(d).getByTestId('stats-limits-stats.describe').textContent).toBe('A CSV or TSV file must be UTF-8: a file in another encoding is refused.');
    expect(within(d).getByTestId('stats-roadmap-stats.analyze.future')).toBeTruthy();
    expect(within(d).getByTestId('stats-retired-stats.analyze.old')).toBeTruthy();
    expect(within(d).getByTestId('sw-help-method-stats.describe')).toBeTruthy();
    expect(within(d).queryByTestId('sw-help-method-stats.analyze.future')).toBeNull();
  });

  it('gives the limits sentence, from stats_desc_limits, and none without an answer', () => {
    render(<HelpDialog topic="methods" capabilities={LIST} unread={false} limits={limits8 as MethodLimitsView} onClose={noop} />);
    expect(screen.getByTestId('sw-limits').textContent).toBe(limitsSentence(limits8 as MethodLimitsView));
    cleanup();
    render(<HelpDialog topic="methods" capabilities={LIST} unread={false} limits={null} onClose={noop} />);
    expect(screen.queryByTestId('sw-limits')).toBeNull();
  });

  it('follows the Stats Check flag, as the home does', () => {
    flags.statsCheck = false;
    render(<HelpDialog topic="methods" capabilities={LIST} unread={false} limits={null} onClose={noop} />);
    expect(screen.queryByTestId('sw-help-method-stats.check')).toBeNull();
    cleanup();
    flags.statsCheck = true;
    render(<HelpDialog topic="methods" capabilities={LIST} unread={false} limits={null} onClose={noop} />);
    expect(screen.getByTestId('sw-help-method-stats.check')).toBeTruthy();
  });

  it('names the verifier\'s methods once, as the home does, and links nowhere', () => {
    render(<HelpDialog topic="methods" capabilities={LIST} unread={false} limits={null} onClose={noop} />);
    expect(within(screen.getByTestId('sw-help-verifier')).getByRole('list').querySelectorAll('li')).toHaveLength(1);
    expect(screen.getByTestId('sw-help-methods').querySelectorAll('a')).toHaveLength(0);
  });

  it("says the home's sentence when the list could not be read", () => {
    render(<HelpDialog topic="methods" capabilities={null} unread limits={null} onClose={noop} />);
    expect(screen.getByRole('alert').textContent).toBe(HOME_TEXT.unread);
  });

  it('imports the home\'s pieces rather than holding a copy', () => {
    const src = fs.readFileSync(path.join(__dirname, 'HelpDialog.tsx'), 'utf8');
    expect(src).toMatch(/import \{[^}]*\bLimitations\b[^}]*\bplace\b[^}]*\bRetiredList\b[^}]*\bRoadmapList\b[^}]*\} from '\.\.\/StatisticsHome'/);
    expect(src).toMatch(/import \{ ProjectDialog \} from '\.\.\/\.\.\/\.\.\/design-system\/ProjectDialog'/);
    expect(src).not.toMatch(/stats-roadmap-|stats-retired-|stats-limits-/);
  });
});

describe('How Gaply reads a file', () => {
  it('is the fixed text, word for word', () => {
    render(<HelpDialog topic="file" capabilities={null} unread={false} limits={null} onClose={noop} />);
    expect(screen.getByTestId('sw-help-file').textContent).toBe(
      "Gaply reads your file once and never changes it. The Data View shows it as Gaply read it. Numbers are shown in full, so 1.50 shows as 1.5. A cell your file leaves empty or marks as missing, and an SPSS user-missing value, is shown empty. SPSS's value labels are not shown. Text is shown up to 512 characters a cell. A scale score is computed from your columns and kept as a recipe; your file is not changed."
    );
  });
});

describe('Keyboard', () => {
  it("names the menu's keys and the grid's", () => {
    render(<HelpDialog topic="keyboard" capabilities={null} unread={false} limits={null} onClose={noop} />);
    const t = screen.getByTestId('sw-help-keyboard').textContent as string;
    for (const k of ['Tab', 'Left', 'Right', 'Up', 'Down', 'Enter', 'Escape', 'letter']) expect(t, k).toContain(k);
    for (const k of ['grid', 'arrow keys', 'Page Up', 'Page Down', 'Home and End', 'Cmd or Ctrl']) expect(t, k).toContain(k);
  });
});

describe('the dialog', () => {
  it.each([['methods'], ['file'], ['keyboard']] as const)('%s: is a dialog with a title and a Close, and Escape closes it', (topic) => {
    const onClose = vi.fn();
    render(<HelpDialog topic={topic} capabilities={LIST} unread={false} limits={null} onClose={onClose} />);
    const d = screen.getByRole('dialog');
    expect(d.getAttribute('aria-labelledby')).toBeTruthy();
    fireEvent.click(within(d).getByTestId('sw-help-close'));
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(d, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
    expect(d.contains(document.activeElement)).toBe(true);
  });
});
