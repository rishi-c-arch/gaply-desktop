// Gaply — "is the local AI usable?", in one place.
//
// Every AI surface needs this answer, and each computing it separately is how
// two screens end up disagreeing about whether a feature exists.
import { aiBridge, AiModelStatus } from './aiBridge';
import { isNotInstalled } from './AiStatusPanel';

export { aiBridge };
export type { AiModelStatus };

/** Resolves false rather than throwing: "not available" is a state, not an error. */
export async function isAiReady(bridge: Pick<typeof aiBridge, 'modelStatus'>): Promise<boolean> {
  try {
    return !isNotInstalled(await bridge.modelStatus());
  } catch {
    return false;
  }
}
