// Gaply — a golden sample PublishReadyReport matching compile_report()'s output
// shape. Used by the /app/report demo until a `compile_report` Tauri command is
// wired, and as the golden fixture in tests. Deliberately mixes all three
// certainty tiers, a reconsidered verdict, and an over-limit checklist item.
import { PublishReadyReport } from './reportTypes';

export const SAMPLE_REPORT: PublishReadyReport = {
  verdict: 'concern',
  combined_confidence: 1.0,
  findings: [
    {
      severity: 'critical',
      tier: 'mathematically_certain',
      certainty_label: 'mathematically certain',
      agent: 'validation_maths',
      title: 'statistical rule failed: missing effect size',
      detail:
        'A p-value is reported without an accompanying effect size (e.g. Cohen’s d). Significance does not convey magnitude.',
      confidence: 1.0,
      provenance: [
        'rule:MissingEffectSize (MAJOR)',
        'location:Results paragraph 2',
        'agent:validation_maths (deterministic)',
      ],
      section: 'Results',
    },
    {
      severity: 'major',
      tier: 'reconsidered_after_peer_review',
      certainty_label: 'reconsidered after peer review',
      agent: 'verification',
      title: 'citation c1 could not be verified (UNKNOWN)',
      detail:
        'Initially SUPPORTED; after the plagiarism agent surfaced a 0.86 similarity match to a retracted work, the verdict was reconsidered and downgraded.',
      confidence: 0.3,
      provenance: ['evidence:ev-c1-0', 'agent:verification (harness-gated, via proxy)'],
      reconsidered: { from: 'SUPPORTED', to: 'UNKNOWN' },
      section: 'References',
    },
    {
      severity: 'major',
      tier: 'ai_assessed_moderate',
      certainty_label: 'AI-assessed, moderate confidence',
      agent: 'ai_detection',
      title: 'AI Detection: concern',
      detail:
        'Discussion section leans AI-like (low burstiness). Statistical signal only — review recommended.',
      confidence: 0.6,
      provenance: ['swarm:round-table (1 round(s), rescaled weight 0.360)'],
      section: 'Discussion',
    },
    {
      severity: 'minor',
      tier: 'ai_assessed_moderate',
      certainty_label: 'AI-assessed, moderate confidence',
      agent: 'plagiarism',
      title: 'Plagiarism: concern',
      detail: '1 corpus match at 0.86 similarity (per-session isolated store).',
      confidence: 0.86,
      provenance: ['swarm:round-table (1 round(s), rescaled weight 0.688)'],
      section: 'Introduction',
    },
    {
      severity: 'info',
      tier: 'ai_assessed_moderate',
      certainty_label: 'AI-assessed, moderate confidence',
      agent: 'extraction',
      title: 'Extraction: pass',
      detail: 'Parsed 42 sections, 318 citations, 27 statistical claims without parse anomalies.',
      confidence: 0.9,
      provenance: ['swarm:round-table (1 round(s), rescaled weight 0.810)'],
      section: 'Abstract',
    },
  ],
  checklist: [
    { requirement: 'required section: Abstract', passed: true, detail: 'Abstract found', guideline_source: null },
    { requirement: 'required section: Methods', passed: true, detail: 'Methods found', guideline_source: null },
    { requirement: 'required section: Results', passed: true, detail: 'Results found', guideline_source: null },
    { requirement: 'required section: References', passed: true, detail: 'References found', guideline_source: null },
    {
      requirement: 'word limit (3000 words)',
      passed: false,
      detail: 'manuscript has 4120 words (limit 3000)',
      guideline_source: 'https://rest.example/authors',
    },
    {
      requirement: 'conflict-of-interest declaration',
      passed: true,
      detail: 'conflict-of-interest statement found',
      guideline_source: 'https://rest.example/authors',
    },
  ],
  debate: {
    rounds_run: 2,
    converged: true,
    overridden_by_constraint: true,
    rejected_agents: [],
    revised_agents: ['verification'],
  },
  disclaimer:
    'Certainty tiers: ‘mathematically certain’ findings are deterministic rule verdicts and require correction; ‘AI-assessed, moderate confidence’ findings are statistical or model-derived signals — indicators for human review, never definitive proof; ‘reconsidered after peer review’ findings were revised by the verification agent after seeing other agents’ evidence and remain non-definitive.',
};

/** The sample manuscript body, keyed by section — for the center panel's
 *  inline-highlight demo (real reports anchor to the extraction sections). */
export const SAMPLE_MANUSCRIPT_SECTIONS: { section: string; text: string }[] = [
  { section: 'Abstract', text: 'A randomized trial (n = 96) found improved recall after extended sleep.' },
  { section: 'Introduction', text: 'Sleep supports memory consolidation, as prior work has shown.' },
  { section: 'Methods', text: 'We used a paired t-test comparing pre- and post-intervention recall.' },
  { section: 'Results', text: 'Recall improved significantly (p < 0.01) in the extended-sleep group.' },
  { section: 'Discussion', text: 'These findings suggest a robust causal role for sleep in memory.' },
  { section: 'References', text: 'Doe, J. (2022). Sleep and memory. Journal of Rest, 1(1), 1-9.' },
];
