// Gaply — Research Co-Author community (F13). Slack/Discord three-column forum
// where EVERY post is integrity-scanned and badged by architecture. Posts live
// in Supabase (RLS); the badge is computed locally BEFORE publish, stored with
// the post, shown always, and cannot be removed by the author.
import React, { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  AppShell,
  Badge,
  Button,
  GaplyGlobe,
  HeaderBar,
  NavRail,
  Panel,
  ThreePanelWorkspace,
} from '../../design-system';
import { ToastProvider, useToast } from '../../design-system/Toast';
import { createCommunityService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { BADGE_META, BadgeResult, classifyPost, ExistingPost, IntegrityBadge } from './badge';
import { mayUseCloud } from '../settings/settingsStore';
import './community.css';

/** The fixed channel set (mirrors the Supabase community_channels; shown even
 *  offline so the forum is browsable). */
const CHANNELS = [
  { id: 'publishready-help', name: 'publishready-help' },
  { id: 'statistics-help', name: 'statistics-help' },
  { id: 'predatory-warnings', name: 'predatory-warnings' },
  { id: 'research-gaps', name: 'research-gaps' },
  { id: 'phd-global', name: 'phd-global' },
  { id: 'general', name: 'general' },
];

export interface CommunityPost {
  id: string;
  channelId: string;
  userId: string;
  author: string;
  message: string;
  badge: IntegrityBadge;
  badgeDetail: string | null;
  badgeSource?: string;
  createdAt: string;
  replies?: CommunityPost[];
}

export interface CommunityPageProps {
  communityService?: ReturnType<typeof createCommunityService>;
  /** Seed posts (tests / offline demo). */
  initialPosts?: CommunityPost[];
}

const BadgeChip: React.FC<{ badge: IntegrityBadge; detail?: string | null }> = ({ badge, detail }) => {
  const m = BADGE_META[badge];
  return (
    <span className="gds-badge-chip" data-status={m.status} data-badge={badge} data-testid={`badge-${badge}`} title={detail ?? m.label}>
      {m.icon} {m.label}
    </span>
  );
};

const Inner: React.FC<CommunityPageProps> = ({ communityService, initialPosts = [] }) => {
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const { toast } = useToast();
  const svc = useMemo(() => communityService ?? createCommunityService(), [communityService]);

  const [channel, setChannel] = useState('publishready-help');
  const [posts, setPosts] = useState<CommunityPost[]>(initialPosts);
  const [selectedId, setSelectedId] = useState<string | null>(initialPosts[0]?.id ?? null);
  const [draft, setDraft] = useState('');

  const channelPosts = posts.filter((p) => p.channelId === channel);
  const selected = posts.find((p) => p.id === selectedId) ?? null;

  // live badge preview in the composer (recomputed as you type)
  const preview: BadgeResult | null = useMemo(
    () => (draft.trim().length >= 8 ? classifyPost(draft, existingFor(posts, channel)) : null),
    [draft, posts, channel]
  );

  const submit = async () => {
    const text = draft.trim();
    if (!text || !session) return;
    // F14 privacy gate — posting publishes to Supabase; consent off means the
    // message never leaves the device.
    if (!mayUseCloud('community_sync')) {
      toast('Community posting is turned off in Settings → Sync & Privacy', 'assessed');
      return;
    }
    // Integrity pipeline BEFORE publish — the same computation as the preview.
    const badge = classifyPost(text, existingFor(posts, channel));
    const res = await svc.post(channel, session.user.id, text, {
      integrity_badge: badge.badge,
      badge_detail: badge.detail,
    });
    const post: CommunityPost = {
      id: (res.data as any)?.id ?? `local-${posts.length}`,
      channelId: channel,
      userId: session.user.id,
      author: session.user.email ?? 'you',
      message: text,
      badge: badge.badge, // IMMOVABLE — stored, never author-editable
      badgeDetail: badge.detail,
      badgeSource: badge.source,
      createdAt: new Date().toISOString(),
    };
    setPosts((xs) => [post, ...xs]);
    setSelectedId(post.id);
    setDraft('');
    if (badge.badge === 'copied') toast('Posted — flagged 📋 COPIED (source attached)', 'flagged');
    else if (badge.badge === 'ai_generated') toast('Posted — flagged 🔴 AI GENERATED', 'flagged');
    else toast('Posted', 'certain');
  };

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="community">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'community', label: 'Research Co-Author', icon: '☍' },
            ]}
            activeId="community"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={<HeaderBar title="Research Co-Author"><Badge status="certain">every post integrity-badged</Badge></HeaderBar>}
      >
        <ThreePanelWorkspace
          outline={
            <Panel title="Channels">
              <div className="gds-comm-channels" data-testid="channels">
                {CHANNELS.map((c) => (
                  <button
                    key={c.id}
                    className="gds-comm-channel"
                    aria-current={channel === c.id}
                    data-testid={`channel-${c.id}`}
                    onClick={() => setChannel(c.id)}
                  >
                    <span className="gds-comm-channel__hash">#</span>{c.name}
                  </button>
                ))}
              </div>
            </Panel>
          }
          inspector={
            <Panel title="Thread">
              {selected ? (
                <div className="gds-comm-detail" data-testid="thread-detail">
                  <BadgeChip badge={selected.badge} detail={selected.badgeDetail} />
                  <div className="gds-comm-post__author">{selected.author}</div>
                  <div className="gds-comm-post__body">{selected.message}</div>
                  {selected.badge === 'copied' && selected.badgeSource && (
                    <div style={{ fontSize: 12, color: 'var(--g-flagged)' }} data-testid="copied-source">
                      📋 Source: <span className="gds-mono">{selected.badgeSource}</span>
                    </div>
                  )}
                  <div style={{ fontSize: 11, color: 'var(--g-text-3)' }}>{selected.badgeDetail}</div>
                  {(selected.replies ?? []).map((r) => (
                    <div key={r.id} className="gds-comm-reply">
                      <BadgeChip badge={r.badge} /> <span>{r.message}</span>
                    </div>
                  ))}
                </div>
              ) : (
                <p style={{ color: 'var(--g-text-3)', fontSize: 13 }}>Select a post to view the thread.</p>
              )}
            </Panel>
          }
        >
          <Panel title={`#${channel}`}>
            <div className="gds-comm-feed" data-testid="feed">
              {channelPosts.length === 0 ? (
                <p style={{ color: 'var(--g-text-3)', fontSize: 13 }} data-testid="empty-channel">No posts yet — start the thread.</p>
              ) : (
                channelPosts.map((p) => (
                  <button
                    key={p.id}
                    className="gds-comm-post"
                    aria-current={p.id === selectedId}
                    data-testid={`post-${p.id}`}
                    onClick={() => setSelectedId(p.id)}
                  >
                    <div className="gds-comm-post__head">
                      <span className="gds-comm-post__author">{p.author}</span>
                      <BadgeChip badge={p.badge} detail={p.badgeDetail} />
                      <span className="gds-comm-post__time">{new Date(p.createdAt).toLocaleString()}</span>
                    </div>
                    <div className="gds-comm-post__body">{p.message}</div>
                    <div className="gds-comm-post__foot">
                      <span className="gds-comm-react">👍 react</span>
                      <span className="gds-comm-react">💬 reply</span>
                      <span className="gds-comm-report" data-testid={`report-${p.id}`}>report</span>
                    </div>
                  </button>
                ))
              )}
            </div>

            {/* composer with live badge preview */}
            <div className="gds-comm-composer" data-testid="composer">
              <textarea
                placeholder={session ? `Post to #${channel} — it will be integrity-scanned before publishing…` : 'Sign in to post'}
                value={draft}
                data-testid="composer-input"
                onChange={(e) => setDraft(e.target.value)}
                disabled={!session}
              />
              <div className="gds-comm-composer__row">
                <div className="gds-comm-preview" data-testid="badge-preview">
                  {preview ? (
                    <>Badge preview: <BadgeChip badge={preview.badge} detail={preview.detail} /></>
                  ) : (
                    <span>Badge preview appears as you type — you can’t remove it.</span>
                  )}
                </div>
                <Button style={{ marginLeft: 'auto' }} onClick={submit} disabled={!session || draft.trim().length < 8} data-testid="post-btn">
                  Post
                </Button>
              </div>
            </div>
          </Panel>
        </ThreePanelWorkspace>
      </AppShell>
    </div>
  );
};

/** Other posts in the channel + all posts as plagiarism comparison corpus. */
function existingFor(posts: CommunityPost[], _channel: string): ExistingPost[] {
  return posts.map((p) => ({ id: p.id, message: p.message, author: p.author }));
}

const CommunityPage: React.FC<CommunityPageProps> = (props) => (
  <ToastProvider>
    <Inner {...props} />
  </ToastProvider>
);

export default CommunityPage;
