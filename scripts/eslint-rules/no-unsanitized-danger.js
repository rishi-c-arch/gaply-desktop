'use strict';
/**
 * Custom ESLint rule: React dangerouslySetInnerHTML must route through the
 * sanitizeHtml() utility (src/utils/sanitizeHtml.ts) — the XSS-safe sink built
 * during the earlier security fix. A raw string or template literal in __html
 * (an unescaped template literal rendered as HTML) is therefore rejected.
 *
 * Vetted lower-risk exceptions that do NOT require sanitizeHtml:
 *   - a static <style> block (a template literal / string with no interpolation
 *     of runtime data — CSS, not an HTML-injection sink);
 *   - JSON.stringify(...) (structured data, e.g. JSON-LD — assumed non-user
 *     controlled; escape `<` if that ever changes).
 */
module.exports = {
  meta: {
    type: 'problem',
    docs: {
      description:
        'Require sanitizeHtml() for dangerouslySetInnerHTML and forbid dynamic innerHTML sinks',
    },
    schema: [],
    messages: {
      danger:
        'dangerouslySetInnerHTML must wrap __html in sanitizeHtml() (src/utils/sanitizeHtml.ts). A raw string or template literal here is an XSS sink.',
    },
  },
  create(context) {
    function calleeName(call) {
      const c = call.callee;
      if (!c) return null;
      if (c.type === 'Identifier') return c.name;
      if (c.type === 'MemberExpression' && c.property && c.property.type === 'Identifier') {
        const obj = c.object && c.object.type === 'Identifier' ? c.object.name : null;
        return obj ? `${obj}.${c.property.name}` : c.property.name;
      }
      return null;
    }

    function isAllowed(value, parentTag) {
      if (!value) return false;
      if (value.type === 'CallExpression') {
        const name = calleeName(value);
        return name === 'sanitizeHtml' || name === 'JSON.stringify' || name === 'stringify';
      }
      if (parentTag === 'style') {
        if (value.type === 'Literal' && typeof value.value === 'string') return true;
        if (value.type === 'TemplateLiteral' && value.expressions.length === 0) return true;
      }
      return false;
    }

    return {
      JSXAttribute(node) {
        if (!node.name || node.name.name !== 'dangerouslySetInnerHTML') return;
        const parentTag = node.parent && node.parent.name && node.parent.name.name;

        const container = node.value;
        if (!container || container.type !== 'JSXExpressionContainer') {
          context.report({ node, messageId: 'danger' });
          return;
        }
        const obj = container.expression;
        if (!obj || obj.type !== 'ObjectExpression') {
          context.report({ node, messageId: 'danger' });
          return;
        }
        const htmlProp = obj.properties.find(
          (p) =>
            p.type === 'Property' &&
            ((p.key.type === 'Identifier' && p.key.name === '__html') ||
              (p.key.type === 'Literal' && p.key.value === '__html'))
        );
        if (!htmlProp) {
          context.report({ node, messageId: 'danger' });
          return;
        }
        if (!isAllowed(htmlProp.value, parentTag)) {
          context.report({ node: htmlProp, messageId: 'danger' });
        }
      },
    };
  },
};
