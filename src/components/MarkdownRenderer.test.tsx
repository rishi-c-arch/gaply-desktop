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
