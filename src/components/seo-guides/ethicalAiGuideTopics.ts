/** Footer + page topic links; each opens the same embedded presentation with ?topic=slug */
export interface EthicalAiGuideTopic {
  slug: string;
  /** Exact phrase for H2 / SEO */
  label: string;
  /** Short ethical framing paragraph (visible under each H2). */
  intro: string;
}

export const ETHICAL_AI_GUIDE_TOPICS: EthicalAiGuideTopic[] = [
  {
    slug: 'ai-detection-2026',
    label: 'How to remove AI detection from research paper 2026',
    intro:
      'The slide deck explains why “removing” detection is the wrong frame: focus on transparent use, your own analysis, and institutional rules—not evasion.',
  },
  {
    slug: 'rewriter-bypass-turnitin',
    label: 'Free AI rewriter for academic papers to bypass Turnitin',
    intro:
      'These slides cover why rewriter and “bypass” approaches conflict with academic integrity and what to do instead (policy, disclosure, your own drafting).',
  },
  {
    slug: 'humanizer-tools',
    label: 'Best humanizer tools for research writing',
    intro:
      'The presentation separates myth from reality on “humanizer” claims and points to ethical alternatives: feedback on your own writing and honest disclosure.',
  },
  {
    slug: 'chatgpt-plagiarism-india',
    label: 'Does ChatGPT text pass university plagiarism checks in India?',
    intro:
      'Includes a regional section on Turnitin-style checks and Indian universities—accuracy limits, false positives, and the ethics-over-evasion message.',
  },
  {
    slug: 'literature-review-flagged',
    label: 'How to use AI for literature review without being flagged',
    intro:
      'Slides on using AI to discover and organize sources while you read, verify citations, and synthesize in your own words—so integrity stays intact.',
  },
];

/** Served from public/ after you add the file locally (Finder or: npm run copy-ethical-pptx). */
export const ETHICAL_AI_PPTX_PATH = '/guides/ethical-researcher-guide-ai-2026.pptx';

/** Export the same deck from PowerPoint as PDF for smooth in-page slides (npm run copy-ethical-pdf). */
export const ETHICAL_AI_PDF_PATH = '/guides/ethical-researcher-guide-ai-2026.pdf';

