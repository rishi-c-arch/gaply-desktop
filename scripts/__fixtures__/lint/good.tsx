// FIXTURE (must PASS the security lint): dangerouslySetInnerHTML routed through
// the sanitizeHtml() utility.
import React from 'react';
import { sanitizeHtml } from '../../../src/utils/sanitizeHtml';

export function Good({ html }: { html: string }) {
  return <div dangerouslySetInnerHTML={{ __html: sanitizeHtml(html) }} />;
}
