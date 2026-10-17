//! **Closed words: the trait every code enum and every state enum of the
//! contracts implements, and the run-level codes.**
//!
//! A closed word is a fixed snake_case string on the wire. A code carries
//! its namespace: `run.unknown_run`, `stats.desc.no_values`. Each enum is
//! declared through [`closed!`], which writes the variants, their words and
//! `ALL` from ONE table, so no variant can exist without a word or be left
//! out of `ALL`, and a test pins each enum's full list. The words are never
//! derived from `Debug`: a Rust rename must not change what reaches a screen
//! or an export. An unknown word is refused on the way in, and the refusal
//! does not echo it.
//!
//! Only run-level codes live here. Each capability brings its own code enum
//! in its own module (`stats_desc::DescCode`), so this module names no
//! domain.

use serde::de::{Deserialize, Deserializer, Error as _};
use serde::ser::Serializer;

/// A closed set of wire words.
pub trait Closed: Copy + Eq + Ord + 'static {
    /// The prefix every word of the set starts with, its dot included
    /// (`run.`); empty for a set of plain words such as the run states.
    const NAMESPACE: &'static str;
    /// Every member, in declaration order.
    const ALL: &'static [Self];
    /// The member's word on the wire.
    fn wire(self) -> &'static str;
    /// The member a wire word names, if any.
    fn from_wire(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.wire() == word)
    }
}

/// `Serialize` for a [`Closed`] set: its word.
pub fn serialize_word<C: Closed, S: Serializer>(word: &C, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(word.wire())
}

/// `Deserialize` for a [`Closed`] set: a word of the set, or a refusal that
/// does not repeat what it was given.
pub fn deserialize_word<'de, C: Closed, D: Deserializer<'de>>(d: D) -> Result<C, D::Error> {
    let word = String::deserialize(d)?;
    C::from_wire(&word).ok_or_else(|| D::Error::custom("not a word of this closed set"))
}

/// Declares a [`Closed`] enum from one table of `Variant => "word"` lines.
/// The namespace is prepended to every word.
macro_rules! closed {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident in $ns:literal {
            $( $(#[$vmeta:meta])* $variant:ident => $word:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $vis enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $crate::contracts::codes::Closed for $name {
            const NAMESPACE: &'static str = $ns;
            const ALL: &'static [Self] = &[ $( $name::$variant, )+ ];
            fn wire(self) -> &'static str {
                match self {
                    $( $name::$variant => concat!($ns, $word), )+
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                $crate::contracts::codes::serialize_word(self, s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                $crate::contracts::codes::deserialize_word(d)
            }
        }
    };
}
pub(crate) use closed;

closed! {
    /// Why the run manager refused a request about a run, or how a run ended
    /// without a result. Phase 2B's run manager returns these; `transition`
    /// returns the last two.
    pub enum RunCode in "run." {
        /// No run has that id: never started, or evicted.
        UnknownRun => "unknown_run",
        /// A foreground run is already going.
        RunBusy => "run_busy",
        /// The user cancelled the run.
        Cancelled => "cancelled",
        /// The run passed its time ceiling.
        TimedOut => "timed_out",
        /// The run passed a resource budget.
        ResourceLimit => "resource_limit",
        /// The event has no edge from the run's state.
        IllegalTransition => "illegal_transition",
        /// The run has ended; nothing leaves a terminal state.
        AlreadyTerminal => "already_terminal",
    }
}
