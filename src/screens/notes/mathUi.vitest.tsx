// The formula dialog — what a user can actually do, and what it refuses.
//
// The preview renders through `renderTex`, the same function the document uses,
// so these also pin that the dialog cannot show something the page will not.
import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import MathInput from './MathInput';
import { renderTex, texError } from './MathView';
import RichBody from './RichBody';

afterEach(cleanup);

describe('KaTeX rendering is deterministic and offline', () => {
  it('same TeX in, same HTML out — no model, no network', () => {
    const a = renderTex('\\frac{1}{x}', false);
    const b = renderTex('\\frac{1}{x}', false);
    expect(a.html).toBe(b.html);
    expect(a.html).toContain('katex');
    expect(a.error).toBeNull();
  });

  it('display mode differs from inline', () => {
    expect(renderTex('\\sum_i x_i', true).html).not.toBe(renderTex('\\sum_i x_i', false).html);
  });

  it('the declared subset renders: fractions, integrals, matrices, cases, aligned', () => {
    for (const tex of [
      '\\frac{a}{b}',
      '\\int_0^1 x^2\\,dx',
      '\\sum_{i=1}^{n}\\alpha_i',
      '\\sqrt[3]{x}',
      '\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}',
      '\\begin{bmatrix} 1 \\\\ 2 \\end{bmatrix}',
      '\\begin{cases} x & x>0 \\\\ -x & x\\le 0 \\end{cases}',
      '\\begin{aligned} a &= b \\\\ c &= d \\end{aligned}',
      '\\hat{\\beta} = (X^{\\top}X)^{-1}X^{\\top}y',
      '\\text{rate}_{\\max} \\approx 3.5\\%',
    ]) {
      expect(texError(tex), `should render: ${tex}`).toBeNull();
    }
  });

  // MEASURED, not assumed. The boundary is display-mode dependent, and getting
  // this wrong would have told authors their valid formulas were broken.
  it('display-only environments are accepted in DISPLAY mode and rejected inline', () => {
    for (const tex of ['\\begin{equation} a=b \\end{equation}', '\\begin{align} a&=b \\\\ c&=d \\end{align}', '\\tag{3} a=b']) {
      expect(texError(tex, true), `display: ${tex}`).toBeNull();
      expect(texError(tex, false), `inline: ${tex}`).not.toBeNull();
    }
  });

  it('what is genuinely OUT of the subset fails honestly rather than silently', () => {
    // cross-references need a TeX engine and a second pass — we ship neither
    expect(texError('\\ref{eq:1}', true)).not.toBeNull();
    // mhchem chemistry — the KaTeX extension is not bundled
    expect(texError('\\ce{H2O}', true)).not.toBeNull();
  });

  it('macros ARE supported (KaTeX has them) — recorded so the subset list is true', () => {
    expect(texError('\\newcommand{\\x}{y}\\x', false)).toBeNull();
  });
});

describe('the insert dialog', () => {
  const setup = (props: Partial<React.ComponentProps<typeof MathInput>> = {}) => {
    const onInsert = vi.fn();
    const onClose = vi.fn();
    render(<MathInput onInsert={onInsert} onClose={onClose} {...props} />);
    return { onInsert, onClose };
  };

  it('opens empty, with Insert disabled until there is something to insert', () => {
    setup();
    expect((screen.getByTestId('math-insert') as HTMLButtonElement).disabled).toBe(true);
  });

  it('typing TeX enables Insert and previews it', () => {
    const { onInsert } = setup();
    fireEvent.change(screen.getByTestId('math-tex'), { target: { value: '\\frac{1}{x}' } });
    expect(screen.getByTestId('math-preview').innerHTML).toContain('katex');
    expect((screen.getByTestId('math-insert') as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByTestId('math-insert'));
    expect(onInsert).toHaveBeenCalledWith('\\frac{1}{x}', false);
  });

  it('the display toggle is carried through to the insert', () => {
    const { onInsert } = setup();
    fireEvent.change(screen.getByTestId('math-tex'), { target: { value: 'a+b' } });
    fireEvent.click(screen.getByTestId('math-display-toggle'));
    fireEvent.click(screen.getByTestId('math-insert'));
    expect(onInsert).toHaveBeenCalledWith('a+b', true);
  });

  it('BROKEN TeX blocks the insert and says why', () => {
    const { onInsert } = setup();
    fireEvent.change(screen.getByTestId('math-tex'), { target: { value: '\\frac{1}{' } });
    expect(screen.getByTestId('math-error')).toBeTruthy();
    expect((screen.getByTestId('math-insert') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('math-insert'));
    expect(onInsert).not.toHaveBeenCalled();
  });

  it('TeX containing "]]" is refused at the door, with the reason', () => {
    const { onInsert } = setup();
    fireEvent.change(screen.getByTestId('math-tex'), { target: { value: 'a]]b' } });
    expect(screen.getByTestId('math-error').textContent).toMatch(/can’t contain/);
    expect((screen.getByTestId('math-insert') as HTMLButtonElement).disabled).toBe(true);
    expect(onInsert).not.toHaveBeenCalled();
  });

  it('editing prefills the existing formula and its display mode', () => {
    const { onInsert } = setup({ initialTex: '\\alpha', initialDisplay: true });
    expect((screen.getByTestId('math-tex') as HTMLTextAreaElement).value).toBe('\\alpha');
    expect((screen.getByTestId('math-display-toggle') as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId('math-insert').textContent).toBe('Update');
    fireEvent.click(screen.getByTestId('math-insert'));
    expect(onInsert).toHaveBeenCalledWith('\\alpha', true);
  });

  it('an example button fills the field with something that renders', () => {
    setup();
    fireEvent.click(screen.getByTestId('math-example-matrix'));
    expect((screen.getByTestId('math-tex') as HTMLTextAreaElement).value).toContain('pmatrix');
    expect(screen.queryByTestId('math-error')).toBeNull();
  });

  it('Escape closes without inserting', () => {
    const { onInsert, onClose } = setup();
    fireEvent.keyDown(screen.getByTestId('math-input'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
    expect(onInsert).not.toHaveBeenCalled();
  });
});

describe('the writing surface', () => {
  it('offers the formula control on the manuscript surface only', () => {
    const { unmount } = render(<RichBody value="" onChange={vi.fn()} withCitations testid="ms" />);
    expect(screen.queryByTestId('rb-insert-math')).toBeTruthy();
    unmount();

    render(<RichBody value="" onChange={vi.fn()} testid="proj" />);
    expect(screen.queryByTestId('rb-insert-math')).toBeNull(); // project notes stay plain
  });

  it('the toolbar button opens the dialog, and inserting writes a math node', () => {
    const onChange = vi.fn();
    render(<RichBody value="" onChange={onChange} withCitations testid="ms" />);
    fireEvent.mouseDown(screen.getByTestId('rb-insert-math'));
    expect(screen.getByTestId('math-input')).toBeTruthy();

    fireEvent.change(screen.getByTestId('math-tex'), { target: { value: '\\alpha' } });
    fireEvent.click(screen.getByTestId('math-insert'));

    expect(onChange).toHaveBeenCalled();
    const last = onChange.mock.calls[onChange.mock.calls.length - 1][0] as string;
    expect(last).toContain('[[math:\\alpha]]');
    expect(screen.queryByTestId('math-input')).toBeNull();
  });

  it('an existing formula in the body renders as a node, not as its token', async () => {
    // NB: a JS expression, not a JSX attribute literal — JSX attributes do not
    // process backslash escapes, so value="…\\frac…" would be two backslashes.
    render(<RichBody value={'Given [[math:\\frac{1}{x}]] we proceed.'} onChange={vi.fn()} withCitations testid="ms" />);
    const node = await screen.findByTestId('math-inline');
    expect(node.getAttribute('data-tex')).toBe('\\frac{1}{x}');
    expect(node.innerHTML).toContain('katex');
    // the raw token must not be visible anywhere in the surface
    expect(screen.getByTestId('ms').textContent).not.toContain('[[math:');
  });

  it('display math renders as a block node', async () => {
    render(<RichBody value={'[[math-block:\\int_0^1 x\\,dx]]'} onChange={vi.fn()} withCitations testid="ms" />);
    expect((await screen.findByTestId('math-block')).getAttribute('data-tex')).toBe('\\int_0^1 x\\,dx');
  });
});
