/**
 * Human-like TTS via OpenAI (tts-1-hd). Chunks long text and plays in sequence.
 * Returns a controller with stop() to cancel playback.
 */

const TTS_CHUNK_SIZE = 4000;

function chunkText(text: string): string[] {
  const trimmed = text.replace(/\s+/g, ' ').trim();
  if (trimmed.length <= TTS_CHUNK_SIZE) return [trimmed];
  const chunks: string[] = [];
  const sentences = trimmed.split(/(?<=[.!?])\s+/);
  let current = '';
  for (const s of sentences) {
    if (current.length + s.length + 1 <= TTS_CHUNK_SIZE) {
      current += (current ? ' ' : '') + s;
    } else {
      if (current) chunks.push(current);
      current = s.length <= TTS_CHUNK_SIZE ? s : s.slice(0, TTS_CHUNK_SIZE);
    }
  }
  if (current) chunks.push(current);
  return chunks;
}

export type HumanTTSController = {
  stop: () => void;
  pause: () => void;
  resume: () => void;
};

export async function playHumanTTS(
  text: string,
  options: {
    voice?: string;
    onEnd?: () => void;
    onError?: (err: string) => void;
    apiFetch: (input: string, init?: RequestInit) => Promise<Response>;
  }
): Promise<HumanTTSController> {
  const voice = options.voice || 'nova';
  const chunks = chunkText(text);
  let stopped = false;
  let paused = false;
  let currentAudio: HTMLAudioElement | null = null;

  const stop = () => {
    stopped = true;
    paused = false;
    if (currentAudio) {
      currentAudio.pause();
      currentAudio.src = '';
      currentAudio = null;
    }
  };

  const pause = () => {
    if (currentAudio && !stopped) {
      paused = true;
      currentAudio.pause();
    }
  };

  const resume = () => {
    if (currentAudio && paused && !stopped) {
      paused = false;
      void currentAudio.play();
    }
  };

  const playNext = async (index: number): Promise<void> => {
    if (stopped || index >= chunks.length) {
      options.onEnd?.();
      return;
    }
    try {
      const res = await options.apiFetch('/api/ai/tts', {
        method: 'POST',
        body: JSON.stringify({ text: chunks[index], voice }),
      });
      if (!res.ok) {
        const err = await res.text();
        options.onError?.(`TTS failed: ${res.status} ${err}`);
        options.onEnd?.();
        return;
      }
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const audio = new Audio(url);
      currentAudio = audio;
      audio.onended = () => {
        URL.revokeObjectURL(url);
        currentAudio = null;
        if (stopped) {
          options.onEnd?.();
          return;
        }
        playNext(index + 1);
      };
      audio.onerror = () => {
        URL.revokeObjectURL(url);
        options.onError?.('Playback failed');
        options.onEnd?.();
      };
      await audio.play();
    } catch (e: any) {
      options.onError?.(e?.message || 'TTS request failed');
      options.onEnd?.();
    }
  };

  playNext(0);
  return { stop, pause, resume };
}
