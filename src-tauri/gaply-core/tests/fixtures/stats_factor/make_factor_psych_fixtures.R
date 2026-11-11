# The psych tier for gaply_core::stats_factor (Research OS, Phase 9B-1, decision
# 701), beside make_factor_fixtures.R, compared by tests/factor_r_golden.rs.
#
# Run only as
#
#   /usr/local/bin/Rscript make_factor_psych_fixtures.R
#
# after make_factor_fixtures.R, whose data_*.csv files it reads; it writes
# psych_expected.tsv next to itself. psych is already in R's library (its
# version is in the file's header); nothing is installed. R and psych are the
# reference, run outside Gaply (ADR-008). Synthetic data only.
#
# What it gives, each against Gaply run with convergence 1e-10 and 1,000
# iterations:
#   - psych::KMO: the overall MSA and each item's;
#   - psych::cortest.bartlett: chi-square and p;
#   - psych::fa(fm = "pa", rotate = "none" and "varimax", max.iter = 1000,
#     min.err = 1e-13): the communalities and loadings. psych stops when the sum
#     of the communalities changes by less than min.err, not SPSS's largest
#     change; run tight, both reach the same fixed point. fa sorts factors by
#     their variance and sets signs by column sums (psych:::fac, printed in
#     this session); this script puts psych's factors back in stats::varimax's
#     order, matching each by its largest absolute correlation, and applies
#     decision 700's sign rule, before writing them.
#   - psych::Promax on fa's unrotated loadings. FIND FIRST of 9B-1 printed
#     psych::Promax: it sets GPA <- TRUE and calls GPArotation::Varimax(x),
#     whose default is no Kaiser normalisation, and its own `normalize` is never
#     passed on. So psych's Promax is promax without the row normalisation that
#     R's promax, SPSS and decision 700 use. This script writes psych's pattern
#     and factor correlations, and beside them the same steps in base R from
#     stats::varimax(x, normalize = FALSE) ("unnormalised"), to show that the
#     difference from Gaply is that normalisation and nothing else.
# requireNamespace("GPArotation") is recorded in the header: decision 701
# expected it absent; on this machine it is present.

here <- local({
  a <- commandArgs(trailingOnly = FALSE)
  f <- sub("^--file=", "", a[grep("^--file=", a)])
  if (length(f)) dirname(normalizePath(f)) else getwd()
})
setwd(here)
suppressPackageStartupMessages(library(psych))
gpa <- requireNamespace("GPArotation", quietly = TRUE)

g17 <- function(x) sprintf("%.17g", x)
rows <- list()
put <- function(case, key, i, j, v) rows[[length(rows) + 1L]] <<- data.frame(case = case, key = key, i = i, j = j, value = g17(v), stringsAsFactors = FALSE)

sign_flip <- function(v) {
  s <- sum(v)
  if (abs(s) > 1e-10 * sum(abs(v))) return(s < 0)
  k <- which(abs(v) > 1e-10)[1]
  !is.na(k) && v[k] < 0
}
signed <- function(x) { for (k in seq_len(ncol(x))) if (sign_flip(x[, k])) x[, k] <- -x[, k]; x }
# psych's columns put in the order of `reference`'s, each by its largest |r|.
matched <- function(x, reference) {
  m <- ncol(x); out <- x; used <- integer(0)
  for (k in seq_len(m)) {
    cand <- setdiff(seq_len(m), used)
    best <- cand[which.max(abs(cor(reference[, k], x[, cand, drop = FALSE])))]
    used <- c(used, best); out[, k] <- x[, best]
  }
  out
}
promax_steps <- function(a, t) {
  q <- a * abs(a)^3
  u <- lm.fit(a, q)$coefficients
  u <- u %*% diag(sqrt(diag(solve(t(u) %*% u))))
  w <- t %*% u
  list(pattern = a %*% u, phi = solve(w) %*% t(solve(w)))
}

for (spec in list(c("two_factor_6", 2), c("three_factor_12", 3), c("two_factor_20", 2))) {
  case <- spec[1]; m <- as.integer(spec[2])
  x <- as.matrix(read.csv(sprintf("data_%s.csv", case)))
  r <- cor(x); n <- nrow(x); p <- ncol(x)
  k <- KMO(r)
  put(case, "kmo", 0L, 0L, k$MSA)
  for (j in seq_len(p)) put(case, "msa", j - 1L, 0L, k$MSAi[j])
  b <- cortest.bartlett(r, n = n)
  put(case, "bartlett_chi", 0L, 0L, b$chisq); put(case, "bartlett_p", 0L, 0L, b$p.value)
  f0 <- fa(r, nfactors = m, fm = "pa", rotate = "none", max.iter = 1000, min.err = 1e-13, n.obs = n, warnings = FALSE)
  l0 <- signed(unclass(f0$loadings))
  for (j in seq_len(p)) { put(case, "comm_ext", j - 1L, 0L, f0$communality[j]); for (kk in seq_len(m)) put(case, "loading", j - 1L, kk - 1L, l0[j, kk]) }
  v <- varimax(l0)
  fv <- fa(r, nfactors = m, fm = "pa", rotate = "varimax", max.iter = 1000, min.err = 1e-13, n.obs = n, warnings = FALSE)
  lv <- signed(matched(unclass(fv$loadings), unclass(v$loadings)))
  for (j in seq_len(p)) for (kk in seq_len(m)) put(case, "rot_loading", j - 1L, kk - 1L, lv[j, kk])
  pm <- Promax(l0)
  pp <- unclass(pm$loadings); phi <- pm$Phi
  for (kk in seq_len(m)) if (sign_flip(pp[, kk])) { pp[, kk] <- -pp[, kk]; phi[kk, ] <- -phi[kk, ]; phi[, kk] <- -phi[, kk] }
  vu <- varimax(l0, normalize = FALSE)
  un <- promax_steps(unclass(vu$loadings), vu$rotmat)
  upat <- un$pattern; uphi <- un$phi
  for (kk in seq_len(m)) if (sign_flip(upat[, kk])) { upat[, kk] <- -upat[, kk]; uphi[kk, ] <- -uphi[kk, ]; uphi[, kk] <- -uphi[, kk] }
  ord <- sapply(seq_len(m), function(kk) which.max(abs(cor(upat[, kk], pp))))
  pp <- pp[, ord, drop = FALSE]; phi <- phi[ord, ord, drop = FALSE]
  for (j in seq_len(p)) for (kk in seq_len(m)) { put(case, "psych_promax_pattern", j - 1L, kk - 1L, pp[j, kk]); put(case, "unnormalised_pattern", j - 1L, kk - 1L, upat[j, kk]) }
  for (jj in seq_len(m)) for (kk in seq_len(m)) { put(case, "psych_promax_phi", jj - 1L, kk - 1L, phi[jj, kk]); put(case, "unnormalised_phi", jj - 1L, kk - 1L, uphi[jj, kk]) }
  cat(sprintf("%s: psych Promax vs unnormalised base-R promax, largest |difference| %.3e (pattern), %.3e (phi)\n",
              case, max(abs(pp - upat)), max(abs(phi - uphi))))
}
out <- do.call(rbind, rows)
writeLines(c(
  "# Written by /usr/local/bin/Rscript make_factor_psych_fixtures.R (decision 701).",
  sprintf("# %s; psych %s; requireNamespace(\"GPArotation\") %s.", R.version.string, as.character(packageVersion("psych")), gpa),
  "case\tkey\ti\tj\tvalue", do.call(paste, c(out, sep = "\t"))), "psych_expected.tsv")
cat("rows:", nrow(out), " GPArotation:", gpa, "\n")
