// Part C follow-up 3: the Careers page's "Apply Now" mailto carries at most a
// subject, so in the desktop app it goes through the checked command
// (`open_external_link` accepts a listed address with one subject and refuses
// a body). Before, it called `window.open`, which the desktop webview never
// opens, with a body the command would refuse. The web build keeps window.open.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const { invoke, desktop } = vi.hoisted(() => ({
  invoke: vi.fn(async (_cmd: string, _args?: unknown) => undefined),
  desktop: { value: true },
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../utils/isTauri', () => ({
  get isTauri() {
    return desktop.value;
  },
  get default() {
    return desktop.value;
  },
}));

import CareerPage from './CareerPage';

afterEach(() => {
  cleanup();
  invoke.mockClear();
  vi.restoreAllMocks();
});

function apply() {
  render(
    <MemoryRouter>
      <CareerPage />
    </MemoryRouter>
  );
  fireEvent.click(screen.getByRole('button', { name: 'Apply Now' }));
}

/** The mailto's parts: the address and every parameter name. */
function parts(url: string) {
  const u = new URL(url);
  return { scheme: u.protocol, to: u.pathname, params: Array.from(u.searchParams.keys()), subject: u.searchParams.get('subject') };
}

describe('Careers "Apply Now"', () => {
  it('in the desktop app, opens a subject-only mailto through the checked command', async () => {
    desktop.value = true;
    const open = vi.spyOn(window, 'open').mockReturnValue(null);
    apply();
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    const [cmd, args] = invoke.mock.calls[0];
    expect(cmd).toBe('open_external_link');
    expect(parts((args as { url: string }).url)).toEqual({
      scheme: 'mailto:',
      to: 'helloresearcher@gaply.in',
      params: ['subject'],
      subject: 'Career Application - Gaply.in',
    });
    expect(open).not.toHaveBeenCalled();
  });

  it('on the web, opens the same subject-only mailto with window.open', () => {
    desktop.value = false;
    const open = vi.spyOn(window, 'open').mockReturnValue(null);
    apply();
    expect(open).toHaveBeenCalledTimes(1);
    expect(parts(String(open.mock.calls[0][0])).params).toEqual(['subject']);
    expect(invoke).not.toHaveBeenCalled();
  });
});
