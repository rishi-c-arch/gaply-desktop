//! **The run state: the dataset, the claims, the proposals, the confirmed
//! bindings and the token store, in memory for the session.**
//!
//! Nothing here is persisted. A new dataset replaces the old one and kills
//! every token; an edit to a claim gives it a new revision and kills only that
//! claim's tokens. [`L1RunState::gate`] is the one door to a computation: it
//! takes a token and the claim the caller wants computed, and lets through
//! only a confirmed binding whose token is live and still bound to the live
//! dataset, claim revision, method version and engine version. Step 4's seam
//! takes its claims and bindings from here rather than extracting the
//! manuscript again.

use super::binding::{self, BindingDraft, BindingProposal, ConfirmedBinding, ProposalId, ProposalSource, Rejection};
use super::catalogue::{MethodCatalogue, VariantChoice};
use super::claim::{ClaimId, ClaimOrigin, L1Claim, RevisionHash, UserClaim};
use super::dataset::Dataset;
use super::propose::{propose_local, LocalProposal};
use super::token::{ConfirmationToken, RevokedBecause, TokenRecord, TokenRejection, TokenStore};
use super::ENGINE_VERSION;
use crate::stats_l0::input::DocumentContext;
use crate::stats_l0::number::NumberNotRead;

/// Why a token did not open the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateRejection {
    /// Not a token this run issued.
    Unknown,
    Revoked(RevokedBecause),
    /// A live token, but for another claim than the one asked for.
    ClaimMismatch,
    /// A live token whose record no longer matches the run: the field that
    /// moved.
    Stale(StaleField),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleField {
    Fingerprint,
    ClaimRevision,
    MethodVersion,
    EngineVersion,
    /// The binding is no longer held (should not happen; defensive).
    Binding,
}

/// Why a proposal could not be added or edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalError {
    NoDataset,
    UnknownClaim(ClaimId),
    UnknownProposal(ProposalId),
    MethodNotInCatalogue,
}

#[derive(Debug, Default)]
pub struct L1RunState {
    dataset: Option<Dataset>,
    claims: Vec<L1Claim>,
    proposals: Vec<BindingProposal>,
    next_proposal: u32,
    confirmed: Vec<(ConfirmedBinding, ConfirmationToken)>,
    tokens: TokenStore,
    /// The manuscript's alpha, one-tailed and correction mentions (Level 0's
    /// own reading), for comparing p. `None` for claims entered by hand.
    context: Option<DocumentContext>,
}

impl L1RunState {
    pub fn new() -> L1RunState {
        L1RunState::default()
    }

    /* ---- dataset ------------------------------------------------------ */

    /// Replace the dataset. Every proposal, confirmation and token dies: they
    /// were about the old bytes.
    pub fn load_dataset(&mut self, dataset: Dataset) {
        let revoked = self.tokens.revoke_where(RevokedBecause::DatasetReplaced, |_| true);
        self.proposals.clear();
        self.confirmed.clear();
        tracing::debug!(revoked, "stats_l1: dataset loaded; prior tokens revoked");
        self.dataset = Some(dataset);
    }

    pub fn dataset(&self) -> Option<&Dataset> {
        self.dataset.as_ref()
    }

    /* ---- claims ------------------------------------------------------- */

    /// Replace the claims with ones read from the manuscript. Ids are their
    /// positions; the claims passed in must carry them.
    pub fn set_claims(&mut self, claims: Vec<L1Claim>) {
        self.tokens.revoke_where(RevokedBecause::ClaimRevised, |_| true);
        self.proposals.clear();
        self.confirmed.clear();
        self.claims = claims;
    }

    pub fn claims(&self) -> &[L1Claim] {
        &self.claims
    }

    /// The manuscript's context, read by Level 0 (`DocumentContext::read`).
    /// Phase 2, Step 4: Level 1 compares p with it.
    pub fn set_context(&mut self, context: DocumentContext) {
        self.context = Some(context);
    }

    pub fn context(&self) -> Option<&DocumentContext> {
        self.context.as_ref()
    }

    pub fn claim(&self, id: ClaimId) -> Option<&L1Claim> {
        self.claims.iter().find(|c| c.id == id)
    }

    /// A claim the user adds, through [`crate::stats_l0::number::ReportedNumber`].
    pub fn add_claim(&mut self, claim: &UserClaim<'_>) -> Result<ClaimId, NumberNotRead> {
        let id = ClaimId(self.claims.iter().map(|c| c.id.0 + 1).max().unwrap_or(0));
        let c = L1Claim::user_entered(id, claim, ClaimOrigin::UserAdded)?;
        self.claims.push(c);
        Ok(id)
    }

    /// Edit one claim: a new revision; only that claim's proposals,
    /// confirmations and tokens die.
    pub fn edit_claim(&mut self, id: ClaimId, claim: &UserClaim<'_>) -> Result<RevisionHash, EditError> {
        let pos = self.claims.iter().position(|c| c.id == id).ok_or(EditError::UnknownClaim)?;
        let from = self.claims[pos].revision.clone();
        let edited = L1Claim::user_entered(id, claim, ClaimOrigin::UserEdited { from }).map_err(EditError::Number)?;
        let revision = edited.revision.clone();
        self.claims[pos] = edited;
        self.tokens.revoke_where(RevokedBecause::ClaimRevised, |r| r.claim == id);
        self.proposals.retain(|p| p.claim != id);
        self.confirmed.retain(|(b, _)| b.claim() != id);
        Ok(revision)
    }

    /* ---- proposals ---------------------------------------------------- */

    fn push_proposal(&mut self, claim: &L1Claim, binding: BindingDraft, source: ProposalSource) -> ProposalId {
        let fingerprint_digest = self.dataset.as_ref().map(|d| d.schema.fingerprint.digest()).unwrap_or_default();
        self.next_proposal += 1;
        let id = ProposalId(self.next_proposal);
        self.proposals.push(BindingProposal {
            id,
            claim: claim.id,
            claim_revision: claim.revision.clone(),
            fingerprint_digest,
            binding,
            source,
        });
        id
    }

    /// The local proposer's suggestions for `claim`, added to the run.
    pub fn propose_local(&mut self, claim: ClaimId) -> Result<Vec<ProposalId>, ProposalError> {
        let dataset = self.dataset.as_ref().ok_or(ProposalError::NoDataset)?;
        let c = self.claim(claim).ok_or(ProposalError::UnknownClaim(claim))?.clone();
        let drafts: Vec<LocalProposal> = propose_local(&dataset.schema, &c);
        Ok(drafts.into_iter().map(|p| self.push_proposal(&c, p.binding, p.source)).collect())
    }

    /// A proposal from the user or from an AI helper task. An AI suggestion's
    /// variant is ALWAYS reset to not stated: a model's choice is not the
    /// paper's statement.
    pub fn add_proposal(&mut self, claim: ClaimId, mut binding: BindingDraft, source: ProposalSource) -> Result<ProposalId, ProposalError> {
        if self.dataset.is_none() {
            return Err(ProposalError::NoDataset);
        }
        let c = self.claim(claim).ok_or(ProposalError::UnknownClaim(claim))?.clone();
        if matches!(source, ProposalSource::AiSuggestion { .. }) {
            let kind = MethodCatalogue::by_id(&binding.method_id).ok_or(ProposalError::MethodNotInCatalogue)?.variant_kind;
            binding.variant = VariantChoice::not_stated(kind);
        }
        Ok(self.push_proposal(&c, binding, source))
    }

    /// An edit: a NEW proposal, `Manual`, edited from the original's source.
    /// The original stays.
    pub fn edit_proposal(&mut self, id: ProposalId, binding: BindingDraft) -> Result<ProposalId, ProposalError> {
        let original = self.proposals.iter().find(|p| p.id == id).ok_or(ProposalError::UnknownProposal(id))?.clone();
        let c = self.claim(original.claim).ok_or(ProposalError::UnknownClaim(original.claim))?.clone();
        let source = ProposalSource::Manual { edited_from: Some(Box::new(original.source)) };
        Ok(self.push_proposal(&c, binding, source))
    }

    pub fn proposals(&self) -> &[BindingProposal] {
        &self.proposals
    }

    pub fn proposal(&self, id: ProposalId) -> Option<&BindingProposal> {
        self.proposals.iter().find(|p| p.id == id)
    }

    /* ---- confirmation ------------------------------------------------- */

    /// The one way to a confirmed binding and its token.
    pub fn confirm(&mut self, id: ProposalId) -> Result<(&ConfirmedBinding, ConfirmationToken), ConfirmError> {
        let proposal = self.proposals.iter().find(|p| p.id == id).ok_or(ConfirmError::UnknownProposal)?;
        let dataset = self.dataset.as_ref().ok_or(ConfirmError::NoDataset)?;
        let claim = self.claims.iter().find(|c| c.id == proposal.claim).ok_or(ConfirmError::UnknownClaim)?;
        let confirmed = binding::confirm(proposal, dataset, claim).map_err(ConfirmError::Rejected)?;
        let token = self.tokens.issue(TokenRecord {
            claim: confirmed.claim(),
            claim_revision: confirmed.claim_revision().clone(),
            fingerprint_digest: confirmed.fingerprint_digest().to_string(),
            method_id: confirmed.method().id().to_string(),
            method_version: confirmed.method().version(),
            binding_digest: confirmed.digest().to_string(),
            engine_version: ENGINE_VERSION,
        });
        self.confirmed.push((confirmed, token.clone()));
        Ok((&self.confirmed.last().expect("just pushed").0, token))
    }

    /// Withdraw a confirmation: its token dies.
    pub fn withdraw(&mut self, token: &ConfirmationToken) -> bool {
        let before = self.confirmed.len();
        self.confirmed.retain(|(_, t)| t != token);
        let removed = self.confirmed.len() != before;
        if removed {
            self.tokens.revoke(token, RevokedBecause::Withdrawn);
        }
        removed
    }

    pub fn confirmed(&self) -> impl Iterator<Item = &ConfirmedBinding> {
        self.confirmed.iter().map(|(b, _)| b)
    }

    /* ---- the gate ----------------------------------------------------- */

    /// The door to a computation. Returns the confirmed binding for `claim`
    /// only when `token` is live, was issued for that claim, and is still
    /// bound to the live dataset, claim revision, method version and engine.
    pub fn gate(&self, token: &ConfirmationToken, claim: ClaimId) -> Result<&ConfirmedBinding, GateRejection> {
        let record = self.tokens.lookup(token).map_err(|e| match e {
            TokenRejection::Unknown => GateRejection::Unknown,
            TokenRejection::Revoked(why) => GateRejection::Revoked(why),
        })?;
        if record.claim != claim {
            return Err(GateRejection::ClaimMismatch);
        }
        let dataset = self.dataset.as_ref().ok_or(GateRejection::Stale(StaleField::Fingerprint))?;
        if record.fingerprint_digest != dataset.schema.fingerprint.digest() {
            return Err(GateRejection::Stale(StaleField::Fingerprint));
        }
        let live = self.claim(claim).ok_or(GateRejection::Stale(StaleField::ClaimRevision))?;
        if record.claim_revision != live.revision {
            return Err(GateRejection::Stale(StaleField::ClaimRevision));
        }
        let (binding, _) = self.confirmed.iter().find(|(_, t)| t == token).ok_or(GateRejection::Stale(StaleField::Binding))?;
        if record.method_version != binding.method().version() || record.method_id != binding.method().id() {
            return Err(GateRejection::Stale(StaleField::MethodVersion));
        }
        if record.engine_version != ENGINE_VERSION {
            return Err(GateRejection::Stale(StaleField::EngineVersion));
        }
        if record.binding_digest != binding.digest() {
            return Err(GateRejection::Stale(StaleField::Binding));
        }
        Ok(binding)
    }

    pub fn live_tokens(&self) -> usize {
        self.tokens.live_count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmError {
    UnknownProposal,
    NoDataset,
    UnknownClaim,
    Rejected(Rejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    UnknownClaim,
    Number(NumberNotRead),
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;
    use crate::stats_l0::number::Relation;
    use crate::stats_l0::result::{read_results, Family, TableStructure};
    use crate::stats_l1::binding::RoleBinding;
    use crate::stats_l1::catalogue::{L1MethodId, RoleName, TVariant, Variant, VariantKind, VariantSetBy};
    use crate::stats_l1::claim::{claims_from_reading, ClaimLocation, UserP};
    use crate::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef};
    use crate::extract::SectionKind;

    const DOC: &str = "A Title\n\nResults\nReaction time rose in the treatment condition, t(4) = 2.10, p = .04. Accuracy fell, t(4) = 3.10, p = .02.\n";

    fn dataset(bytes: &[u8]) -> Dataset {
        let raw = RawTable::from_rows(
            vec!["reaction_time".into(), "condition".into(), "accuracy".into()],
            vec![
                vec!["410".into(), "control".into(), "0.9".into()],
                vec!["420".into(), "treatment".into(), "0.8".into()],
                vec!["430".into(), "control".into(), "0.7".into()],
                vec!["440".into(), "treatment".into(), "0.6".into()],
                vec!["450".into(), "control".into(), "".into()],
                vec!["460".into(), "treatment".into(), "0.4".into()],
            ],
        );
        build_dataset(raw, DataFingerprint::of_bytes(bytes, "p", TableRef::FirstTable)).unwrap()
    }

    fn state() -> L1RunState {
        let ex = extract_from_text(DOC);
        let reading = read_results(&ex, TableStructure::NotAvailable);
        let (claims, not_located) = claims_from_reading(&ex, &reading);
        assert_eq!((claims.len(), not_located), (2, 0), "{claims:?}");
        let mut s = L1RunState::new();
        s.load_dataset(dataset(b"v1"));
        s.set_claims(claims);
        s
    }

    fn t_draft() -> BindingDraft {
        BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T))
            .role(RoleName::Outcome, &[ColumnAlias(1)])
            .role(RoleName::Group, &[ColumnAlias(2)])
    }

    fn edited_claim<'a>(p: &'a str) -> UserClaim<'a> {
        UserClaim {
            family: Family::T,
            statistic: "3.10",
            df: vec!["4"],
            n: None,
            p: UserP::Reported { raw: p, relation: Relation::Eq },
            location: ClaimLocation { section_index: 1, section: SectionKind::Results, paragraph: 0, sentence: 1 },
            sentence: None,
        }
    }

    #[test]
    fn a_local_proposal_can_be_confirmed_and_its_token_opens_the_gate_for_its_claim_only() {
        let mut s = state();
        let ids = s.propose_local(ClaimId(0)).unwrap();
        assert!(!ids.is_empty());
        let (b, token) = s.confirm(ids[0]).unwrap();
        assert_eq!(b.claim(), ClaimId(0));
        assert!(matches!(b.source(), ProposalSource::Local { .. }));
        assert_eq!(s.gate(&token, ClaimId(0)).map(|_| ()), Ok(()));
        // The same token for the other claim.
        assert_eq!(s.gate(&token, ClaimId(1)).map(|_| ()), Err(GateRejection::ClaimMismatch));
        assert_eq!(s.live_tokens(), 1);
    }

    #[test]
    fn an_unconfirmed_proposal_has_no_token_and_a_forged_one_is_unknown() {
        let mut s = state();
        let id = s.add_proposal(ClaimId(0), t_draft(), ProposalSource::Manual { edited_from: None }).unwrap();
        assert!(s.proposal(id).is_some());
        assert_eq!(s.live_tokens(), 0, "proposing issues nothing");
        for forged in ["", "0", &"f".repeat(64), "ConfirmationToken(…)"] {
            assert_eq!(s.gate(&ConfirmationToken::presented(forged), ClaimId(0)).map(|_| ()), Err(GateRejection::Unknown));
        }
        // Confirming an id that was never proposed.
        assert_eq!(s.confirm(ProposalId(99)).unwrap_err(), ConfirmError::UnknownProposal);
    }

    #[test]
    fn editing_a_claim_kills_only_its_token_and_changes_only_its_revision() {
        let mut s = state();
        let a = s.add_proposal(ClaimId(0), t_draft(), ProposalSource::Manual { edited_from: None }).unwrap();
        let b = s.add_proposal(ClaimId(1), t_draft().roles_set(RoleName::Outcome, ColumnAlias(3)), ProposalSource::Manual { edited_from: None }).unwrap();
        let (_, ta) = s.confirm(a).unwrap();
        let (_, tb) = s.confirm(b).unwrap();
        let before: Vec<RevisionHash> = s.claims().iter().map(|c| c.revision.clone()).collect();

        let new_rev = s.edit_claim(ClaimId(1), &edited_claim(".021")).unwrap();
        assert_ne!(new_rev, before[1]);
        assert_eq!(s.claims()[0].revision, before[0], "the earlier claim's revision is unchanged");
        assert!(matches!(s.claims()[1].origin, ClaimOrigin::UserEdited { ref from } if *from == before[1]));
        assert_eq!(s.gate(&ta, ClaimId(0)).map(|_| ()), Ok(()), "the earlier claim's token lives");
        assert_eq!(s.gate(&tb, ClaimId(1)).map(|_| ()), Err(GateRejection::Revoked(RevokedBecause::ClaimRevised)));
        assert!(s.proposals().iter().all(|p| p.claim != ClaimId(1)));
        // A proposal made for the old revision cannot be confirmed later.
        let mut s2 = state();
        let p = s2.add_proposal(ClaimId(1), t_draft(), ProposalSource::Manual { edited_from: None }).unwrap();
        let stale = s2.proposal(p).unwrap().clone();
        s2.edit_claim(ClaimId(1), &edited_claim(".021")).unwrap();
        s2.proposals.push(stale);
        assert_eq!(s2.confirm(p).unwrap_err(), ConfirmError::Rejected(Rejection::ClaimRevisionChanged));
    }

    #[test]
    fn editing_a_later_claim_leaves_every_earlier_and_later_revision_unchanged() {
        let mut s = state();
        let loc = ClaimLocation { section_index: 1, section: SectionKind::Results, paragraph: 0, sentence: 2 };
        let third = s.add_claim(&UserClaim { location: loc, ..edited_claim(".01") }).unwrap();
        assert_eq!(third, ClaimId(2));
        let before: Vec<RevisionHash> = s.claims().iter().map(|c| c.revision.clone()).collect();
        s.edit_claim(ClaimId(1), &edited_claim(".019")).unwrap();
        let after: Vec<RevisionHash> = s.claims().iter().map(|c| c.revision.clone()).collect();
        assert_eq!(after[0], before[0]);
        assert_ne!(after[1], before[1]);
        assert_eq!(after[2], before[2]);
    }

    #[test]
    fn one_changed_byte_in_the_data_kills_every_token() {
        let mut s = state();
        let id = s.add_proposal(ClaimId(0), t_draft(), ProposalSource::Manual { edited_from: None }).unwrap();
        let (_, token) = s.confirm(id).unwrap();
        assert!(s.gate(&token, ClaimId(0)).is_ok());
        s.load_dataset(dataset(b"v2"));
        assert_eq!(s.gate(&token, ClaimId(0)).map(|_| ()), Err(GateRejection::Revoked(RevokedBecause::DatasetReplaced)));
        assert_eq!(s.live_tokens(), 0);
        assert!(s.proposals().is_empty());
    }

    #[test]
    fn an_ai_suggestion_never_carries_a_stated_variant_and_an_edit_records_where_it_came_from() {
        let mut s = state();
        let draft = BindingDraft { variant: VariantChoice::user_ticked(Variant::T(TVariant::Welch)), ..t_draft() };
        let ai = s.add_proposal(ClaimId(0), draft, ProposalSource::AiSuggestion { task_id: "binding_suggestion".into(), request_id: "req_1".into() }).unwrap();
        let p = s.proposal(ai).unwrap();
        assert_eq!(p.binding.variant.variant(), Variant::T(TVariant::NotStated));
        assert_eq!(p.binding.variant.set_by(), VariantSetBy::NotStated);

        let edited = s.edit_proposal(ai, BindingDraft { variant: VariantChoice::user_ticked(Variant::T(TVariant::Welch)), ..t_draft() }).unwrap();
        assert_ne!(edited, ai);
        let e = s.proposal(edited).unwrap();
        assert!(matches!(&e.source, ProposalSource::Manual { edited_from: Some(from) } if matches!(**from, ProposalSource::AiSuggestion { .. })));
        assert_eq!(e.binding.variant.set_by(), VariantSetBy::UserTicked, "the user's own tick stands");
        assert!(s.proposal(ai).is_some(), "the original stays");
        let mut bad = t_draft();
        bad.method_id = "nope".into();
        assert_eq!(s.add_proposal(ClaimId(0), bad, ProposalSource::AiSuggestion { task_id: "t".into(), request_id: "r".into() }).unwrap_err(), ProposalError::MethodNotInCatalogue);
    }

    #[test]
    fn a_withdrawn_confirmation_closes_the_gate() {
        let mut s = state();
        let id = s.add_proposal(ClaimId(0), t_draft(), ProposalSource::Manual { edited_from: None }).unwrap();
        let (_, token) = s.confirm(id).unwrap();
        assert!(s.withdraw(&token));
        assert_eq!(s.gate(&token, ClaimId(0)).map(|_| ()), Err(GateRejection::Revoked(RevokedBecause::Withdrawn)));
        assert!(!s.withdraw(&token));
    }

    #[test]
    fn without_a_dataset_nothing_can_be_proposed_or_confirmed() {
        let mut s = L1RunState::new();
        let ex = extract_from_text(DOC);
        let reading = read_results(&ex, TableStructure::NotAvailable);
        s.set_claims(claims_from_reading(&ex, &reading).0);
        assert_eq!(s.propose_local(ClaimId(0)).unwrap_err(), ProposalError::NoDataset);
        assert_eq!(s.add_proposal(ClaimId(0), t_draft(), ProposalSource::Manual { edited_from: None }).unwrap_err(), ProposalError::NoDataset);
        assert_eq!(s.propose_local(ClaimId(9)).unwrap_err(), ProposalError::NoDataset);
        s.load_dataset(dataset(b"v1"));
        assert_eq!(s.propose_local(ClaimId(9)).unwrap_err(), ProposalError::UnknownClaim(ClaimId(9)));
    }

    impl BindingDraft {
        fn roles_set(mut self, role: RoleName, column: ColumnAlias) -> BindingDraft {
            for r in &mut self.roles {
                if r.role == role {
                    *r = RoleBinding { role, columns: vec![column] };
                }
            }
            self
        }
    }
}
