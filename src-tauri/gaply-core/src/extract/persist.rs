//! Persist an [`ExtractionResult`] into the `extractions` and `findings`
//! tables (schema from the migrations in Prompt 3).

use rusqlite::params;
use serde::Serialize;

use crate::db::Database;
use crate::error::GaplyError;
use crate::extract::citations::Reference;
use crate::extract::stats::Stat;
use crate::extract::{ExtractionResult, StatClaim};

#[derive(Debug, Serialize)]
pub struct StoreReport {
    pub extraction_id: i64,
    pub findings: usize,
}

fn describe_stat(claim: &StatClaim) -> (String, String) {
    let loc = &claim.location;
    let where_ = format!("{:?}¶{}", loc.section, loc.paragraph + 1);
    let msg = match &claim.stat {
        Stat::PValue { operator, value, .. } => format!("p-value p {operator} {value} ({where_})"),
        Stat::ConfidenceInterval { level, low, high, .. } => {
            let lvl = level.map(|l| format!("{l}% ")).unwrap_or_default();
            format!("{lvl}CI [{low}, {high}] ({where_})")
        }
        Stat::SampleSize { n, .. } => format!("sample size n = {n} ({where_})"),
        Stat::Test { name, .. } => format!("statistical test: {name} ({where_})"),
        Stat::TestStatistic { name, value, df, .. } => {
            let dfs: Vec<String> = df.iter().map(|d| d.to_string()).collect();
            format!("{name}({}) = {value} ({where_})", dfs.join(", "))
        }
        Stat::EffectSize { name, value, .. } => format!("effect size: {name} = {value} ({where_})"),
    };
    ("statistic".to_string(), msg)
}

/// Store the full result as one `extractions` row (JSON), then record
/// per-claim `findings`: each statistical claim (severity `info`), and each
/// reference missing a DOI (severity `warning`).
pub fn store_extraction(
    db: &Database,
    manuscript_id: i64,
    result: &ExtractionResult,
) -> Result<StoreReport, GaplyError> {
    let json = serde_json::to_string(result)
        .map_err(|e| GaplyError::Internal(format!("serialize extraction: {e}")))?;

    let extraction_id = {
        let conn = db.conn()?;
        conn.execute(
            "INSERT INTO extractions (manuscript_id, kind, content, created_at)
             VALUES (?1, 'extraction_result', ?2, ?3)",
            params![manuscript_id, json, crate::now_epoch()],
        )?;
        conn.last_insert_rowid()
    };

    let mut findings = 0usize;
    for claim in &result.statistics {
        let (category, message) = describe_stat(claim);
        insert_finding(db, manuscript_id, extraction_id, "info", &category, &message)?;
        findings += 1;
    }
    for r in result.references.iter().filter(|r: &&Reference| r.doi.is_none()) {
        let msg = format!(
            "reference missing DOI: {} ({})",
            r.authors,
            r.year.map(|y| y.to_string()).unwrap_or_else(|| "n.d.".into())
        );
        insert_finding(db, manuscript_id, extraction_id, "warning", "reference", &msg)?;
        findings += 1;
    }

    tracing::info!(manuscript_id, extraction_id, findings, "extraction stored");
    Ok(StoreReport { extraction_id, findings })
}

fn insert_finding(
    db: &Database,
    manuscript_id: i64,
    extraction_id: i64,
    severity: &str,
    category: &str,
    message: &str,
) -> Result<(), GaplyError> {
    db.conn()?.execute(
        "INSERT INTO findings (manuscript_id, extraction_id, severity, category, message, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![manuscript_id, extraction_id, severity, category, message, crate::now_epoch()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;

    const SAMPLE: &str = "\
A Study

Abstract
A randomized trial (n = 120) found an effect (p < 0.001).

Results
A paired t-test was used (p = 0.02).

References
Smith, J. (2023). A paper without a doi. Journal, 1(1), 2-3.
";

    #[test]
    fn store_writes_extraction_and_findings() {
        let db = Database::in_memory().unwrap();
        let manuscript_id = db.create_manuscript("A Study", "", "").unwrap();
        let result = extract_from_text(SAMPLE);

        let report = store_extraction(&db, manuscript_id, &result).unwrap();
        assert!(report.extraction_id > 0);
        // stats: p<0.001, n=120, p=0.02, t-test = 4; plus 1 missing-DOI reference
        assert_eq!(report.findings, result.statistics.len() + 1);

        let conn = db.conn().unwrap();
        let extraction_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM extractions WHERE manuscript_id = ?1 AND kind = 'extraction_result'",
                [manuscript_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(extraction_count, 1);

        let warn_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM findings WHERE severity = 'warning' AND category = 'reference'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(warn_count, 1);

        // findings link back to the extraction row
        let linked: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM findings WHERE extraction_id = ?1",
                [report.extraction_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(linked as usize, report.findings);
    }
}
