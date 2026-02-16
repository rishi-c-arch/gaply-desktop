# Build Audit – Gaply React Frontend

## Build Warnings (Do NOT cause runtime failures)

These are ESLint warnings. The build completes successfully. They do **not** cause the blank/loading page.

| File | Warning | Impact |
|------|---------|--------|
| AcademicAIRemoverPage.tsx | `handleParaphraseClientSide` unused | None |
| ExpertSearch.tsx | `getMockSearchResults`, `SearchResponse`, `handleExpertClick` unused | None |
| ExpertSearchResultsPage.tsx | `ExpertSearch` unused | None |
| HTMLReportGenerator.tsx | `formatSectionScore` unused | None |
| HireRequestDialog.tsx | `expertSearchAPI` unused, escape chars | None |
| JournalMatchingPage.tsx | `domainAnalysis` unused | None |
| LoginPage.tsx | `Text` unused | None |
| ManuscriptOrchestratorPage.tsx | `resultPayload`, `lineReview`, `refereeExpanded` unused | None |
| PackageSelection.tsx | `subscription` unused | None |
| SignupPage.tsx | `Text` unused | None |
| StatisticalResearchOrchestratorPage.tsx | Multiple unused vars | None |
| TurnitinStyleReportGenerator.tsx | `getTypeColor` unused | None |
| AuthContext.tsx | useEffect missing `refreshSubscription` dep | Minor – may cause stale closure |
| premiumService.ts | `API_BASE_URL` unused | None |

## Actual Causes of Loading/Blank Page

1. **Static fallback** – `index.html` shows "Document Analysis Orchestrator" / "Loading Gaply…" until React renders content. If React is slow or fails, this stays visible.
2. **Path-based entry** – `/final-orchestrator` should load `FinalOrchestratorApp` (no Three.js). Other routes load full App with Three.js.
3. **Auth init** – `AuthContext` calls `verifyToken()` on mount; a slow/failing backend can delay `isLoading`.

## Fixes Applied

- More aggressive fallback hiding (timeout + `data-app-mounted` check)
- Path-based entry already in place for `/final-orchestrator`
