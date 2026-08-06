// Gaply — Billing & plans (/app/billing). Razorpay checkout for INR/UPI+cards,
// plans ending in 9, annual effective-monthly, student discount, and the usage
// meters that double as conversion nudges. Free-offline stays unlimited.
import React, { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  AppShell,
  Badge,
  Button,
  Card,
  GaplyGlobe,
  HeaderBar,
  NavRail,
  Panel,
} from '../../design-system';
import { ToastProvider, useToast } from '../../design-system/Toast';
import { useGaplySession } from '../session/SessionProvider';
import { useSubscription } from './useSubscription';
import { formatInr, Plan, PLANS, studentPrice } from './pricing';
import { RazorpayClient, UnavailableRazorpayClient } from './razorpay';
import { useFeatureFlag } from '../../config/Feature';

export interface BillingPageProps {
  razorpay?: RazorpayClient;
}

const Inner: React.FC<BillingPageProps> = ({ razorpay }) => {
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const { toast } = useToast();
  const { tier, isPremium } = useSubscription();
  const paymentsEnabled = useFeatureFlag('payments');
  // Honest default: no fabricated success. Real checkout only when a real
  // client is injected AND the payments flag is on.
  const rzp = useMemo(() => razorpay ?? new UnavailableRazorpayClient(), [razorpay]);

  const [annual, setAnnual] = useState(true);
  const [student, setStudent] = useState(false);

  const priceFor = (p: Plan) => {
    const base = annual ? p.effectiveMonthlyInr : p.monthlyInr;
    return student ? studentPrice(base) : base;
  };

  const choose = async (p: Plan) => {
    // Payments are off: never fabricate a "started"/success. Say so plainly.
    if (!paymentsEnabled) {
      toast('Payments coming soon — you can’t be charged yet.', 'neutral');
      return;
    }
    const amount = annual ? (student ? studentPrice(p.annualInr) : p.annualInr) : priceFor(p);
    const res = await rzp.checkout({ planId: p.id, amountInr: amount, annual, student });
    // Only a REAL ok from a real client shows success; otherwise the real error.
    if (res.ok) {
      toast('Payment started — your plan activates once confirmed.', 'certain');
    } else {
      toast(res.error ?? 'Checkout failed', 'flagged');
    }
  };

  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="billing">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'billing', label: 'Plans & Billing', icon: '★' },
            ]}
            activeId="billing"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title="Plans & Billing">
            <Badge status={isPremium ? 'certain' : 'neutral'}>{isPremium ? 'premium' : 'free'}</Badge>
          </HeaderBar>
        }
      >
        <Panel title="Choose your plan">
          <div style={{ display: 'grid', gap: 16 }}>
            {/* free-offline reassurance */}
            <Card title="Free — always">
              <p style={{ margin: 0, color: 'var(--g-text-2)', fontSize: 14 }}>
                Every <strong>offline</strong> suite is unlimited, forever — extraction, statistical
                validation, AI check, local plagiarism, and report viewing all run on your device.
                Premium unlocks the cloud flagships.
              </p>
            </Card>

            {/* The premise this once carried — "server-side metering isn't
                deployed" — is no longer true. The proxy meters /verify per user
                and per period, and the shipped free limit is ZERO: entitlement.py
                returns not-entitled with reason `no_plan` when the tier limit is
                <= 0, so a free account's FIRST cloud call is refused. Saying
                "unlimited" here promised the opposite of what ships. */}
            {session && !isPremium && (
              <Card title="Cloud features">
                <p style={{ maxWidth: 480, color: 'var(--g-text-2)' }} data-testid="free-online-note">
                  Citation verification, journal checks and the PublishReady review run in the
                  cloud and need a paid plan. Every on-device suite stays free and unlimited.
                </p>
              </Card>
            )}

            {/* billing toggles */}
            <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
              <Button variant={annual ? 'primary' : 'secondary'} onClick={() => setAnnual(true)} data-testid="toggle-annual">Annual (2 months free)</Button>
              <Button variant={!annual ? 'primary' : 'secondary'} onClick={() => setAnnual(false)} data-testid="toggle-monthly">Monthly</Button>
              <label style={{ marginLeft: 'auto', fontSize: 13, color: 'var(--g-text-2)', display: 'flex', gap: 6, alignItems: 'center' }}>
                <input type="checkbox" checked={student} onChange={(e) => setStudent(e.target.checked)} data-testid="student-toggle" /> Student discount (50%)
              </label>
            </div>

            {/* plans */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 12 }} data-testid="plans">
              {PLANS.map((p) => (
                <Card key={p.id} title={p.name} data-testid={`plan-${p.id}`}>
                  <div style={{ fontSize: 26, fontWeight: 800 }} data-testid={`price-${p.id}`}>
                    {formatInr(priceFor(p))}<span style={{ fontSize: 13, color: 'var(--g-text-3)', fontWeight: 400 }}>/mo</span>
                  </div>
                  <div style={{ fontSize: 12, color: 'var(--g-text-3)' }}>
                    {annual ? `${formatInr(student ? studentPrice(p.annualInr) : p.annualInr)}/yr billed annually` : 'billed monthly'}
                  </div>
                  <p style={{ fontSize: 12, color: 'var(--g-text-3)', margin: '6px 0' }}>{p.tagline}</p>
                  <ul style={{ margin: '8px 0', paddingLeft: 18, fontSize: 13, color: 'var(--g-text-2)' }}>
                    {p.features.map((f) => <li key={f}>{f}</li>)}
                  </ul>
                  <Button
                    onClick={() => choose(p)}
                    disabled={isPremium || !paymentsEnabled}
                    title={!paymentsEnabled ? 'Payments coming soon' : undefined}
                    data-testid={`choose-${p.id}`}
                  >
                    {isPremium ? 'Current plan' : paymentsEnabled ? 'Choose' : 'Payments coming soon'}
                  </Button>
                </Card>
              ))}
            </div>
            <p style={{ fontSize: 12, color: 'var(--g-text-3)' }}>Pay with UPI or card via Razorpay (INR). Cancel anytime.</p>
          </div>
        </Panel>
      </AppShell>
    </div>
  );
};

const BillingPage: React.FC<BillingPageProps> = (props) => (
  <ToastProvider>
    <Inner {...props} />
  </ToastProvider>
);

export default BillingPage;
