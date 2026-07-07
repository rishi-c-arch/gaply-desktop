// Gaply — Research Copilot context assembly (LOCAL) + proxy payload. The context
// is built from (a) structured findings, (b) local RAG snippets (guidelines /
// standards / "what makes a strong methods section"), and (c) citation metadata.
// The RAW MANUSCRIPT TEXT IS NEVER SENT — same structured-summary discipline as
// PublishReady's buildProxyPayload.
import { PublishReadyReport } from '../report/reportTypes';
import { SYSTEM_PROMPT } from './chatGuards';

export interface RagSnippet {
  source: string; // e.g. "Lancet Author Guidelines"
  url: string;
  text: string; // guideline/standard text (public knowledge, not the manuscript)
}

export interface CitationMeta {
  title: string;
  doi: string | null;
  retracted: boolean;
}

export interface ChatContext {
  report: PublishReadyReport;
  ragSnippets: RagSnippet[];
  citations: CitationMeta[];
}

/** The structured payload sent to the proxy for one turn. Findings, RAG source
 *  text, citation metadata, the question — never manuscript text. */
export interface ChatProxyPayload {
  task: 'research_copilot';
  system: string;
  language: string;
  question: string;
  findings: Array<{ agent: string; tier: string; severity: string; summary: string; provenance: string[] }>;
  rag: Array<{ source: string; text: string }>;
  citations: Array<{ title: string; doi: string | null; retracted: boolean }>;
}

function structuredProvenance(p: string[]): string[] {
  return p.filter((s) => /^(rule:|evidence:|swarm:|agent:|gate:|similarity:|match_type:|source:|signal:)/.test(s));
}

export function buildChatPayload(ctx: ChatContext, question: string, language: string): ChatProxyPayload {
  return {
    task: 'research_copilot',
    system: SYSTEM_PROMPT,
    language,
    question,
    findings: ctx.report.findings.map((f) => ({
      agent: f.agent,
      tier: f.tier,
      severity: f.severity,
      summary: f.title, // structured summary, never an excerpt (detail is omitted)
      provenance: structuredProvenance(f.provenance),
    })),
    rag: ctx.ragSnippets.map((s) => ({ source: s.source, text: s.text })),
    citations: ctx.citations.map((c) => ({ title: c.title, doi: c.doi, retracted: c.retracted })),
  };
}
