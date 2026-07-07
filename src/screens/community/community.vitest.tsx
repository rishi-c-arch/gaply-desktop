// F13 — Research Co-Author community + integrity badge tests.
import React from 'react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import CommunityPage, { CommunityPost } from './CommunityPage';
import { classifyPost } from './badge';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const USER = { user: { id: 'u1', email: 'me@lab.edu' } };
function auth(session: any = USER): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: session === null }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}
function mockSvc(overrides: any = {}) {
  return {
    listChannels: vi.fn(), listPosts: vi.fn(),
    post: vi.fn().mockResolvedValue({ data: { id: 'srv-1' }, error: null, offline: false }),
    ...overrides,
  } as any;
}
function renderCommunity(props: any = {}, session: any = USER) {
  const svc = props.communityService ?? mockSvc();
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>
        <CommunityPage {...props} communityService={svc} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
  return svc;
}

const HUMAN = 'Quick question — my reviewer wants effect sizes for a chi-square. Which one should I report? Cramér’s V? Also is n=48 enough here, roughly?';
const AI_GENERATED =
  'Furthermore, it is important to note that navigating the realm of academic publishing plays a crucial role. ' +
  'Moreover, in conclusion, leveraging the ever-evolving tapestry of research is a testament to scholarly work. ' +
  'Additionally, it is worth noting that delving into the subject furthermore requires careful consideration always.';

/* ------------------------------ classifier ------------------------------ */

describe('badge classifier (deterministic)', () => {
  it('flags obvious AI-generated text as ai_generated', () => {
    expect(classifyPost(AI_GENERATED).badge).toBe('ai_generated');
  });
  it('passes a natural human question as human_written', () => {
    expect(classifyPost(HUMAN).badge).toBe('human_written');
  });
  it('flags a copied post as copied WITH a source', () => {
    const existing = [{ id: 'p9', message: HUMAN, author: 'Meera' }];
    const res = classifyPost(HUMAN, existing);
    expect(res.badge).toBe('copied');
    expect(res.source).toBe('p9');
    expect(res.detail).toMatch(/overlap with community post by Meera/);
  });
  it('is consistent — same text yields the same badge', () => {
    expect(classifyPost(AI_GENERATED).badge).toBe(classifyPost(AI_GENERATED).badge);
  });
});

/* ------------------------------ composer preview ------------------------ */

describe('composer badge preview (pre-publish)', () => {
  it('shows a live badge preview as the user types', async () => {
    renderCommunity();
    fireEvent.change(await screen.findByTestId('composer-input'), { target: { value: AI_GENERATED } });
    const preview = screen.getByTestId('badge-preview');
    await waitFor(() => expect(within(preview).getByTestId('badge-ai_generated')).toBeTruthy());
  });
});

/* ------------------------------ posting + storage ----------------------- */

describe('posting attaches an immovable badge', () => {
  it('a posted AI-generated message stores + shows 🔴 and passes the badge to Supabase', async () => {
    const svc = renderCommunity();
    fireEvent.change(await screen.findByTestId('composer-input'), { target: { value: AI_GENERATED } });
    fireEvent.click(screen.getByTestId('post-btn'));

    // stored with the badge (metadata) to Supabase
    await waitFor(() => expect(svc.post).toHaveBeenCalled());
    const [, , message, badge] = svc.post.mock.calls[0];
    expect(badge.integrity_badge).toBe('ai_generated');
    // shown on the post in the feed
    expect(within(screen.getByTestId('feed')).getByTestId('badge-ai_generated')).toBeTruthy();
    // no manuscript-ish content in the badge metadata (it's a label + reason)
    expect(JSON.stringify(badge)).not.toMatch(/full_?text|manuscript/i);
  });

  it('a copied post shows 📋 with its source link', async () => {
    const seeded: CommunityPost[] = [
      { id: 'orig', channelId: 'publishready-help', userId: 'u2', author: 'Meera', message: HUMAN, badge: 'human_written', badgeDetail: null, createdAt: '2026-07-01T00:00:00Z' },
    ];
    renderCommunity({ initialPosts: seeded });
    fireEvent.change(screen.getByTestId('composer-input'), { target: { value: HUMAN } }); // copy
    fireEvent.click(screen.getByTestId('post-btn'));
    await waitFor(() => expect(within(screen.getByTestId('feed')).getAllByTestId('badge-copied').length).toBeGreaterThan(0));
    // the copied post's thread shows the source
    fireEvent.click(screen.getAllByTestId(/^post-/)[0]);
    expect(screen.getByTestId('copied-source').textContent).toMatch(/orig/);
  });

  it('the badge is immovable — there is no author affordance to change/remove it', async () => {
    const seeded: CommunityPost[] = [
      { id: 'mine', channelId: 'publishready-help', userId: 'u1', author: 'me@lab.edu', message: AI_GENERATED, badge: 'ai_generated', badgeDetail: 'flagged', createdAt: '2026-07-01T00:00:00Z' },
    ];
    renderCommunity({ initialPosts: seeded });
    // the badge renders on my own post...
    expect(within(screen.getByTestId('feed')).getByTestId('badge-ai_generated')).toBeTruthy();
    // ...and there is NO control to edit/remove the badge (only a report button)
    expect(screen.queryByTestId('remove-badge')).toBeNull();
    expect(screen.queryByTestId('edit-badge')).toBeNull();
    expect(screen.getByTestId('report-mine')).toBeTruthy();
  });
});

/* ------------------------------ RLS (static) ---------------------------- */

describe('RLS: a user cannot edit another user’s post (policy definition)', () => {
  it('community_posts update policy is uid-scoped and the badge is trigger-locked', () => {
    const sql0001 = readFileSync(join(__dirname, '../../../supabase/migrations/0001_auth_metadata_schema.sql'), 'utf8').replace(/--[^\n]*/g, '');
    expect(sql0001).toMatch(/community_posts_update_own[\s\S]{0,160}auth\.uid\(\)\s*=\s*user_id/i);
    const sql0003 = readFileSync(join(__dirname, '../../../supabase/migrations/0003_community_integrity_badge.sql'), 'utf8').replace(/--[^\n]*/g, '');
    // the badge is immovable at the DB level (trigger blocks changing it)
    expect(sql0003).toMatch(/integrity_badge is immovable/i);
    expect(sql0003).toMatch(/create trigger community_posts_lock_badge/i);
  });
});
