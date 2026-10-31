# Written for /usr/local/bin/Rscript (R 4.6.1): the q points of the
# studentized range's reference grid (Research OS, Phase 8G, decision 315).
# For df >= 2, q is R's qtukey at a target upper-tail p (placement only: the
# gold p at that q is mpmath's, make_srange_fixtures.py), and R's own
# ptukey upper tail at q is recorded beside it for comparison. R's ptukey
# and qtukey return NaN below df 2 (measured: ptukey(3, 3, 1.5) and
# qtukey(0.95, 3, 1.5) are NaN), so df 1, 1.25 and 1.5 take a fixed q grid
# and NA for R. Run: /usr/local/bin/Rscript make_srange_placement.R > srange_placement.tsv
ks <- c(2, 3, 5, 10, 20, 50)
dfs <- c("1", "1.25", "1.5", "2", "3", "5", "10", "30", "120", "1000", "1e5", "1e7", "inf")
ps <- c(0.5, 0.1, 0.05, 0.01, 1e-3, 1e-5, 1e-8, 1e-10)
fixed_q <- c(2, 4, 8, 20, 100, 1e3, 1e5, 1e8)
cat("# written by: /usr/local/bin/Rscript make_srange_placement.R (", R.version.string, ")\n", sep = "")
cat("q\tk\tdf\tr_ptukey_upper\tp_target\n")
for (k in ks) for (d in dfs) {
  nu <- if (d == "inf") Inf else as.numeric(d)
  if (nu < 2) {
    for (q in fixed_q) cat(sprintf("%.12g\t%d\t%s\tNA\tNA\n", q, k, d))
  } else {
    for (p in ps) {
      q <- suppressWarnings(qtukey(p, k, nu, lower.tail = FALSE))
      if (!is.finite(q)) next
      q <- signif(q, 12)
      rp <- suppressWarnings(ptukey(q, k, nu, lower.tail = FALSE))
      cat(sprintf("%.12g\t%d\t%s\t%.17g\t%.17g\n", q, k, d, rp, p))
    }
  }
}
