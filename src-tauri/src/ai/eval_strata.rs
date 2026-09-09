//! Stratified estimation for the advisory lane's measured rates (§11 D126).
//!
//! # WHY A POOLED RATE IS THE WRONG NUMBER HERE
//!
//! The audit judges two very different populations of sentence. In PRIOR-WORK
//! sections — introduction, related work, theory — "does this need a citation?"
//! is a real question. In OWN-WORK sections — the authors' methods, results,
//! ablations, discussion, conclusion — the answer is structurally *no*: it is
//! their own hardware, their own split, their own numbers.
//!
//! Measured across the two manuscripts labelled so far, the population is
//! **68% own-work**. A labelled set that is 14% own-work therefore describes
//! mostly the half where citations are genuinely in question, and a rate pooled
//! over it over-claims — which is exactly how "43% precision" came to be
//! printed (§11 D123).
//!
//! The fix is not a bigger sample of the same shape. It is to sample each
//! stratum deliberately and re-weight to the population:
//!
//! ```text
//! f_h  = N_h / n_h                 (population size / sample size, per stratum)
//! TP   = Σ f_h · tp_h              FP = Σ f_h · fp_h      FN = Σ f_h · fn_h
//! precision = TP / (TP + FP)       recall = TP / (TP + FN)
//! ```
//!
//! # THE FAILURE THIS MODULE EXISTS TO MAKE IMPOSSIBLE
//!
//! **A stratified estimate that is computed and then reported UNWEIGHTED looks
//! rigorous and is the same over-claim.** It would pass any floor check — the
//! strata are populated, the counts are real — while the published figure is
//! still the pooled one. So [`pooled_precision_pct`] exists beside
//! [`stratified_precision_pct`] not as an alternative but as the CONTRAST the
//! guard prints, and `a_pooled_rate_over_claims_on_an_unbalanced_sample` pins
//! the gap between them.

/// One stratum's labelled counts, and how much of the population it stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stratum {
    /// `prior_work` / `own_work` — the half, as declared in
    /// `evals/population_mix.json` and read the same way on both sides.
    pub name: String,
    /// Sentences in this stratum that the AUDIT judges, across every source
    /// document. Measured by `label-cn` from a real pre-pass — never typed.
    pub population: usize,
    pub tp: u32,
    pub fp: u32,
    pub fn_: u32,
    pub tn: u32,
}

impl Stratum {
    /// Labelled cases in this stratum.
    pub fn sample(&self) -> u32 {
        self.tp + self.fp + self.fn_ + self.tn
    }

    /// How many population sentences each labelled case stands for.
    ///
    /// `None` when nothing was labelled here: a stratum with population and no
    /// sample cannot be weighted, and silently treating it as zero would drop a
    /// whole half of the document out of the estimate.
    pub fn scale(&self) -> Option<f64> {
        (self.sample() > 0).then(|| self.population as f64 / self.sample() as f64)
    }
}

/// Population-weighted precision, as a percentage.
///
/// `None` if any stratum that has population has no sample — see
/// [`Stratum::scale`].
pub fn stratified_precision_pct(strata: &[Stratum]) -> Option<u32> {
    let (tp, fp, _) = weighted_totals(strata)?;
    ratio_pct(tp, tp + fp)
}

/// Population-weighted recall, as a percentage.
pub fn stratified_recall_pct(strata: &[Stratum]) -> Option<u32> {
    let (tp, _, fn_) = weighted_totals(strata)?;
    ratio_pct(tp, tp + fn_)
}

/// UNWEIGHTED precision over the pooled sample — the number that must never be
/// printed from an unbalanced set. Present so the guard can show both.
pub fn pooled_precision_pct(strata: &[Stratum]) -> Option<u32> {
    let tp: u32 = strata.iter().map(|s| s.tp).sum();
    let fp: u32 = strata.iter().map(|s| s.fp).sum();
    ratio_pct(tp as f64, (tp + fp) as f64)
}

/// UNWEIGHTED recall over the pooled sample.
pub fn pooled_recall_pct(strata: &[Stratum]) -> Option<u32> {
    let tp: u32 = strata.iter().map(|s| s.tp).sum();
    let fn_: u32 = strata.iter().map(|s| s.fn_).sum();
    ratio_pct(tp as f64, (tp + fn_) as f64)
}

fn weighted_totals(strata: &[Stratum]) -> Option<(f64, f64, f64)> {
    let mut totals = (0.0, 0.0, 0.0);
    for s in strata {
        if s.population == 0 && s.sample() == 0 {
            continue;
        }
        let f = s.scale()?;
        totals.0 += f * s.tp as f64;
        totals.1 += f * s.fp as f64;
        totals.2 += f * s.fn_ as f64;
    }
    Some(totals)
}

fn ratio_pct(num: f64, den: f64) -> Option<u32> {
    (den > 0.0).then(|| (100.0 * num / den).round() as u32)
}

/// The population's share of each stratum, for display — "weighted 68/32".
pub fn population_shares(strata: &[Stratum]) -> Vec<(String, u32)> {
    let total: usize = strata.iter().map(|s| s.population).sum();
    strata
        .iter()
        .map(|s| {
            let pct = if total == 0 {
                0
            } else {
                (100.0 * s.population as f64 / total as f64).round() as u32
            };
            (s.name.clone(), pct)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(name: &str, population: usize, tp: u32, fp: u32, fn_: u32, tn: u32) -> Stratum {
        Stratum { name: name.into(), population, tp, fp, fn_, tn }
    }

    /// THE REQUIREMENT, PINNED.
    ///
    /// Same counts, two ways of combining them. The sample is 42 prior-work and
    /// 15 own-work; the population is the other way round (68% own-work). The
    /// model is good on prior-work sentences and poor on own-work ones, which
    /// is the real pattern — so pooling flatters it, and the gap is not a
    /// rounding difference.
    ///
    /// A "stratified" estimate reported unweighted would print the pooled
    /// figure while the strata are fully populated: every floor check passes
    /// and the number is still the over-claim of §11 D123.
    #[test]
    fn a_pooled_rate_over_claims_on_an_unbalanced_sample() {
        let strata = [
            // prior-work: 12 of 16 flagged were right — 75%.
            s("prior_work", 111, 12, 4, 3, 23),
            // own-work: 2 of 12 flagged were right — 17%.
            s("own_work", 236, 2, 10, 1, 2),
        ];
        let pooled = pooled_precision_pct(&strata).unwrap();
        let weighted = stratified_precision_pct(&strata).unwrap();
        // 14 of 28 flagged, pooled. Weighted: prior scales by 111/42 = 2.64,
        // own by 236/15 = 15.73, so the stratum the model is BAD at carries
        // six times the weight per case and the estimate more than halves.
        assert_eq!(pooled, 50, "pooled precision");
        assert_eq!(weighted, 27, "population-weighted precision");
        assert!(
            pooled > weighted + 10,
            "the whole point: pooling an unbalanced sample over-claims by \
             {}pp, and both numbers come from the SAME per-stratum counts",
            pooled - weighted
        );
    }

    /// Weighting is what carries the population in; identical strata make the
    /// two agree, so a passing guard on a balanced set proves nothing about
    /// whether the weights were applied. (Which is why the test above exists.)
    #[test]
    fn weighting_is_a_no_op_only_when_the_sample_matches_the_population() {
        let balanced =
            [s("prior_work", 100, 5, 5, 0, 10), s("own_work", 100, 5, 5, 0, 10)];
        assert_eq!(
            pooled_precision_pct(&balanced).unwrap(),
            stratified_precision_pct(&balanced).unwrap()
        );
    }

    /// A stratum with population and no sample cannot be weighted. Returning
    /// `None` refuses; treating it as zero would silently drop that half.
    #[test]
    fn a_stratum_with_no_sample_refuses_rather_than_dropping_a_half() {
        let strata = [s("prior_work", 111, 12, 4, 3, 23), s("own_work", 236, 0, 0, 0, 0)];
        assert_eq!(stratified_precision_pct(&strata), None);
        assert_eq!(stratified_recall_pct(&strata), None);
        // Pooling does NOT refuse — it just answers about the half it has,
        // which is the trap.
        assert!(pooled_precision_pct(&strata).is_some());
    }

    #[test]
    fn recall_is_weighted_the_same_way() {
        let strata = [s("prior_work", 100, 8, 0, 2, 0), s("own_work", 300, 1, 0, 9, 0)];
        // prior: f=10 -> tp 80, fn 20.  own: f=30 -> tp 30, fn 270.
        // recall = 110 / 400 = 28%. Pooled would be 9/20 = 45%.
        assert_eq!(stratified_recall_pct(&strata).unwrap(), 28);
        assert_eq!(pooled_recall_pct(&strata).unwrap(), 45);
    }

    #[test]
    fn shares_render_the_weights_that_were_applied() {
        let strata = [s("prior_work", 111, 1, 0, 0, 0), s("own_work", 236, 1, 0, 0, 0)];
        assert_eq!(
            population_shares(&strata),
            vec![("prior_work".to_string(), 32), ("own_work".to_string(), 68)]
        );
    }
}
