// Gaply — Level 1 AI settings (Phase 2, Step 7). Transport T-BYOK: the user's
// own OpenAI key, saved to the OS keychain by Rust. The screen shows only
// "Key saved" or "No key": no command returns the key, and the field is
// cleared the moment it is sent.
//
// T-PROXY (an account status) is not shown: Phase 2's decision is T-BYOK and
// no proxy transport exists for these tasks, so there is no account state to
// report here.
import React, { useEffect, useState } from 'react';
import { Button } from '../../../design-system';
import type { StatsL1Bridge } from '../statsL1Bridge';
import type { AiSettings, L1UsageView } from '../statsL1Types';
import { refusalSentence, TASK_NAMES } from './l1Wording';

const KEY_TEST_WORDS: Record<string, string> = {
  accepted: 'OpenAI accepted the key.',
  no_key: 'No key is saved.',
  rejected: 'OpenAI rejected the key.',
  offline: 'OpenAI could not be reached; this device appears to be offline.',
  timeout: 'The test timed out.',
  failed: 'The test failed.',
};

export function keyTestSentence(result: string): string {
  if (KEY_TEST_WORDS[result]) return KEY_TEST_WORDS[result];
  const m = /^http_(\d+)$/.exec(result);
  return m ? `OpenAI answered the test with status ${m[1]}.` : 'The test failed.';
}

export interface L1SettingsProps {
  bridge: StatsL1Bridge;
  settings: AiSettings;
  onSettings: (s: AiSettings) => void;
}

export const L1Settings: React.FC<L1SettingsProps> = ({ bridge, settings, onSettings }) => {
  const [saved, setSaved] = useState<boolean | null>(null);
  const [typed, setTyped] = useState('');
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    bridge.keyStatus().then((s) => live && setSaved(s.saved)).catch(() => live && setSaved(null));
    return () => {
      live = false;
    };
  }, [bridge]);

  const set = async (next: AiSettings) => {
    setMessage(null);
    try {
      onSettings(await bridge.settingSet(next));
    } catch (e) {
      setMessage(refusalSentence(e));
    }
  };

  const save = async () => {
    const key = typed;
    setTyped('');
    setMessage(null);
    try {
      setSaved((await bridge.keySave(key)).saved);
    } catch (e) {
      setMessage(refusalSentence(e));
    }
  };

  const test = async () => {
    setMessage(null);
    try {
      setMessage(keyTestSentence((await bridge.keyTest()).result));
    } catch (e) {
      setMessage(refusalSentence(e));
    }
  };

  const remove = async () => {
    setMessage(null);
    try {
      setSaved((await bridge.keyRemove()).saved);
    } catch (e) {
      setMessage(refusalSentence(e));
    }
  };

  return (
    <section className="gds-card" aria-labelledby="l1-settings-h" data-testid="l1-settings">
      <h4 id="l1-settings-h" style={{ marginTop: 0 }}>AI help</h4>
      <p className="gds-jc__disclaimer">
        Off by default. Every number and verdict comes from Gaply's engine on this device; AI help can only suggest a
        method or columns, explain a finished result, or find cited guidance, and only after you see the request and
        click Send.
      </p>
      <div>
        <label>
          <input
            type="checkbox"
            data-testid="l1-ai-help-switch"
            checked={settings.aiHelp}
            onChange={() => void set({ ...settings, aiHelp: !settings.aiHelp, webContext: settings.aiHelp ? false : settings.webContext })}
          />{' '}
          AI help
        </label>
      </div>
      <div>
        <label>
          <input
            type="checkbox"
            data-testid="l1-web-context-switch"
            checked={settings.webContext}
            disabled={!settings.aiHelp}
            onChange={() => void set({ ...settings, webContext: !settings.webContext })}
          />{' '}
          Method context from the web (needs AI help)
        </label>
      </div>

      <div style={{ marginTop: 8 }}>
        <p style={{ margin: '4px 0' }}>
          Your OpenAI key: <strong data-testid="l1-key-status">{saved === null ? 'Checking…' : saved ? 'Key saved' : 'No key'}</strong>
        </p>
        <label htmlFor="l1-key-input">OpenAI API key</label>{' '}
        <input
          id="l1-key-input"
          type="password"
          autoComplete="off"
          spellCheck={false}
          data-testid="l1-key-input"
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
        />{' '}
        <Button variant="secondary" onClick={() => void save()} disabled={!typed.trim()} data-testid="l1-key-save">Save key</Button>{' '}
        <Button variant="secondary" onClick={() => void test()} disabled={!saved} data-testid="l1-key-test">Test key</Button>{' '}
        <Button variant="ghost" onClick={() => void remove()} disabled={!saved} data-testid="l1-key-remove">Remove key</Button>
        <p className="gds-jc__disclaimer">
          The key goes to your operating system's keychain and is never shown again. You pay OpenAI for what you send.
        </p>
      </div>
      {message && <p role="status" data-testid="l1-settings-message">{message}</p>}
    </section>
  );
};

/** The run's usage line: every request that left the device, per task, and
 *  the tokens the replies reported. */
export const L1UsageLine: React.FC<{ usage: L1UsageView | null }> = ({ usage }) => {
  const used = usage?.tasks.filter((t) => t.requests > 0) ?? [];
  return (
    <p className="gds-jc__disclaimer" data-testid="l1-usage">
      {used.length === 0
        ? 'No request has left this device in this check.'
        : `Sent from this device in this check (every attempt, retries and failed calls included): ${used
            .map(
              (t) =>
                `${TASK_NAMES[t.task] ?? t.task}: ${t.requests} ${t.requests === 1 ? 'request' : 'requests'}, ${t.inputTokens} tokens in and ${t.outputTokens} out as the replies reported`
            )
            .join('; ')}.`}
    </p>
  );
};

export default L1Settings;
