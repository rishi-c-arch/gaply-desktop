//! **The confirmation token: an opaque, random capability Rust issues when a
//! binding is confirmed, and the only thing that can reach a computation.**
//!
//! Tokens are held in an in-memory store, never persisted, and die with the
//! process. A token is bound to the dataset fingerprint, the claim revision,
//! the method id and version, the normalised binding and the engine version;
//! the store records those, and [`super::run_state::L1RunState::gate`] checks
//! each against the live run before a computation may start. A change to any
//! of them kills the token.
//!
//! # Entropy without a dependency
//!
//! `gaply_core` has no `rand`. The secret behind each store is derived from
//! `std::collections::hash_map::RandomState`, whose keys the standard library
//! seeds from the operating system's random source, folded through SHA-256
//! with a counter. A token is `SHA-256(secret ‖ counter ‖ binding digest)`,
//! hex. Nobody outside the process holds the secret, so no token can be
//! predicted; and whatever its structure, a token is accepted only if the
//! store issued it, so a forged one fails on lookup rather than on arithmetic.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use super::claim::{ClaimId, RevisionHash};
use super::dataset::sha256_hex;

/// Opaque. `Debug` prints no digits, so a token cannot reach a log through a
/// derived `Debug` on a type that holds one.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ConfirmationToken(String);

impl ConfirmationToken {
    /// The token as the webview will hold it. 64 hex characters.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// A token presented by a caller: whatever string they hold.
    pub fn presented(s: &str) -> ConfirmationToken {
        ConfirmationToken(s.to_string())
    }
}

impl std::fmt::Debug for ConfirmationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConfirmationToken(…)")
    }
}

/// What a token is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRecord {
    pub claim: ClaimId,
    pub claim_revision: RevisionHash,
    pub fingerprint_digest: String,
    pub method_id: String,
    pub method_version: &'static str,
    pub binding_digest: String,
    pub engine_version: &'static str,
}

/// Why a presented token is not live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRejection {
    /// Never issued by this store: forged, or from another process.
    Unknown,
    /// Issued and since revoked.
    Revoked(RevokedBecause),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevokedBecause {
    ClaimRevised,
    DatasetReplaced,
    Withdrawn,
}

pub struct TokenStore {
    secret: [u8; 32],
    counter: u64,
    live: HashMap<String, TokenRecord>,
    dead: HashMap<String, RevokedBecause>,
}

impl std::fmt::Debug for TokenStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TokenStore {{ live: {}, dead: {} }}", self.live.len(), self.dead.len())
    }
}

fn entropy() -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for i in 0..8u64 {
        let mut x = RandomState::new().build_hasher();
        x.write_u64(i);
        h.update(x.finish().to_le_bytes());
    }
    h.finalize().into()
}

impl Default for TokenStore {
    fn default() -> Self {
        TokenStore::new()
    }
}

impl TokenStore {
    pub fn new() -> TokenStore {
        TokenStore { secret: entropy(), counter: 0, live: HashMap::new(), dead: HashMap::new() }
    }

    pub fn issue(&mut self, record: TokenRecord) -> ConfirmationToken {
        self.counter += 1;
        let mut material = Vec::with_capacity(32 + 8 + record.binding_digest.len());
        material.extend_from_slice(&self.secret);
        material.extend_from_slice(&self.counter.to_le_bytes());
        material.extend_from_slice(record.binding_digest.as_bytes());
        let token = sha256_hex(&material);
        self.live.insert(token.clone(), record);
        ConfirmationToken(token)
    }

    pub fn lookup(&self, token: &ConfirmationToken) -> Result<&TokenRecord, TokenRejection> {
        if let Some(r) = self.live.get(&token.0) {
            return Ok(r);
        }
        match self.dead.get(&token.0) {
            Some(why) => Err(TokenRejection::Revoked(*why)),
            None => Err(TokenRejection::Unknown),
        }
    }

    /// Kill every token whose record satisfies `pred`.
    pub fn revoke_where(&mut self, because: RevokedBecause, pred: impl Fn(&TokenRecord) -> bool) -> usize {
        let doomed: Vec<String> = self.live.iter().filter(|(_, r)| pred(r)).map(|(t, _)| t.clone()).collect();
        for t in &doomed {
            self.live.remove(t);
            self.dead.insert(t.clone(), because);
        }
        doomed.len()
    }

    /// Kill one token.
    pub fn revoke(&mut self, token: &ConfirmationToken, because: RevokedBecause) -> bool {
        match self.live.remove(&token.0) {
            Some(_) => {
                self.dead.insert(token.0.clone(), because);
                true
            }
            None => false,
        }
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(claim: usize, binding: &str) -> TokenRecord {
        TokenRecord {
            claim: ClaimId(claim),
            claim_revision: RevisionHash::synthetic(b"rev"),
            fingerprint_digest: "fp".into(),
            method_id: "independent_t".into(),
            method_version: "1",
            binding_digest: binding.into(),
            engine_version: "e",
        }
    }

    #[test]
    fn tokens_are_sixty_four_hex_characters_distinct_and_unguessable_from_each_other() {
        let mut s = TokenStore::new();
        let a = s.issue(record(0, "b"));
        let b = s.issue(record(0, "b"));
        assert_eq!(a.as_str().len(), 64);
        assert!(a.as_str().chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "the same binding twice is two tokens");
        // Two stores never agree: the secret is per store.
        let mut t = TokenStore::new();
        assert_ne!(t.issue(record(0, "b")), a);
        assert_eq!(format!("{a:?}"), "ConfirmationToken(…)");
    }

    #[test]
    fn a_forged_token_is_unknown_and_a_revoked_one_says_why() {
        let mut s = TokenStore::new();
        let a = s.issue(record(0, "b"));
        let _b = s.issue(record(1, "c"));
        assert!(s.lookup(&a).is_ok());
        assert_eq!(s.lookup(&ConfirmationToken::presented(&"0".repeat(64))), Err(TokenRejection::Unknown));
        assert_eq!(s.lookup(&ConfirmationToken::presented("")), Err(TokenRejection::Unknown));
        assert_eq!(s.revoke_where(RevokedBecause::ClaimRevised, |r| r.claim == ClaimId(0)), 1);
        assert_eq!(s.lookup(&a), Err(TokenRejection::Revoked(RevokedBecause::ClaimRevised)));
        assert_eq!(s.live_count(), 1);
    }
}
