/** FAQ mainEntity entries specific to the ethical researcher guide (merged after global FAQs in one FAQPage). */
export const ETHICAL_AI_GUIDE_FAQ_MAIN_ENTITIES = [
  {
    '@type': 'Question',
    name: 'How to remove AI detection from a research paper in 2026?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Trying to hide or remove AI detection signals is the wrong goal for scholarly work. Ethical practice is to write your own analysis, use AI transparently where your institution allows it, disclose assistance, and verify every claim. Detection tools are imperfect but evasion can constitute academic dishonesty.',
    },
  },
  {
    '@type': 'Question',
    name: 'Is there a free AI rewriter for academic papers to bypass Turnitin?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Using rewriters or humanizers to bypass Turnitin or similar checks risks academic misconduct. Turnitin and university policies treat undisclosed AI-generated work as a integrity issue. Ethical alternatives include drafting your own text, using AI only for permitted feedback, and following your course policy on disclosure.',
    },
  },
  {
    '@type': 'Question',
    name: 'What are the best humanizer tools for research writing?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Tools marketed as AI humanizers or detector bypasses are unreliable and often violate academic integrity rules. Better approaches: write first, use AI as a coach for structure or clarity suggestions you apply yourself, vary your own sentence style, and disclose AI use when required.',
    },
  },
  {
    '@type': 'Question',
    name: 'Does ChatGPT text pass university plagiarism checks in India?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Many Indian universities use Turnitin and similar systems with AI detection. ChatGPT-style text can be flagged, and detectors have false positives and limits. The safer question is whether your use follows your institution policy and whether you can disclose and defend your authorship—not whether text passes a detector.',
    },
  },
  {
    '@type': 'Question',
    name: 'How to use AI for a literature review without being flagged?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Use AI to discover or organize papers, then read primary sources yourself, verify every citation in a database, synthesize in your own words, and disclose AI assistance if required. AI cannot replace critical reading; relying on summaries alone produces shallow reviews and higher risk.',
    },
  },
] as const;
