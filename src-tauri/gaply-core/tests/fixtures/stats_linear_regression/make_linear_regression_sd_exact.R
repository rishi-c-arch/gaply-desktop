# The exact values for two of gaply_core's linear regression cases (Research OS,
# Phase 9B-1, decision 698): a model column's standard deviation where its
# variance is subnormal (sd_from's scaled sum), and a predictor's VIF there (the
# VIF's branch for a subnormal variance). Written into linear_regression_cases.rs
# by hand from this script's output; nothing reads a file this script writes.
#
# Run by its full path, from anywhere, with CRAN's gmp on the library path
# R_LIBS names (never the system library):
#
#   R_LIBS=<a library holding gmp> /usr/local/bin/Rscript make_linear_regression_sd_exact.R
#
# R and gmp are the reference, run outside Gaply (ADR-008): nothing of either is
# copied into Gaply. On make_linear_regression_exact.R's pattern; that script and
# make_linear_regression_fixtures.R are not run by this one.
#
# The data, i = 1 to 100,000, built by the same integer formulas in Rust:
#   u_i = (7919 i) mod 1000 and w_i = (7907 i) mod 1013, both exact integers;
#   the small predictor is u_i times 2^-527, an exact double, so every exact
#   quantity of it is u's times a power of two: its sum of squares about its mean
#   is SS(u) 2^-1054, within a factor of 2 of the smallest normal double 2^-1022,
#   and its variance SS / (n - 1) is subnormal. A VIF does not change when a
#   column is scaled, so the small predictor's VIF beside w is u's.
# Each exact value is a rational (gmp's bigq), its square root found by Newton's
# method on integers (gmp has no square root of its own here), and each is
# rounded once to the nearest double by exact comparison among a double and its
# two neighbours, as make_linear_regression_exact.R does.
#
# Synthetic data only. No manuscript or data of anyone's is read.

suppressPackageStartupMessages(library(gmp))

cat("R:", R.version.string, "\n")
cat("gmp:", as.character(packageVersion("gmp")), "\n")
cat("Rscript: /usr/local/bin/Rscript\n")

n <- 100000L
i <- as.bigz(seq_len(n))
u <- (i * 7919L) %% 1000L
w <- (i * 7907L) %% 1013L

# The sum of squares about the mean, exactly.
ss <- function(x) {
  m <- as.bigq(sum(x)) / n
  sum((as.bigq(x) - m)^2)
}
sp <- function(x, y) {
  mx <- as.bigq(sum(x)) / n
  my <- as.bigq(sum(y)) / n
  sum((as.bigq(x) - mx) * (as.bigq(y) - my))
}

# The floor of the square root of a nonnegative integer, by Newton's method.
isqrt <- function(N) {
  if (N == 0) return(as.bigz(0))
  x <- as.bigz(2)^(as.integer(ceiling(sizeinbase(N, 2) / 2)) + 1L)
  repeat {
    y <- (x + N %/% x) %/% 2
    if (y >= x) return(x)
    x <- y
  }
}

# The square root of a positive rational to 2^-K relative, as a rational.
sqrt_q <- function(q, K = 400L) {
  num <- numerator(q)
  den <- denominator(q)
  # sqrt(num / den) = sqrt(num den) / den.
  s <- isqrt(num * den * as.bigz(2)^(2L * K))
  as.bigq(s) / (as.bigq(den) * as.bigq(as.bigz(2)^K))
}

neighbours <- function(x) {
  if (x == 0) return(c(-2^-1074, 0, 2^-1074))
  e <- floor(log2(abs(x)))
  ulp <- 2^(max(e, -1022) - 52)
  c(x - ulp, x, x + ulp)
}
nearest <- function(q, guess) {
  repeat {
    cand <- neighbours(guess)
    dq <- lapply(cand, function(c) abs(as.bigq(c) - q))
    best <- 1
    for (j in 2:3) if (dq[[j]] < dq[[best]]) best <- j
    for (j in 1:3) if (j != best && dq[[j]] == dq[[best]]) stop("a tie between two doubles")
    if (best == 2) return(cand[2])
    guess <- cand[best]
  }
}

ss_u <- ss(u)
ss_w <- ss(w)
sp_uw <- sp(u, w)
cat("SS(u) =", as.character(ss_u), "(100 x 1000 (1000^2 - 1) / 12 =", 100 * 1000 * (1000^2 - 1) / 12, ")\n")
stopifnot(ss_u == as.bigq(8333325000))

# The small predictor's exact SD: sd(u) 2^-527, the power of two exact.
sd_u <- sqrt_q(ss_u / (n - 1L))
sd_small_q <- sd_u / as.bigq(as.bigz(2)^527)
sd_small <- nearest(sd_small_q, as.double(sd_u) * 2^-527)
# The exact sum of squares as a double, to show where it lies.
ss_small <- as.double(ss_u) * 2^-1054
cat(sprintf("ss_small = %.17g (smallest normal %.17g, ratio %.6f)\n", ss_small, 2^-1022, ss_small / 2^-1022))
cat(sprintf("var_small = %.17g (subnormal: %s)\n", ss_small / (n - 1), ss_small / (n - 1) < 2^-1022))
cat(sprintf("sd_small_exact = %.17g\n", sd_small))

# The VIF of u beside w: 1 / (1 - r^2), r^2 = SP^2 / (SS(u) SS(w)), exactly.
r2 <- sp_uw^2 / (ss_u * ss_w)
vif_q <- 1 / (1 - r2)
vif <- nearest(vif_q, as.double(vif_q))
cat(sprintf("r2_uw = %.17g\n", as.double(r2)))
cat(sprintf("vif_small_exact = %.17g\n", vif))

# 9A-1's forms, in doubles, to show the error the decision expects (about 1e-11):
# the SD as the root of the subnormal variance, and the VIF as (n - 1) var s_j,
# where s_j = VIF / ((n - 1) var) in exact arithmetic.
var_d <- ss_small / (n - 1)
sd_9a1 <- sqrt(var_d)
cat(sprintf("sd_9a1_form = %.17g  relative error %.3e\n", sd_9a1, abs(sd_9a1 - sd_small) / sd_small))
