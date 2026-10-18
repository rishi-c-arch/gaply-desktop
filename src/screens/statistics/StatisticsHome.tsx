// Gaply — the Statistics method selector (Research OS, Phase 4B-2), at
// /app/statistics. It lists what Rust's capability registry says this build
// can run, read through capability_list, which needs no workspace (decision
// 76). The screen never decides what is runnable:
//
// * a capability gets a card if and only if Rust says it is runnable;
// * Planned ones appear only on the roadmap panel, Deprecated ones only in
//   the list of retired methods, and neither has a way to run;
// * the runnable stats.verify.* methods are one Stats Verifier card that
//   names them and links to its screen; stats.check links to the Stats Check
//   only while REACT_APP_FEATURE_STATS_CHECK is on; neither runs here;
// * the Stats Verifier card shows useEntitlement('stats_verifier'); the Stats
//   Check needs none; the descriptive summary and the t-test show none until
//   their tier is decided (decisions 9 and 66, decision 77).
//
// Behind REACT_APP_FEATURE_STATS_DESCRIBE, which hides unfinished UI and is
// not an authorization boundary.
import React, { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { isFeatureEnabled } from '../../config/featureFlags';
import { useEntitlement, type EntitlementStatus } from '../subscription/entitlement';
import { ENTITLEMENT_TEXT } from '../workspace/workspaceWording';
import { capabilityBridge, type CapabilityBridge } from './ttestBridge';
import { capabilitySentence, HOME_TEXT, LIFECYCLE_LABELS } from './capabilityWording';
import type { CapabilityView } from './ttestTypes';
import { DESCRIBE_PATH } from './describeRoute';
import { TTEST_PATH } from './statisticsRoute';

export const VERIFY_PREFIX = 'stats.verify.';
const CHECK_ID = 'stats.check';
const STATS_CHECK_PATH = '/app/check/stats';
const VERIFIER_PATH = '/app/statsverifier';

/** Where each runnable capability with a screen of its own opens. */
const ROUTES: Record<string, string> = {
  'stats.describe': DESCRIBE_PATH,
  'stats.analyze.independent_t': TTEST_PATH,
};

export interface StatisticsHomeProps {
  bridge?: CapabilityBridge;
  /** Test seam: an entitlement status instead of asking the server. */
  forceEntitlement?: EntitlementStatus;
}

/** What the screen draws, from Rust's list and the build's flags. */
export interface Placement {
  /** Runnable capabilities with their own card, in Rust's order. */
  cards: CapabilityView[];
  /** The runnable stats.verify.* methods, shown as one card. */
  verifier: CapabilityView[];
  /** stats.check, when runnable and its flag is on. */
  check: CapabilityView | null;
  roadmap: CapabilityView[];
  retired: CapabilityView[];
}

/** The one rule: a card only for what Rust says is runnable. */
export function place(list: CapabilityView[], statsCheckOn: boolean): Placement {
  const runnable = list.filter((c) => c.runnable === true);
  return {
    cards: runnable.filter((c) => c.id !== CHECK_ID && !c.id.startsWith(VERIFY_PREFIX)),
    verifier: runnable.filter((c) => c.id.startsWith(VERIFY_PREFIX)),
    check: statsCheckOn ? (runnable.find((c) => c.id === CHECK_ID) ?? null) : null,
    roadmap: list.filter((c) => c.lifecycle === 'planned'),
    retired: list.filter((c) => c.lifecycle === 'deprecated'),
  };
}

const Limitations: React.FC<{ c: CapabilityView }> = ({ c }) =>
  c.knownLimitations.length === 0 ? null : (
    <>
      <p style={{ margin: '8px 0 2px' }}>
        <strong>{HOME_TEXT.limitations}</strong>
      </p>
      <ul data-testid={`stats-limits-${c.id}`} style={{ margin: 0 }}>
        {c.knownLimitations.map((code) => (
          <li key={code}>{capabilitySentence(code)}</li>
        ))}
      </ul>
    </>
  );

const CapabilityCard: React.FC<{ c: CapabilityView; open: (path: string) => void }> = ({ c, open }) => {
  const route = ROUTES[c.id];
  const title = capabilitySentence(c.titleCode);
  return (
    <Card>
      <section data-testid={`stats-card-${c.id}`} aria-labelledby={`stats-title-${c.id}`}>
        <h3 id={`stats-title-${c.id}`} className="gds-card__title">
          {title}
        </h3>
        <p data-testid={`stats-lifecycle-${c.id}`} style={{ margin: 0 }}>
          {LIFECYCLE_LABELS[c.lifecycle]}
        </p>
        <p>{capabilitySentence(c.descriptionCode)}</p>
        <Limitations c={c} />
        {route ? (
          <a
            href={route}
            data-testid={`stats-open-${c.id}`}
            onClick={(e) => {
              e.preventDefault();
              open(route);
            }}
          >
            {HOME_TEXT.open}: {title}
          </a>
        ) : (
          <p data-testid={`stats-no-route-${c.id}`}>{HOME_TEXT.noRoute}</p>
        )}
      </section>
    </Card>
  );
};

/** The Stats Verifier's one card: its methods, its entitlement as shown by
 *  useEntitlement (presentation only; decision 9), and a link. */
const VerifierCard: React.FC<{
  methods: CapabilityView[];
  open: (path: string) => void;
  forceEntitlement?: EntitlementStatus;
}> = ({ methods, open, forceEntitlement }) => {
  const entitlement = useEntitlement('stats_verifier', undefined, forceEntitlement);
  return (
    <Card>
      <section data-testid="stats-card-verifier" aria-labelledby="stats-title-verifier">
        <h3 id="stats-title-verifier" className="gds-card__title">
          Statistical Analysis Verifier
        </h3>
        <p data-testid="stats-verifier-access" style={{ margin: 0 }}>
          {ENTITLEMENT_TEXT[entitlement.status]}
        </p>
        <p>{capabilitySentence(methods[0].descriptionCode)}</p>
        <p style={{ margin: '8px 0 2px' }}>{HOME_TEXT.verifierMethods}</p>
        <ul data-testid="stats-verifier-methods" style={{ margin: 0 }}>
          {methods.map((m) => (
            <li key={m.id}>{capabilitySentence(m.titleCode)}</li>
          ))}
        </ul>
        <a
          href={VERIFIER_PATH}
          data-testid="stats-open-verifier"
          onClick={(e) => {
            e.preventDefault();
            open(VERIFIER_PATH);
          }}
        >
          {HOME_TEXT.open}: Statistical Analysis Verifier
        </a>
      </section>
    </Card>
  );
};

export const StatisticsHome: React.FC<StatisticsHomeProps> = ({ bridge: given, forceEntitlement }) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => given ?? capabilityBridge(), [given]);
  const [list, setList] = useState<CapabilityView[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    bridge.list().then(
      (l) => alive && setList(l),
      () => alive && setError(HOME_TEXT.unread)
    );
    return () => {
      alive = false;
    };
  }, [bridge]);

  const placed = list ? place(list, isFeatureEnabled('statsCheck')) : null;

  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="stats-home">
      <p>{HOME_TEXT.intro}</p>
      {error && (
        <p role="alert" data-testid="stats-home-error">
          {error}
        </p>
      )}
      {placed && (
        <>
          <div data-testid="stats-cards" style={{ display: 'grid', gap: 12 }}>
            {placed.cards.map((c) => (
              <CapabilityCard key={c.id} c={c} open={navigate} />
            ))}
            {placed.check && (
              <Card>
                <section data-testid="stats-card-check" aria-labelledby="stats-title-check">
                  <h3 id="stats-title-check" className="gds-card__title">
                    {capabilitySentence(placed.check.titleCode)}
                  </h3>
                  <p data-testid="stats-lifecycle-check" style={{ margin: 0 }}>
                    {LIFECYCLE_LABELS[placed.check.lifecycle]}
                  </p>
                  <p>{capabilitySentence(placed.check.descriptionCode)}</p>
                  <Limitations c={placed.check} />
                  <a
                    href={STATS_CHECK_PATH}
                    data-testid="stats-open-check"
                    onClick={(e) => {
                      e.preventDefault();
                      navigate(STATS_CHECK_PATH);
                    }}
                  >
                    {HOME_TEXT.open}: {capabilitySentence(placed.check.titleCode)}
                  </a>
                </section>
              </Card>
            )}
            {placed.verifier.length > 0 && (
              <VerifierCard methods={placed.verifier} open={navigate} forceEntitlement={forceEntitlement} />
            )}
          </div>
          {placed.roadmap.length > 0 && (
            <aside aria-labelledby="stats-roadmap-heading" data-testid="stats-roadmap">
              <h3 id="stats-roadmap-heading">{HOME_TEXT.roadmap}</h3>
              <p>{HOME_TEXT.roadmapIntro}</p>
              <ul>
                {placed.roadmap.map((c) => (
                  <li key={c.id} data-testid={`stats-roadmap-${c.id}`}>
                    {capabilitySentence(c.titleCode)}
                  </li>
                ))}
              </ul>
            </aside>
          )}
          {placed.retired.length > 0 && (
            <aside aria-labelledby="stats-retired-heading" data-testid="stats-retired">
              <h3 id="stats-retired-heading">{HOME_TEXT.retired}</h3>
              <p>{HOME_TEXT.retiredIntro}</p>
              <ul>
                {placed.retired.map((c) => (
                  <li key={c.id} data-testid={`stats-retired-${c.id}`}>
                    {capabilitySentence(c.titleCode)}
                  </li>
                ))}
              </ul>
            </aside>
          )}
        </>
      )}
    </div>
  );
};

export default StatisticsHome;

/** /app/statistics — the page around the selector. */
export const StatisticsHomePage: React.FC<{ bridge?: CapabilityBridge }> = ({ bridge }) => {
  const navigate = useNavigate();
  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="statistics-home-page">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'statistics', label: HOME_TEXT.heading, icon: 'Σ' },
            ]}
            activeId="statistics"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={<HeaderBar title={HOME_TEXT.heading} />}
      >
        <Panel title="Statistical methods">
          <StatisticsHome bridge={bridge} />
        </Panel>
      </AppShell>
    </div>
  );
};
