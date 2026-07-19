import React from 'react';
import { render } from '@testing-library/react';
import MarkdownRenderer from './MarkdownRenderer';

describe('MarkdownRenderer XSS protection', () => {
  it('strips script tags embedded in an AI response before rendering', () => {
    const maliciousAiResponse =
      'Your analysis is ready. <script>window.stolen = localStorage.getItem("authToken")</script> **Summary** below.';
    const { container } = render(<MarkdownRenderer content={maliciousAiResponse} />);

    expect(container.querySelector('script')).toBeNull();
    expect(container.innerHTML).not.toContain('localStorage');
    expect(container.textContent).toContain('Your analysis is ready.');
    // Legitimate markdown still renders
    expect(container.querySelector('strong')?.textContent).toBe('Summary');
  });

  it('strips onerror-based image payloads', () => {
    const maliciousAiResponse = 'Result: <img src="x" onerror="document.title=\'pwned\'"> done';
    const { container } = render(<MarkdownRenderer content={maliciousAiResponse} />);

    const img = container.querySelector('img');
    expect(img?.getAttribute('onerror') ?? null).toBeNull();
    expect(container.innerHTML).not.toContain('onerror');
    expect(document.title).not.toBe('pwned');
  });

  it('strips javascript: links injected via backend content', () => {
    const { container } = render(
      <MarkdownRenderer content={'<a href="javascript:alert(1)">Download report</a>'} />
    );
    const link = container.querySelector('a');
    expect(link?.getAttribute('href') ?? '').not.toContain('javascript:');
  });
});

describe('MarkdownRenderer — note-export constructs (blockquote, hr, boundary italic)', () => {
  it('renders `> quote` as a <blockquote>, marker consumed', () => {
    const { container } = render(<MarkdownRenderer content={'> a structure (p. 737)'} />);
    const bq = container.querySelector('blockquote');
    expect(bq).not.toBeNull();
    expect(bq?.textContent).toContain('a structure (p. 737)');
    expect(container.textContent).not.toContain('> '); // no raw marker
  });

  it('renders a standalone `---` as an <hr>', () => {
    const { container } = render(<MarkdownRenderer content={'A\n\n---\n\nB'} />);
    expect(container.querySelector('hr')).not.toBeNull();
    expect(container.textContent).not.toContain('---');
  });

  it('boundary underscores italicize: `_Tags: writing_` → <em>', () => {
    const { container } = render(<MarkdownRenderer content={'_Tags: writing, intro_'} />);
    expect(container.querySelector('em')?.textContent).toBe('Tags: writing, intro');
    expect(container.textContent).not.toContain('_'); // underscores consumed
  });

  it('INTRAWORD underscores are NEVER italicized — snake_case is protected', () => {
    const ids = 'verify_provenance p_value t_test chi_square is_retracted';
    const { container } = render(<MarkdownRenderer content={ids} />);
    expect(container.querySelector('em')).toBeNull(); // nothing italicized
    // every identifier survives verbatim, underscores intact
    for (const id of ['verify_provenance', 'p_value', 't_test', 'chi_square', 'is_retracted']) {
      expect(container.textContent).toContain(id);
    }
  });

  it('mixed: an identifier stays literal while boundary emphasis still works', () => {
    const { container } = render(<MarkdownRenderer content={'the verify_provenance field _matters_ here'} />);
    expect(container.textContent).toContain('verify_provenance'); // literal
    expect(container.querySelector('em')?.textContent).toBe('matters'); // emphasized
  });

  // Exact innerHTML pins — boundaries provably correct, not just "an <em> exists".
  // These catch the failure mode where two italics bleed or swallow the middle.
  it('multiple boundary italics on one line → SEPARATE <em>, no bleed', () => {
    const { container } = render(<MarkdownRenderer content={'text _one_ and _two_ here'} />);
    expect(container.querySelector('.md-paragraph span')?.innerHTML)
      .toBe('text <em>one</em> and <em>two</em> here');
  });

  it('adjacent `_a_ _b_` → two <em>, the middle space is NOT swallowed', () => {
    const { container } = render(<MarkdownRenderer content={'_a_ _b_'} />);
    expect(container.querySelector('.md-paragraph span')?.innerHTML).toBe('<em>a</em> <em>b</em>');
  });

  it('mixed line: `_note_` emphasizes, `my_var` stays literal — one pass, exact HTML', () => {
    const { container } = render(<MarkdownRenderer content={'see _note_ below and my_var too'} />);
    expect(container.querySelector('.md-paragraph span')?.innerHTML)
      .toBe('see <em>note</em> below and my_var too');
  });

  it('REGRESSION: headings, bold, and *asterisk* italic still render', () => {
    const { container } = render(<MarkdownRenderer content={'# H1\n## H2\n**bold** and *emph*'} />);
    expect(container.querySelector('.md-h1')?.textContent).toBe('H1');
    expect(container.querySelector('.md-h2')?.textContent).toBe('H2');
    expect(container.querySelector('strong')?.textContent).toBe('bold');
    expect(Array.from(container.querySelectorAll('em')).some((e) => e.textContent === 'emph')).toBe(true);
  });
});
