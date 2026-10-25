//! **A delete's confirmation (decision 124), shared by the projects and the
//! Stored data section (Research OS, Phase 5D-1b, decision 213).**
//!
//! Extracted from `project_commands.rs`, unchanged in what it does: a
//! confirmation is 32 random bytes as a [`ConfirmationId`], issued for one
//! subject (a project, a Stored data category), used once, void after five
//! minutes or when a newer one is issued, and held only in memory, so a
//! restart voids it.
//!
//! Each user holds its own [`Confirmations`], so a confirmation issued for one
//! never voids another's: the projects' book is `ProjectSession`'s, and the
//! Stored data book is `StoredDataSession`'s. The book takes the time from
//! its caller, whose clock a test moves.

use std::collections::VecDeque;
use std::time::Instant;

use gaply_core::contracts::capability::ConfirmationId;
use gaply_core::research_project::hex_lower;

use crate::runs::Clock;

/// How long a delete confirmation stays good (decision 124).
pub const CONFIRMATION_LIFETIME_MS: u64 = 5 * 60 * 1000;

/// How many confirmations a book remembers, so a used or replaced one is
/// refused as such rather than as unknown.
pub const CONFIRMATIONS_REMEMBERED: usize = 16;

/// Milliseconds since the clock was made.
pub struct Monotonic(pub Instant);

impl Clock for Monotonic {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Pending,
    Used,
    Replaced,
}

/// One confirmation issued, in memory only.
struct Issued<S> {
    id: ConfirmationId,
    subject: S,
    issued_ms: u64,
    standing: Standing,
}

/// Why a confirmation was not good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Missing,
    Unknown,
    Used,
    Replaced,
    Expired,
    /// Issued for another subject.
    OtherSubject,
}

/// The system gave no random bytes, so no confirmation was issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoRandomness;

/// One user's confirmations, newest last, at most
/// [`CONFIRMATIONS_REMEMBERED`].
pub struct Confirmations<S> {
    book: VecDeque<Issued<S>>,
}

impl<S> Default for Confirmations<S> {
    fn default() -> Self {
        Confirmations {
            book: VecDeque::new(),
        }
    }
}

impl<S: PartialEq> Confirmations<S> {
    /// How many it remembers.
    pub fn len(&self) -> usize {
        self.book.len()
    }

    pub fn is_empty(&self) -> bool {
        self.book.is_empty()
    }

    /// A new confirmation for `subject` at `now_ms`; the one before it, for
    /// any subject of this book, is void from now on.
    pub fn issue(&mut self, subject: S, now_ms: u64) -> Result<ConfirmationId, NoRandomness> {
        let mut b = [0u8; 32];
        getrandom::getrandom(&mut b).map_err(|_| NoRandomness)?;
        let id = ConfirmationId::parse(&hex_lower(&b)).expect("64 lowercase hex characters");
        for i in self.book.iter_mut() {
            if i.standing == Standing::Pending {
                i.standing = Standing::Replaced;
            }
        }
        self.book.push_back(Issued {
            id: id.clone(),
            subject,
            issued_ms: now_ms,
            standing: Standing::Pending,
        });
        while self.book.len() > CONFIRMATIONS_REMEMBERED {
            self.book.pop_front();
        }
        Ok(id)
    }

    /// Uses the confirmation `given` for `subject` at `now_ms`: refused when
    /// there is none, when it is unknown, used, replaced, expired, or for
    /// another subject. Any confirmation found is used up by this call.
    pub fn redeem(&mut self, subject: &S, given: Option<&str>, now_ms: u64) -> Result<(), Refused> {
        let given = match given {
            None => return Err(Refused::Missing),
            Some(g) if g.is_empty() => return Err(Refused::Missing),
            Some(g) => g,
        };
        let Some(i) = self
            .book
            .iter_mut()
            .find(|i| same_bytes(i.id.as_str(), given))
        else {
            return Err(Refused::Unknown);
        };
        let standing = i.standing;
        i.standing = Standing::Used;
        match standing {
            Standing::Used => Err(Refused::Used),
            Standing::Replaced => Err(Refused::Replaced),
            Standing::Pending if now_ms.saturating_sub(i.issued_ms) >= CONFIRMATION_LIFETIME_MS => {
                Err(Refused::Expired)
            }
            Standing::Pending if &i.subject != subject => Err(Refused::OtherSubject),
            Standing::Pending => Ok(()),
        }
    }
}

/// Equal bytes, compared in time that does not depend on where they differ.
pub fn same_bytes(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
