// Gaply — Research Copilot chat bridge. Sends the structured payload to the LLM
// through the Railway proxy (cloud handles translation). Mock for tests. The
// real ChatClient is documented; wiring the proxy chat endpoint is deferred.
import { ChatProxyPayload } from './chatContext';

export interface ChatResponse {
  /** Answer text, in the user's language. */
  answer: string;
  /** Which finding/RAG source(s) the answer is grounded in. */
  provenance: string[];
  /** True when the answer concerns AI-assessed (non-definitive) items. */
  aiAssessed: boolean;
}

export interface ChatClient {
  ask(payload: ChatProxyPayload): Promise<ChatResponse>;
}

/** Production path (documented): POST the payload to the proxy /copilot, which
 *  holds the Claude key, translates, and enforces the same system policy. */
export class ProxyChatClient implements ChatClient {
  constructor(private proxyBaseUrl: string) {}
  async ask(_payload: ChatProxyPayload): Promise<ChatResponse> {
    throw new Error('ProxyChatClient: the proxy /copilot endpoint is not wired yet.');
  }
}

/** Test/dev double — records payloads (to prove no manuscript text) and returns
 *  a scripted, language-aware answer. */
export function makeMockChat(
  responder?: (payload: ChatProxyPayload) => ChatResponse
): ChatClient & { payloads: ChatProxyPayload[] } {
  const payloads: ChatProxyPayload[] = [];
  return {
    payloads,
    async ask(payload) {
      payloads.push(payload);
      if (responder) return responder(payload);
      // default: echo grounded guidance in the detected language
      return {
        answer: `[${payload.language}] Guidance grounded in ${payload.findings.length} finding(s).`,
        provenance: payload.findings[0]?.provenance ?? [],
        aiAssessed: payload.findings.some((f) => f.tier === 'ai_assessed_moderate'),
      };
    },
  };
}
