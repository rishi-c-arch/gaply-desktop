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
#   - psych::fa(fm = "pa", rotate = "none", max.iter = 1000, min.err = 1e-13):
#     the communalities and the unrotated loadings, decision 699's sign rule
#     applied. psych stops when the sum of the communalities changes by less
#     than min.err, not SPSS's largest change; run tight, both reach the same
#     fixed point.
# psych's rotations follow R's varimax and an unnormalised promax, not SPSS's,
# so since 9B-2 this tier has no rotation rows (decision 719); the rotations are
# checked against EFAtools' SPSS route (make_factor_fixtures.R) and SPSS's own
# solutions (src/stats_factor/spss_tier.rs). requireNamespace("GPArotation")
# is still recorded in the header.

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
}
out <- do.call(rbind, rows)
writeLines(c(
  "# Written by /usr/local/bin/Rscript make_factor_psych_fixtures.R (decisions 701 and 719).",
  sprintf("# %s; psych %s; requireNamespace(\"GPArotation\") %s.", R.version.string, as.character(packageVersion("psych")), gpa),
  "case\tkey\ti\tj\tvalue", do.call(paste, c(out, sep = "\t"))), "psych_expected.tsv")
cat("rows:", nrow(out), " GPArotation:", gpa, "\n")
