'use strict';
/**
 * Custom ESLint rule: a component prop may not be passed through a type-erasing
 * cast (§11 D109).
 *
 * # Why this exists
 *
 * `<AnnotatedManuscript items={items as any} />` shipped a bug that no test and
 * no guard could see. `items` was `AuditItem[]` — the `ai_job_results` wire
 * shape, carrying `resultJson` as a JSON STRING — and the component read
 * `result.output.verdict`, the parsed shape that a different command returns.
 * The field was always `undefined`, so no `citation_support` or `citation_need`
 * sentence could ever be highlighted; only `unverifiable`, which ignores the
 * verdict, ever drew.
 *
 * Both types were correct. The compiler was told not to look.
 *
 * That is the fifth wire-shape drift in this codebase (§11 D46, D53, D103,
 * D109) and the FIRST that a Rust-side guard could not have caught, because
 * Rust emitted the right key and the reader asked for a different one. The
 * defect lives entirely on the boundary between a component and its caller,
 * which is exactly what this rule watches.
 *
 * # What it forbids
 *
 * A JSX attribute whose value is `x as any`, `x as unknown`, or
 * `x as unknown as T` — the forms that stop the checker comparing the caller's
 * SHAPE against the component's props.
 *
 * Not `x!`. A non-null assertion erases nullability, not shape: the checker
 * still compares every field, so it cannot produce D109's bug. Including it
 * would fire on two already-narrowed branches in PlagiarismCheckPage that are
 * fully type-checked — and a rule that reports non-bugs is a rule someone
 * switches off, which is the lesson §11 D108 recorded when the obvious
 * "no underscores may cross" rule flagged 248 correct fields.
 *
 * # What it deliberately allows
 *
 *   - a cast to a REAL type (`value as Verdict`). That is a claim the checker
 *     still verifies against the target type; `any` is the absence of one.
 *   - test files. A fixture is allowed to be approximate — but note that a test
 *     which casts is testing the vocabulary, not the path, which is precisely
 *     how D109 stayed invisible. Prefer a fixture built to the real shape.
 */
module.exports = {
  meta: {
    type: 'problem',
    docs: {
      description:
        'Forbid `as any` / `as unknown` on JSX prop values — the cast that hid §11 D109',
    },
    schema: [],
    messages: {
      anyCast:
        'Prop "{{name}}" is passed through `{{form}}`, which stops TypeScript comparing what you have against what the component expects. §11 D109 shipped a broken screen this way: the caller held `resultJson` (a JSON string) and the component read `result.output`, and nothing could tell. Fix the shape, widen the prop type, or convert explicitly — do not silence the checker.',
    },
  },
  create(context) {
    /** The type-erasing forms. A cast to a real type is fine and is not here. */
    function erasure(node) {
      if (!node) return null;
      if (node.type === 'TSAsExpression' || node.type === 'TSTypeAssertion') {
        const t = node.typeAnnotation;
        if (!t) return null;
        if (t.type === 'TSAnyKeyword') return 'as any';
        if (t.type === 'TSUnknownKeyword') return 'as unknown';
        // `x as unknown as Foo` — the inner cast is the erasure, and the outer
        // one launders it. Reported on the outer node so the message points at
        // the whole expression the reader sees.
        const inner = erasure(node.expression);
        if (inner) return `${inner} as …`;
      }
      return null;
    }

    return {
      JSXAttribute(node) {
        const v = node.value;
        if (!v || v.type !== 'JSXExpressionContainer') return;
        const form = erasure(v.expression);
        if (!form) return;
        context.report({
          node: v,
          messageId: 'anyCast',
          data: {
            name: node.name && node.name.name ? String(node.name.name) : 'prop',
            form,
          },
        });
      },
    };
  },
};
