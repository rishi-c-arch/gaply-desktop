// Gaply — onboarding (3 steps, every step skippable). Writes profile METADATA
// to the profiles table (role, field, target journals, country) and ends by
// offering a sample manuscript so a first scan is reachable in under a minute.
// ONLINE-ONLY (wrapped in <RequireSession> at the route): it writes to the
// user's own profiles row, which needs a session for RLS to attribute it.
import React, { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button, Card } from '../../design-system';
import { createProfileService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import '../auth/auth.css';

const ROLES = [
  { id: 'phd_scholar', label: 'PhD scholar', hint: 'Writing or defending a thesis' },
  { id: 'research_guide', label: 'Research guide', hint: 'Supervising researchers and reviews' },
  { id: 'institution', label: 'Institution', hint: 'Integrity checks at department scale' },
];

export interface OnboardingPageProps {
  /** Test seam — production uses the default (env-configured) service. */
  profileService?: ReturnType<typeof createProfileService>;
}

const OnboardingPage: React.FC<OnboardingPageProps> = ({ profileService }) => {
  const { session } = useGaplySession();
  const navigate = useNavigate();
  const profiles = useMemo(() => profileService ?? createProfileService(), [profileService]);

  const [step, setStep] = useState(0); // 0=role, 1=field+journals, 2=country, 3=done
  const [role, setRole] = useState<string | null>(null);
  const [field, setField] = useState('');
  const [journals, setJournals] = useState('');
  const [country, setCountry] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveNote, setSaveNote] = useState<string | null>(null);

  const finish = async (thenSample: boolean) => {
    const user = session?.user;
    if (user) {
      setSaving(true);
      const res = await profiles.upsertOwn({
        id: user.id,
        email: user.email ?? '',
        role: role ?? 'researcher',
        field: field || null,
        target_journals: journals || null,
        country: country || null,
      });
      setSaving(false);
      if (res.error) setSaveNote(`Profile not saved (${res.error}) — you can update it later in Settings.`);
    }
    navigate(thenSample ? '/app?sample=1' : '/app');
  };

  const next = () => setStep((s) => Math.min(3, s + 1));

  return (
    <div className="gds-root gds-onboarding" data-testid="onboarding-page">
      <Card glass className="gds-onboarding__card">
        <div className="gds-onboarding__steps" aria-hidden="true">
          {[0, 1, 2, 3].map((i) => (
            <span key={i} className="gds-onboarding__step-dot" data-active={i <= step} />
          ))}
        </div>

        {step === 0 && (
          <>
            <h2 style={{ margin: 0 }}>What describes you best?</h2>
            <div className="gds-onboarding__choices" role="group" aria-label="Role">
              {ROLES.map((r) => (
                <button
                  key={r.id}
                  type="button"
                  className="gds-onboarding__choice"
                  aria-pressed={role === r.id}
                  onClick={() => setRole(r.id)}
                  data-testid={`role-${r.id}`}
                >
                  <strong>{r.label}</strong>
                  <div style={{ color: 'var(--g-text-3)', fontSize: 12 }}>{r.hint}</div>
                </button>
              ))}
            </div>
            <div className="gds-onboarding__row">
              <Button variant="ghost" onClick={next}>Skip</Button>
              <Button onClick={next} disabled={!role}>Continue</Button>
            </div>
          </>
        )}

        {step === 1 && (
          <>
            <h2 style={{ margin: 0 }}>Your field & target journals</h2>
            <div className="gds-auth__field">
              <label className="gds-auth__label" htmlFor="ob-field">Research field</label>
              <input
                id="ob-field"
                className="gds-auth__input"
                placeholder="e.g. cognitive neuroscience"
                value={field}
                onChange={(e) => setField(e.target.value)}
              />
            </div>
            <div className="gds-auth__field">
              <label className="gds-auth__label" htmlFor="ob-journals">Typical target journals</label>
              <input
                id="ob-journals"
                className="gds-auth__input"
                placeholder="e.g. Nature Neuroscience, eLife"
                value={journals}
                onChange={(e) => setJournals(e.target.value)}
              />
            </div>
            <div className="gds-onboarding__row">
              <Button variant="ghost" onClick={next}>Skip</Button>
              <Button onClick={next}>Continue</Button>
            </div>
          </>
        )}

        {step === 2 && (
          <>
            <h2 style={{ margin: 0 }}>Where are you based?</h2>
            <div className="gds-auth__field">
              <label className="gds-auth__label" htmlFor="ob-country">Country</label>
              <input
                id="ob-country"
                className="gds-auth__input"
                placeholder="e.g. India"
                value={country}
                onChange={(e) => setCountry(e.target.value)}
              />
            </div>
            <div className="gds-onboarding__row">
              <Button variant="ghost" onClick={next}>Skip</Button>
              <Button onClick={next}>Continue</Button>
            </div>
          </>
        )}

        {step === 3 && (
          <>
            <h2 style={{ margin: 0 }}>You're set.</h2>
            <p style={{ margin: 0, color: 'var(--g-text-2)' }}>
              Try Gaply on a sample manuscript — your first integrity scan is under a minute away.
            </p>
            {saveNote && <p className="gds-auth__error">{saveNote}</p>}
            <div className="gds-onboarding__row">
              <Button variant="ghost" onClick={() => finish(false)} disabled={saving} data-testid="finish-plain">
                Go to dashboard
              </Button>
              <Button onClick={() => finish(true)} disabled={saving} data-testid="finish-sample">
                Scan a sample manuscript
              </Button>
            </div>
          </>
        )}
      </Card>
    </div>
  );
};

export default OnboardingPage;
