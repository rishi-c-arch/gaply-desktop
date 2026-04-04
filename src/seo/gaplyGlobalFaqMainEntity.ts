/**
 * Site-wide FAQ items for JSON-LD (single FAQPage per document).
 * Used on most routes via App; merged on the ethical AI guide page.
 */
export const GAPLY_GLOBAL_FAQ_MAIN_ENTITIES = [
  {
    '@type': 'Question',
    name: 'What are Q1 journals?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Q1 journals are the highest impact quartile in Scopus and Web of Science. They represent the top 25% of journals in a subject area by citation metrics.',
    },
  },
  {
    '@type': 'Question',
    name: 'What are Q2, Q3, Q4 journals?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Q2 journals are in the 25-50% range, Q3 in 50-75%, and Q4 in 75-100%. Gaply helps researchers find matching journals across all quartiles for research paper publications.',
    },
  },
  {
    '@type': 'Question',
    name: 'How can I improve my research paper writing?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Focus on clear research questions, measurable research objectives, and testable research hypotheses. Structure your findings clearly and use journal matching to target the right research journals for publication.',
    },
  },
  {
    '@type': 'Question',
    name: 'Where is the free citation generator on Gaply?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Use the free citation and reference tool at https://www.gaply.in/citation-generator. It supports APA, Vancouver, Harvard, BibTeX, RIS, DOI and PubMed lookup, and conversion between common academic formats. No payment is required.',
    },
  },
  {
    '@type': 'Question',
    name: 'What free research tools does Gaply offer?',
    acceptedAnswer: {
      '@type': 'Answer',
      text: 'Gaply includes free features such as journal matching, paper search, journal quartile guidance, a citation generator, and other academic utilities. See https://www.gaply.in/features for the full list.',
    },
  },
] as const;
