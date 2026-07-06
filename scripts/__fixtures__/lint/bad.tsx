// FIXTURE (must FAIL the security lint): dangerouslySetInnerHTML with raw HTML,
// bypassing sanitizeHtml(). Lives outside src/ so the real gate never scans it.
import React from 'react';

export function Bad({ html }: { html: string }) {
  // XSS sink: untrusted HTML injected without sanitizeHtml()
  return <div dangerouslySetInnerHTML={{ __html: html }} />;
}
