//! **What the SPSS reader noted about a `.sav` file, as closed codes
//! (Research OS, Phase 8I, decision 365).**
//!
//! ambers 0.4.6 reads a file that contradicts itself from its more reliable
//! part and lists each contradiction as an English sentence in
//! `SpssMetadata::warnings`. Those sentences can name a variable, so none of
//! them reaches a screen, a log or a payload (decision 310): the app maps
//! each to one of these codes with numeric parameters
//! (`supplementary::sav_findings_of`) and keeps nothing of the text. A
//! sentence no pattern matches is counted under
//! [`SavFindingCode::UnrecognisedFinding`], so an ambers upgrade that
//! rewords one shows as that code and turns the mapping test red.
//!
//! The header's case count is not taken from ambers: it drops that finding
//! whenever a row limit is set, and Gaply always sets one, so the app
//! derives [`SavFindingCode::HeaderRowsMismatch`] itself.
//!
//! Pure: no file, clock, network or model. The words are pinned by this
//! module's tests and each code's sentence by the screen's vitest.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::contracts::codes::closed;

closed! {
    /// One thing the SPSS reader noted about a file. Never a verdict on the
    /// data: each says what the file declares and what Gaply read instead.
    pub enum SavFindingCode in "sav." {
        /// The header's case count is not the number of rows read, and the
        /// read did not stop at the row limit (`declared`, `read`).
        HeaderRowsMismatch => "header_rows_mismatch",
        /// The header's values per case are not the number the variables
        /// define (`declared`, `used`).
        CaseSizeMismatch => "case_size_mismatch",
        /// The header gives 0 or -1 values per case (`used`).
        CaseSizeMissing => "case_size_missing",
        /// Date or time values outside the years 1 to 9999, read as missing
        /// (`count`).
        DatesOutOfRange => "dates_out_of_range",
        /// A variable record with a negative type other than -1, read as
        /// part of the variable before it (`record`, its position from 1).
        TypeContinuation => "type_continuation",
        /// The header's compression bias, other than 100 (`bias`). ambers
        /// decodes compressed whole numbers with it, where SPSS assumes 100.
        CompressionBias => "compression_bias",
        /// A `.zsav` trailer's compression bias, other than 100 (`bias`).
        /// Decoding uses the header's, so it changes nothing read
        /// (8I-2, decision 376).
        ZsavTrailerBias => "zsav_trailer_bias",
        /// Very-long-string declarations ambers ignored, each variable then
        /// read at the width of its own record, at most 255 bytes (`count`).
        VeryLongStringIgnored => "very_long_string_ignored",
        /// Findings no pattern matches (`count`).
        UnrecognisedFinding => "unrecognised_finding",
    }
}

closed! {
    /// The name of a finding's parameter.
    pub enum SavParam in "" {
        Declared => "declared",
        Read => "read",
        Used => "used",
        Count => "count",
        Record => "record",
        Bias => "bias",
    }
}

/// A finding: its code and its numbers, never text.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SavFinding {
    pub code: SavFindingCode,
    pub params: BTreeMap<SavParam, f64>,
}

impl SavFinding {
    /// A finding with the parameters given, each a finite number.
    pub fn new(code: SavFindingCode, params: &[(SavParam, f64)]) -> SavFinding {
        SavFinding {
            code,
            params: params.iter().copied().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::codes::Closed;

    #[test]
    fn the_words_are_pinned() {
        let words: Vec<&str> = SavFindingCode::ALL.iter().map(|c| c.wire()).collect();
        assert_eq!(
            words,
            [
                "sav.header_rows_mismatch",
                "sav.case_size_mismatch",
                "sav.case_size_missing",
                "sav.dates_out_of_range",
                "sav.type_continuation",
                "sav.compression_bias",
                "sav.zsav_trailer_bias",
                "sav.very_long_string_ignored",
                "sav.unrecognised_finding",
            ]
        );
        let params: Vec<&str> = SavParam::ALL.iter().map(|c| c.wire()).collect();
        assert_eq!(
            params,
            ["declared", "read", "used", "count", "record", "bias"]
        );
    }

    #[test]
    fn a_finding_serialises_as_its_code_and_numbers() {
        let f = SavFinding::new(
            SavFindingCode::HeaderRowsMismatch,
            &[(SavParam::Read, 20.0), (SavParam::Declared, 0.0)],
        );
        assert_eq!(
            serde_json::to_string(&f).unwrap(),
            r#"{"code":"sav.header_rows_mismatch","params":{"declared":0.0,"read":20.0}}"#
        );
    }
}
