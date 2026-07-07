// Gaply — Razorpay integration seam (INR / UPI + cards). Checkout runs
// client-side with the PUBLIC key (REACT_APP_RAZORPAY_KEY_ID); the secret and
// webhook verification live server-side. The webhook flips the subscriptions
// table; the frontend reads tier + status from Supabase (F2) to gate features.
import { SubscriptionRow } from '../../services/supabase';

export interface CheckoutRequest {
  planId: string;
  amountInr: number;
  annual: boolean;
  student: boolean;
}

export interface CheckoutResult {
  ok: boolean;
  razorpaySubscriptionId?: string;
  error?: string;
}

export interface RazorpayClient {
  /** Open Razorpay checkout for a plan; resolves after the user completes/aborts. */
  checkout(req: CheckoutRequest): Promise<CheckoutResult>;
}

/** Test/dev double — no real Razorpay SDK. */
export class MockRazorpayClient implements RazorpayClient {
  public requests: CheckoutRequest[] = [];
  constructor(private result?: CheckoutResult) {}
  async checkout(req: CheckoutRequest): Promise<CheckoutResult> {
    this.requests.push(req);
    return this.result ?? { ok: true, razorpaySubscriptionId: `sub_mock_${req.planId}` };
  }
}

/* --------------------------- webhook processing ------------------------- */

export interface RazorpayWebhookEvent {
  event: string; // 'subscription.activated' | 'subscription.charged' | 'subscription.cancelled' | 'subscription.halted' | ...
  payload: {
    subscription: { entity: { id: string; status: string } };
  };
}

/** The subscriptions-table update a webhook should apply. Pure + testable — a
 *  server webhook handler (or a mocked test) calls this, then upserts the row.
 *  Returns null for events we don't act on. */
export function subscriptionUpdateFromWebhook(
  event: RazorpayWebhookEvent
): Partial<SubscriptionRow> | null {
  const sub = event.payload?.subscription?.entity;
  if (!sub) return null;
  switch (event.event) {
    case 'subscription.activated':
    case 'subscription.charged':
    case 'subscription.resumed':
      return { tier: 'premium', status: 'active', razorpay_subscription_id: sub.id };
    case 'subscription.pending':
    case 'subscription.halted':
      return { status: 'halted', razorpay_subscription_id: sub.id };
    case 'subscription.cancelled':
    case 'subscription.completed':
      return { tier: 'free', status: 'cancelled', razorpay_subscription_id: sub.id };
    default:
      return null;
  }
}
