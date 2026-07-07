// Gaply — Supabase integration (auth + metadata ONLY; the manuscript itself
// never touches Supabase). Public surface for the rest of the app.
export { getSupabase, isSupabaseConfigured, isOfflineMode } from './client';
export { createAuthService } from './auth';
export type { AuthService, AuthResult, SessionResult, OAuthProvider } from './auth';
export {
  createProfileService,
  createSubscriptionService,
  createAnalysisHistoryService,
  createCitationLibraryService,
  createUsageService,
  createCommunityService,
} from './data';
export type { DataResult } from './data';
export type {
  ProfileRow,
  SubscriptionRow,
  SubscriptionTier,
  AnalysisHistoryRow,
  CertaintySummary,
  CitationLibraryRow,
  UsageCounterRow,
  CommunityChannelRow,
  CommunityPostRow,
} from './types';
