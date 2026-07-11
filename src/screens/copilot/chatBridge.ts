// Gaply — Research Copilot chat bridge. The production path goes through the
// Rust backend (`run_copilot_chat`): the code-enforced integrity firewall +
// llm_safe discipline live there, and the proxy hop is signed with the App
// Check key from the OS keychain — no secrets in the frontend. Mock for tests.
import { isTauri } from '../../utils/isTauri';
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

/** Backend `run_copilot_chat` turn shape (snake_case, as serialized by Rust).
 *  `kind` distinguishes answered / refused_ghostwriting / blocked_ghostwriting
 *  / unavailable; `answer` always carries the honest text to show. */
export interface RustChatTurn {
  kind: 'answered' | 'refused_ghostwriting' | 'blocked_ghostwriting' | 'unavailable';
  answer: string;
  provenance: string[];
  ai_assessed: boolean;
  warnings: string[];
  available: boolean;
}

/** Production client: hand the turn to the Rust backend, which runs the
 *  code-enforced firewall (pre-filter → cloud proxy when live → post-filter)
 *  and degrades honestly when the cloud is unreachable. Desktop-app only. */
export class TauriChatClient implements ChatClient {
  /** `getUserToken` (Set 8): supplies the signed-in user's JWT per ask, so the
   *  proxy can run the REAL server-side entitlement gate on the paid chat. */
  constructor(private getUserToken?: () => string | undefined) {}

  async ask(payload: ChatProxyPayload): Promise<ChatResponse> {
    if (!isTauri) {
      throw new Error('The Research Copilot runs in the Gaply desktop app.');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    const turn = (await invoke('run_copilot_chat', {
      context: { findings: payload.findings, rag: payload.rag, citations: payload.citations },
      question: payload.question,
      language: payload.language,
      userToken: this.getUserToken?.() ?? null,
    })) as RustChatTurn;
    return {
      answer: turn.answer,
      provenance: turn.provenance ?? [],
      aiAssessed: !!turn.ai_assessed,
    };
  }
}

/** Documented direct-proxy path (deferred): a future proxy /copilot endpoint.
 *  The Tauri path above is production; this stays as the seam's record. */
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
