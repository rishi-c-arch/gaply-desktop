// Gaply — Research Copilot guardrails. The assistant EDUCATES and GUIDES; it is
// STRUCTURALLY INCAPABLE of writing the paper. Two layers, mirroring the backend
// harness gates:
//   1. REQUEST guard  — detect "write/rewrite/draft my X" intents and refuse
//      up front with an educational redirect (no cloud call needed).
//   2. RESPONSE guard — even if the model drifts, block outputs that look like
//      ghostwritten manuscript prose (multi-paragraph drafted content).
// The SYSTEM_PROMPT is sent to the cloud LLM to enforce the same policy there.

export const SYSTEM_PROMPT = `You are Gaply Research Copilot. You EDUCATE and GUIDE a researcher \
about their manuscript analysis. You MAY: explain why a finding matters, suggest what to fix and \
why, recommend papers or sections to study, and teach concepts. You MUST NOT write, rewrite, \
paraphrase, or draft any portion of the manuscript (no abstracts, introductions, methods, \
paragraphs, or sentences in the paper's voice). If asked to write any part, DECLINE and instead \
explain what a strong version of that section needs and point to examples to study. Ground every \
answer in the provided structured findings or RAG sources and cite them. Answer in the user's \
language. For AI-assessed items, include the uncertainty disclaimer. Never reproduce or infer the \
raw manuscript text — you only receive structured summaries.`;

/** Phrasings that request ghostwriting (any language coverage is best-effort;
 *  the system prompt is the primary defense, this is the hard client gate). */
const WRITE_INTENT =
  /\b(write|re-?write|draft|compose|generate|produce|paraphrase|reword|rephrase|polish|edit|proofread|improve the wording of|fix the grammar of)\b[^.?!]{0,40}\b(my|the|this|an?)\b[^.?!]{0,30}\b(abstract|introduction|intro|methods?|results?|discussion|conclusion|paragraph|section|sentence|manuscript|paper|thesis|essay|literature review|title)\b/i;

/** True when the user is asking the assistant to produce manuscript prose. */
export function isGhostwritingRequest(message: string): boolean {
  return WRITE_INTENT.test(message);
}

export const REFUSAL_REDIRECT = (topic: string): string =>
  `I can’t write your ${topic} for you — Gaply guides, it doesn’t ghostwrite. ` +
  `But I can teach you exactly what a strong ${topic} needs, critique yours against your findings, ` +
  `and point you to model examples to study. Want the checklist for a strong ${topic}?`;

/** Extract the section noun for a nicer refusal. */
export function refusalFor(message: string): string {
  const m = message.match(
    /\b(abstract|introduction|methods?|results?|discussion|conclusion|section|paragraph|manuscript|paper|thesis|literature review|title)\b/i
  );
  return REFUSAL_REDIRECT(m ? m[1].toLowerCase() : 'manuscript');
}

/* --------------------------- response-side guard ------------------------ */

export interface GhostwriteVerdict {
  blocked: boolean;
  reason?: string;
}

/** Detect ghostwritten manuscript prose in a model response: multi-paragraph
 *  drafted content, especially in an academic paper voice. Guidance answers are
 *  short and meta ("here's WHY / WHAT to do"); ghostwritten drafts are long,
 *  first-person-plural, and read like the paper itself. */
export function detectGhostwriting(response: string): GhostwriteVerdict {
  const paragraphs = response.split(/\n\s*\n/).map((p) => p.trim()).filter(Boolean);
  const words = (p: string) => p.split(/\s+/).length;
  const longParagraphs = paragraphs.filter((p) => words(p) >= 35);

  // academic "paper voice" markers
  const paperVoice =
    /\b(in this (study|paper|work|manuscript|article)|we (present|propose|investigate|report|demonstrate|hypothesize|conducted|recruited|found)|this (study|paper) (aims|presents|investigates|examines)|our (results|findings|analysis) (show|indicate|demonstrate|suggest))\b/i;

  // (1) substantial drafted prose — 2+ long paragraphs.
  if (longParagraphs.length >= 2) {
    return { blocked: true, reason: 'response contains multi-paragraph drafted manuscript prose' };
  }
  // (2) any non-trivial paragraph written in the paper's authorial voice.
  if (paragraphs.some((p) => paperVoice.test(p) && words(p) >= 25)) {
    return { blocked: true, reason: "response is drafted in the paper's authorial voice" };
  }
  return { blocked: false };
}

export const GHOSTWRITE_BLOCK_MESSAGE =
  'I started to draft that, but Gaply doesn’t ghostwrite manuscript text. Here’s the guidance instead: ' +
  'focus on the structure and evidence your findings point to, and I’ll critique what you write.';
