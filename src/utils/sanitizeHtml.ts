import DOMPurify from 'dompurify';

/**
 * Shared sanitizer for any HTML that originates from the backend or an AI
 * model before it is passed to dangerouslySetInnerHTML.
 *
 * HTML profile only (no SVG/MathML vectors), and form/style elements are
 * stripped so injected markup can't phish or restyle the page. Event-handler
 * attributes (onerror, onclick, ...) and javascript: URLs are removed by
 * DOMPurify's defaults.
 */
const SANITIZE_CONFIG = {
  USE_PROFILES: { html: true },
  FORBID_TAGS: ['style', 'form', 'input', 'textarea', 'select', 'button', 'iframe'],
  FORBID_ATTR: ['style'],
};

export function sanitizeHtml(dirty: string): string {
  return DOMPurify.sanitize(dirty ?? '', SANITIZE_CONFIG);
}

export default sanitizeHtml;
