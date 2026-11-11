# The R reference for gaply_core::stats_linalg (Research OS, Phase 9B-1,
# decision 699): R's eigen, chol, chol2inv, determinant(log = TRUE), svd and
# qr.solve on a fixed set of matrices, written for stats_linalg's golden test
# (src/stats_linalg/stats_linalg_r_golden.rs).
#
# Run only as
#
#   /usr/local/bin/Rscript make_linalg_fixtures.R
#
# from anywhere: it writes cases.tsv and linalg.tsv next to itself. Base R only.
# R is the reference, run outside Gaply (ADR-008): nothing of it is copied into
# Gaply. Synthetic data only; no manuscript or data of anyone's is read.
#
# Each matrix is written in full precision (%.17g), so Rust reads the same
# doubles. Symmetric inputs and inverses are written as their lower triangles.
# Decision 699's two rules are applied to R's results here, as stats_linalg
# applies them to its own:
#   - the sign rule: an eigenvector whose sum s has |s| above 1e-10 times its
#     1-norm is made to have s > 0; otherwise its first entry above 1e-10 in
#     size is made positive;
#   - the Cholesky rule: refused at the first j (1-based) where the value left
#     before the square root, L_jj^2 from R's chol, is at most 1e-9 times a_jj,
#     or where R's chol itself stops (its leading minor of order j).
#   A case whose smallest L_jj^2 / a_jj is within a factor of 2 of 1e-9 is
#   written as chol_borderline, with that ratio, and nothing else of Cholesky.

here <- local({
  a <- commandArgs(trailingOnly = FALSE)
  f <- sub("^--file=", "", a[grep("^--file=", a)])
  if (length(f)) dirname(normalizePath(f)) else getwd()
})
setwd(here)

set.seed(20260699)

g17 <- function(x) sprintf("%.17g", x)
rows <- list()
add <- function(case, item, i, j, v) {
  rows[[length(rows) + 1L]] <<- data.frame(case = case, item = item, i = i, j = j, value = g17(v), stringsAsFactors = FALSE)
}
add_matrix <- function(case, item, m, lower = FALSE) {
  idx <- which(if (lower) lower.tri(m, diag = TRUE) else matrix(TRUE, nrow(m), ncol(m)), arr.ind = TRUE)
  idx <- idx[order(idx[, 1], idx[, 2]), , drop = FALSE]
  rows[[length(rows) + 1L]] <<- data.frame(case = case, item = item, i = idx[, 1] - 1L, j = idx[, 2] - 1L,
                                           value = g17(m[idx]), stringsAsFactors = FALSE)
}
cases <- list()
add_case <- function(case, family, n, cond, note) {
  cases[[length(cases) + 1L]] <<- data.frame(case = case, family = family, n = n, condition = g17(cond), note = note,
                                             stringsAsFactors = FALSE)
}

sign_rule <- function(v) {
  s <- sum(v)
  if (abs(s) > 1e-10 * sum(abs(v))) {
    if (s < 0) v <- -v
  } else {
    k <- which(abs(v) > 1e-10)[1]
    if (!is.na(k) && v[k] < 0) v <- -v
  }
  v
}

# The Cholesky rule applied to R's chol: the 1-based index refused, or 0.
chol_rule <- function(a) {
  r <- tryCatch(chol(a), error = function(e) conditionMessage(e))
  if (is.character(r)) {
    k <- as.integer(sub(".*order ([0-9]+).*", "\\1", r))
    if (is.na(k)) stop(r)
    return(list(refused = k, l = NULL))
  }
  l <- t(r)
  left <- diag(l)^2
  bad <- which(left <= 1e-9 * diag(a))
  if (length(bad)) return(list(refused = bad[1], l = NULL))
  list(refused = 0L, l = l)
}

symmetric_case <- function(case, a, note) {
  a <- (a + t(a)) / 2
  n <- nrow(a)
  e <- eigen(a, symmetric = TRUE)
  cond <- if (min(abs(e$values)) > 0) max(abs(e$values)) / min(abs(e$values)) else Inf
  add_case(case, "symmetric", n, cond, note)
  add_matrix(case, "A", a, lower = TRUE)
  for (k in seq_len(n)) add(case, "eval", k - 1L, 0L, e$values[k])
  v <- e$vectors
  for (k in seq_len(n)) v[, k] <- sign_rule(v[, k])
  add_matrix(case, "evec", v)
  ch <- chol_rule(a)
  r <- tryCatch(chol(a), error = function(e) NULL)
  ratio <- if (is.null(r)) NA else min(diag(r)^2 / diag(a))
  if (!is.na(ratio) && ratio > 0.5e-9 && ratio < 2e-9) {
    # Within a factor of 2 of the rule: R and Rust may fall on either side, so
    # the golden counts the case and compares no Cholesky result for it.
    add(case, "chol_borderline", 0L, 0L, ratio)
  } else if (ch$refused > 0) {
    add(case, "chol_refused", ch$refused, 0L, ch$refused)
  } else {
    add_matrix(case, "chol", ch$l, lower = TRUE)
    add_matrix(case, "inv", chol2inv(t(ch$l)), lower = TRUE)
    d <- determinant(a, logarithm = TRUE)
    stopifnot(d$sign == 1)
    add(case, "logdet", 0L, 0L, as.numeric(d$modulus))
  }
}

random_cor <- function(n, p) cor(matrix(rnorm(n * p), n, p) %*% matrix(rnorm(p * p, sd = 0.5), p, p) + matrix(rnorm(n * p), n, p))
orth <- function(p) qr.Q(qr(matrix(rnorm(p * p), p, p)))
spectral <- function(values) {
  q <- orth(length(values))
  q %*% diag(values) %*% t(q)
}
hilbert <- function(p) outer(seq_len(p), seq_len(p), function(i, j) 1 / (i + j - 1))
contrasts <- function(p) {
  # Helmert-like columns: the first is all ones, the others sum to exactly 0.
  q <- matrix(0, p, p)
  q[, 1] <- 1 / sqrt(p)
  for (k in 2:p) {
    q[1:(k - 1), k] <- 1
    q[k, k] <- -(k - 1)
    q[, k] <- q[, k] / sqrt(sum(q[, k]^2))
  }
  q
}

# Correlation matrices of synthetic data, 3 to 128 items.
for (p in c(3L, 5L, 8L, 12L, 20L, 50L, 128L)) {
  symmetric_case(sprintf("cor_%d", p), random_cor(max(60L, 4L * p), p), "correlation of random data")
}
# Condition numbers 1e2, 1e6 and 1e10 by construction.
for (spec in list(c(6, 2), c(10, 6), c(8, 10))) {
  p <- spec[1]
  symmetric_case(sprintf("cond_1e%d_%d", spec[2], p), spectral(10^seq(0, -spec[2], length.out = p)), "spectral, geometric eigenvalues")
}
# A near-tie: two eigenvalues 1e-9 apart.
symmetric_case("near_tie_5", spectral(c(3, 2, 2 + 1e-9, 1, 0.5)), "two eigenvalues 1e-9 apart")
# Exact ties.
equi <- matrix(0.3, 6, 6); diag(equi) <- 1
symmetric_case("equicorrelation_6", equi, "eigenvalue 0.7 five times")
symmetric_case("identity_4", diag(4), "every off-diagonal 0")
symmetric_case("zero_pair_3", matrix(c(1, 0, 0.5, 0, 1, 0.5, 0.5, 0.5, 1), 3, 3), "a_12 = 0 and a_11 = a_22")
# Eigenvectors whose sums are 0 in exact arithmetic, distinct eigenvalues.
for (p in c(4L, 6L)) {
  q <- contrasts(p)
  symmetric_case(sprintf("contrast_%d", p), q %*% diag(seq(2.5, 0.2, length.out = p)) %*% t(q), "eigenvectors summing to 0 exactly")
}
symmetric_case("hilbert_6", hilbert(6L), "Hilbert")
symmetric_case("hilbert_8", hilbert(8L), "Hilbert")
# Singular: one column the sum of two others.
x <- matrix(rnorm(200 * 4), 200, 4)
symmetric_case("singular_5", cor(cbind(x, x[, 1] + x[, 2])), "column 5 = column 1 + column 2")
x <- matrix(rnorm(300 * 3), 300, 3)
symmetric_case("near_singular_4", cor(cbind(x, x[, 1] + x[, 2] + rnorm(300, sd = 1e-3))), "column 4 nearly column 1 + column 2")

# Polar factors of square matrices, by svd: U V' and the sum of the singular values.
polar_case <- function(case, b, note) {
  s <- svd(b)
  add_case(case, "polar", nrow(b), max(s$d) / min(s$d), note)
  add_matrix(case, "B", b)
  add_matrix(case, "polar", s$u %*% t(s$v))
  add(case, "sigma_sum", 0L, 0L, sum(s$d))
}
for (m in c(2L, 3L, 5L, 8L, 16L)) polar_case(sprintf("polar_%d", m), matrix(rnorm(m * m), m, m), "random normal")
polar_case("polar_negdet_3", diag(c(1, -2, 3)) %*% orth(3), "negative determinant")

# Least squares with several right-hand sides and no intercept, by qr.solve.
ls_case <- function(case, x, y, note) {
  b <- qr.solve(x, y)
  add_case(case, "least_squares", nrow(x), kappa(x, exact = TRUE), note)
  add_matrix(case, "X", x)
  add_matrix(case, "Y", y)
  add_matrix(case, "ls", b)
}
for (d in list(c(10, 3), c(50, 5), c(128, 8))) {
  x <- matrix(rnorm(d[1] * d[2]), d[1], d[2])
  ls_case(sprintf("ls_%dx%d", d[1], d[2]), x, x * abs(x)^3 + matrix(rnorm(d[1] * d[2], sd = 0.1), d[1], d[2]), "promax-like target")
}
x <- matrix(rnorm(20 * 3), 20, 3)
ls_case("ls_illcond_20x4", cbind(x, x[, 1] + x[, 2] + rnorm(20, sd = 1e-5)), matrix(rnorm(20 * 2), 20, 2), "column 4 nearly 1 + 2")

out <- do.call(rbind, rows)
cs <- do.call(rbind, cases)
header <- c(
  "# Written by /usr/local/bin/Rscript make_linalg_fixtures.R (decision 699).",
  paste0("# ", R.version.string, "; seed 20260699; base R only."),
  "# case\titem\ti\tj\tvalue (0-based i and j; symmetric inputs and inverses as lower triangles)"
)
# do.call(paste) rather than apply, which would pad the integer columns.
writeLines(c(header, do.call(paste, c(out, sep = "\t"))), "linalg.tsv")
writeLines(c(header[1:2], "# case\tfamily\tn\tcondition\tnote", do.call(paste, c(cs, sep = "\t"))), "cases.tsv")
cat("cases:", nrow(cs), " rows:", nrow(out), "\n")
print(cs[, c("case", "family", "n", "condition")], row.names = FALSE)
