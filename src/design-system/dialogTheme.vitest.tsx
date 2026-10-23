// The dark theme reaches native controls and the dialogs (Research OS, Phase
// 5B-4a; the hand test's findings c and d; decision 161).
//
// * Both dialogs portal into document.body, outside every .gds-root, while
//   every .gds-btn rule is scoped under .gds-root: their buttons drew as plain
//   words. The panel now carries gds-root itself; the overlay is unchanged.
// * .gds-root set white text and no color-scheme, so native drop-downs drew
//   white on a light box, and radios and checkboxes hid their checked state.
//
// jsdom cannot draw a native control, so the CSS lines are pinned as text and
// the colours are checked in the real app (the 5B-4a checklist).
import React from 'react';
import fs from 'node:fs';
import path from 'node:path';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { Modal } from './Modal';
import { Button } from './primitives';
import { ProjectDialog } from '../screens/statistics/projects/ProjectDialog';

afterEach(cleanup);

const read = (rel: string) => fs.readFileSync(path.resolve(__dirname, rel), 'utf8');

/** The body of the first rule whose selector list is exactly `selector`. */
function ruleBody(css: string, selector: string): string {
  const at = css.indexOf(`${selector} {`);
  if (at < 0) return '';
  const open = css.indexOf('{', at);
  return css.slice(open + 1, css.indexOf('}', open));
}

describe('the dialogs carry the design system', () => {
  it("a button inside an open Modal, the Modal's own Close included, is inside a .gds-root", () => {
    render(
      <Modal open title="HTDIALOG_modal" onClose={() => {}}>
        <Button data-testid="modal-child-button">Continue</Button>
      </Modal>,
    );
    const child = screen.getByTestId('modal-child-button');
    expect(child.classList.contains('gds-btn')).toBe(true);
    expect(child.closest('.gds-root')).not.toBeNull();
    const close = screen.getByRole('button', { name: 'Close' });
    expect(close.classList.contains('gds-btn')).toBe(true);
    expect(close.closest('.gds-root')).not.toBeNull();
  });

  it('a button inside an open ProjectDialog is inside a .gds-root', () => {
    render(
      <ProjectDialog title="HTDIALOG_project" onClose={() => {}} testId="ht-project-dialog">
        <Button data-testid="project-child-button">Save</Button>
      </ProjectDialog>,
    );
    const child = screen.getByTestId('project-child-button');
    expect(child.classList.contains('gds-btn')).toBe(true);
    expect(child.closest('.gds-root')).not.toBeNull();
  });

  it('the scope is the panel, never the overlay behind it', () => {
    render(
      <>
        <Modal open title="HTDIALOG_modal" onClose={() => {}}>
          <span>one</span>
        </Modal>
        <ProjectDialog title="HTDIALOG_project" onClose={() => {}} testId="ht-project-dialog">
          <span>two</span>
        </ProjectDialog>
      </>,
    );
    for (const overlay of [screen.getByTestId('gds-modal-overlay'), screen.getByTestId('ht-project-dialog-overlay')]) {
      expect(overlay.className).toBe('gds-modal-overlay');
      const panel = overlay.firstElementChild as HTMLElement;
      expect(panel.getAttribute('role')).toBe('dialog');
      expect(panel.classList.contains('gds-modal')).toBe(true);
      expect(panel.classList.contains('gds-root')).toBe(true);
    }
  });
});

describe('the theme reaches native controls', () => {
  const tokens = read('./tokens.css');
  const primitives = read('./primitives.css');
  const aicheck = read('../screens/checks/aicheck.css');

  it('.gds-root draws native controls in the dark scheme', () => {
    expect(ruleBody(tokens, '.gds-root')).toMatch(/^\s*color-scheme: dark;$/m);
  });

  it("radios and checkboxes under .gds-root take the theme's accent", () => {
    const body = ruleBody(tokens, ".gds-root input[type='radio'],\n.gds-root input[type='checkbox']");
    expect(body, 'the accent rule for radios and checkboxes is missing').not.toBe('');
    expect(body).toMatch(/^\s*accent-color: var\(--g-accent\);$/m);
  });

  it("AI Check's light mode draws native controls in the light scheme", () => {
    const body = ruleBody(aicheck, ".gds-aicheck[data-theme='light'],\n.gds-aicheck[data-theme='light'] .gds-root");
    expect(body, "AI Check's light block is missing").not.toBe('');
    expect(body).toMatch(/^\s*color-scheme: light;$/m);
  });

  // Finding e (Research OS, 5B-4b): preflight sets a{color:inherit;
  // text-decoration:inherit} for the whole app, so a link drew as text. One
  // class, under .gds-root only: no rule for every a.
  it('the link class draws in the accent colour and underlines on hover and focus, and no rule styles every a', () => {
    expect(ruleBody(primitives, '.gds-root .gds-link')).toMatch(/^\s*color: var\(--g-accent\);$/m);
    const hover = ruleBody(primitives, '.gds-root .gds-link:hover,\n.gds-root .gds-link:focus');
    expect(hover, 'the hover and focus rule is missing').not.toBe('');
    expect(hover).toMatch(/^\s*text-decoration: underline;$/m);
    // Comments stripped first: the rule's own comment quotes preflight's a{…}.
    const rules = primitives.replace(/\/\*[\s\S]*?\*\//g, '');
    expect(rules).toContain('.gds-root .gds-link');
    expect(rules).not.toMatch(/(^|[\s,}])a\s*[{,:[]/);
  });

  it("a panel that is also a .gds-root keeps the panel's background in any stylesheet order", () => {
    // .gds-root and .gds-modal are one class each, so the later rule would
    // win, and .gds-root's background is the page's black. The built chunks
    // put them in either order, so this two-class rule settles it.
    expect(ruleBody(primitives, '.gds-modal.gds-root')).toMatch(/^\s*background: var\(--g-bg-layer1\);$/m);
  });
});
