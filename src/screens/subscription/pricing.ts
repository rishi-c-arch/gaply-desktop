// Gaply — pricing. Prices end in 9 (₹299/₹499/₹999); annual shows the effective
// monthly (two months free). Student-verification discount available.
export interface Plan {
  id: string;
  name: string;
  /** INR / month. */
  monthlyInr: number;
  /** INR / year (2 months free, rounded to end in 9). */
  annualInr: number;
  /** Effective INR / month when billed annually. */
  effectiveMonthlyInr: number;
  tagline: string;
  features: string[];
}

/** annual = 10× monthly (two months free), forced to end in 9. */
function annualFrom(monthly: number): number {
  const base = monthly * 10; // e.g. 2990
  return Math.floor(base / 10) * 10 + 9; // 2999
}

function plan(id: string, name: string, monthly: number, tagline: string, features: string[]): Plan {
  const annual = annualFrom(monthly);
  return {
    id,
    name,
    monthlyInr: monthly,
    annualInr: annual,
    effectiveMonthlyInr: Math.round(annual / 12),
    tagline,
    features,
  };
}

export const PLANS: Plan[] = [
  plan('basic', 'Basic', 299, 'For a single manuscript push', [
    'Everything free, unlimited offline',
    'PublishReady simulated review',
    'Research Copilot chat',
  ]),
  plan('pro', 'Pro', 499, 'For active researchers', [
    'Everything in Basic',
    'Deep plagiarism (Copyleaks)',
    'Unlimited online verifications & journal checks',
  ]),
  plan('max', 'Max', 999, 'For labs & supervisors', [
    'Everything in Pro',
    'Priority cloud review',
    'Multiple manuscripts & collaborators',
  ]),
];

export const STUDENT_DISCOUNT = 0.5;

/** Apply the student discount, forced to end in 9. */
export function studentPrice(inr: number): number {
  const d = Math.round(inr * (1 - STUDENT_DISCOUNT)); // e.g. 150
  return Math.floor(d / 10) * 10 + 9; // 149
}

export function formatInr(inr: number): string {
  return `₹${inr.toLocaleString('en-IN')}`;
}
