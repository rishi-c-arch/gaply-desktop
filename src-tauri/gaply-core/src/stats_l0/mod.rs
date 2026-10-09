//! Level 0 of the Statistical Analysis Verifier: can the numbers a manuscript
//! reports all be right, judged from the manuscript alone?
//!
//! No data file, no model, no proxy, no network, no persistence. This is not
//! [`crate::consistency`], which is CITATION consistency; nothing here reuses
//! its names.
//!
//! Every reported number enters as a [`number::ReportedNumber`]: the interval
//! its printed digits allow, read from the printed string and never through a
//! float parser.

pub mod number;
pub mod result;
