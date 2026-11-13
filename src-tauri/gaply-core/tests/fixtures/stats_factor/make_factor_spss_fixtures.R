# SPSS's own factor solutions, for src/stats_factor/spss_tier.rs (Research OS,
# Phase 9B-2, decisions 718 and 719).
#
# Run only as
#
#   R_LIBS=~/gaply-corpus/os/phase9/9b2/rlib /usr/local/bin/Rscript make_factor_spss_fixtures.R [ALL_FILE]
#
# It writes spss_solutions.tsv next to itself: the solutions SPSS 23 and 27 gave
# for EFAtools 1.1.0's four simulated test models (EFAtools' data SPSS_23 and
# SPSS_27, entries baseline, case_1a, case_6b and case_11b; Grieder and Steiner,
# Behavior Research Methods 54, 54-74, 2022, doi 10.3758/s13428-021-01581-x;
# EFAtools 1.1.0, GPL-3),
# eight solutions in all: SPSS's unrotated principal axis loadings (paf_load),
# its varimax (var_load), its promax pattern (pro_load) and factor correlations
# (pro_phi), and the varimax cycles that reproduce var_load. EFAtools' other
# solutions come from real studies (IDS-2, WJIV, DOSPERT, NEO-PI-R) and are not
# committed; with ALL_FILE given, all 17 are written there too, for a check run
# in the session only.
#
# The cycles: each paf_load rotated by decision 715's cycles, with no stopping
# rule, for 1 to 30 cycles, in base R with atan2, sin and cos (IBM's FACTOR
# algorithms, 14.0, pp. 10-12); the count is the one whose loadings, reflected
# and sorted (steps f and g), are within 1e-13 of var_load. The script stops
# unless exactly one count does, and unless it is the first cycle whose change
# in SV (not divided by p^2) is at most 1e-4 (decision 718). EFAtools is used
# for its data only. R and SPSS's output are the reference, run outside Gaply
# (ADR-008).
here <- local({
  a <- commandArgs(trailingOnly = FALSE)
  f <- sub("^--file=", "", a[grep("^--file=", a)])
  if (length(f)) dirname(normalizePath(f)) else getwd()
})
all_file <- commandArgs(trailingOnly = TRUE)[1]
setwd(here)
stopifnot(as.character(packageVersion("EFAtools")) == "1.1.0")
data(SPSS_23, package = "EFAtools"); data(SPSS_27, package = "EFAtools")
g17 <- function(x) sprintf("%.17g", x)

sv <- function(z) { p <- nrow(z); s <- 0; for (j in seq_len(ncol(z))) { q <- z[, j] * z[, j]; s <- s + (p * sum(q * q) - sum(q) * sum(q)) }; s }
cycle <- function(z) {
  p <- nrow(z); m <- ncol(z)
  for (j in 1:(m - 1)) for (k in (j + 1):m) {
    u <- z[, j] * z[, j] - z[, k] * z[, k]; v <- 2 * z[, j] * z[, k]
    A <- sum(u); B <- sum(v); C <- sum(u * u - v * v); D <- sum(2 * u * v)
    P <- 0.25 * atan2(D - 2 * A * B / p, C - (A * A - B * B) / p)
    if (abs(sin(P)) > 1e-15) { zj <- z[, j]; zk <- z[, k]; z[, j] <- zj * cos(P) + zk * sin(P); z[, k] <- -zj * sin(P) + zk * cos(P) }
  }
  z
}
finish <- function(z, w) {
  l <- z * w; l <- sweep(l, 2, ifelse(colSums(l) < 0, -1, 1), `*`)
  l[, order(-colSums(l * l), seq_len(ncol(l))), drop = FALSE]
}
counts <- function(paf, var) {
  w <- sqrt(rowSums(paf * paf)); w[w < 1e-15] <- 1
  z <- paf / w; s0 <- sv(z); err <- numeric(30); dsv <- numeric(30)
  for (c in 1:30) { z <- cycle(z); s1 <- sv(z); dsv[c] <- abs(s1 - s0); s0 <- s1; err[c] <- max(abs(finish(z, w) - var)) }
  c(match = if (sum(err <= 1e-13) == 1) which(err <= 1e-13) else NA, rule = which(dsv <= 1e-4)[1])
}

rows <- function(name, s) {
  paf <- unname(s$paf_load); var <- unname(s$var_load)
  k <- counts(paf, var)
  if (is.na(k[["match"]]) || k[["match"]] != k[["rule"]]) stop(sprintf("%s: cycles %s (match) and %s (rule)", name, k[["match"]], k[["rule"]]))
  out <- data.frame(solution = name, key = "cycles", i = 0L, j = 0L, value = as.character(k[["match"]]))
  for (key in c("paf_load", "var_load", "pro_load", "pro_phi")) {
    x <- unname(s[[key]])
    for (i in seq_len(nrow(x))) for (j in seq_len(ncol(x)))
      out <- rbind(out, data.frame(solution = name, key = key, i = i - 1L, j = j - 1L, value = g17(x[i, j])))
  }
  out
}
simulated <- c("baseline", "case_1a", "case_6b", "case_11b")
solutions <- c(setNames(SPSS_23, paste0("SPSS_23.", names(SPSS_23))), setNames(SPSS_27, paste0("SPSS_27.", names(SPSS_27))))
keep <- sub("^SPSS_2[37]\\.", "", names(solutions)) %in% simulated
# The data's licence beside its provenance (decision 728): EFAtools' own
# DESCRIPTION, which must say GPL-3.
licence <- packageDescription("EFAtools")$License
stopifnot(identical(licence, "GPL-3"))
hdr <- c("# Written by /usr/local/bin/Rscript make_factor_spss_fixtures.R (decisions 718 and 719).",
         sprintf("# %s; EFAtools %s's data SPSS_23 and SPSS_27 (SPSS 23 and 27 output; Grieder and Steiner, 2022; EFAtools %s, %s).",
                 R.version.string, as.character(packageVersion("EFAtools")), as.character(packageVersion("EFAtools")), licence),
         "solution\tkey\ti\tj\tvalue")
write_set <- function(names, path) {
  out <- do.call(rbind, lapply(names, function(n) rows(n, solutions[[n]])))
  writeLines(c(hdr, do.call(paste, c(out, sep = "\t"))), path)
  cat(sprintf("%s: %d solutions, %d rows\n", path, length(names), nrow(out)))
}
write_set(names(solutions)[keep], "spss_solutions.tsv")
if (!is.na(all_file)) write_set(names(solutions), all_file)
