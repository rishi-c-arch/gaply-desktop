# The R reference for gaply_core::stats_linalg's inverse and pivoted_rank (Research OS,
# Phase 9C-4, decision 796): R's solve() on 40 seeded matrices, and qr(…, tol = 1e-9)$rank
# on 24 symmetric matrices of known rank, written for stats_linalg's golden test
# (src/stats_linalg/stats_linalg_r_golden.rs, its second section).
#
# Run only as
#
#   /usr/local/bin/Rscript make_sem_linalg_fixtures.R
#
# from anywhere: it writes sem_cases.tsv and sem_linalg.tsv next to itself, and leaves
# make_linalg_fixtures.R's cases.tsv and linalg.tsv alone. Base R only. R is the reference,
# run outside Gaply (ADR-008): nothing of it is copied into Gaply. Synthetic data only.
#
# The inverse's three families, each matrix and R's solve() of it written in full precision
# (%.17g), so Rust reads the same doubles:
#   - general: Gaussian entries;
#   - triangular: I - A with A strictly lower triangular, its rows and columns then shuffled
#     alike, as (I - A) is for a structural model without loops in some order of its
#     variables; no pivot is small, so no row exchange is needed;
#   - near_singular: U diag(s) V' with U and V random orthogonal and s from 1 down to 1e-6
#     of the largest, log-spaced, so the smallest singular value is 1e-6 of the largest.
# The rank's family: M = X X' of known rank r, X with Gaussian entries, scaled to a unit
# diagonal; "leading" cases repeat the first row of X as the second (and in some the third),
# so M's first columns are dependent and a Cholesky factor that takes the first remaining
# diagonal stops early, while one that takes the largest does not. R's qr(M, tol = 1e-9)$rank
# is the reference, and the script stops unless it equals r.

here <- local({
  a <- commandArgs(trailingOnly = FALSE)
  f <- sub("^--file=", "", a[grep("^--file=", a)])
  if (length(f)) dirname(normalizePath(f)) else getwd()
})
setwd(here)

set.seed(20260796)

g17 <- function(x) sprintf("%.17g", x)
rows <- list()
add_matrix <- function(case, item, m, lower = FALSE) {
  idx <- which(if (lower) lower.tri(m, diag = TRUE) else matrix(TRUE, nrow(m), ncol(m)), arr.ind = TRUE)
  idx <- idx[order(idx[, 1], idx[, 2]), , drop = FALSE]
  rows[[length(rows) + 1L]] <<- data.frame(case = case, item = item, i = idx[, 1] - 1L, j = idx[, 2] - 1L,
                                           value = g17(m[idx]), stringsAsFactors = FALSE)
}
cases <- list()
add_case <- function(case, family, n, measure, note) {
  cases[[length(cases) + 1L]] <<- data.frame(case = case, family = family, n = n, measure = g17(measure), note = note,
                                             stringsAsFactors = FALSE)
}
orthogonal <- function(n) qr.Q(qr(matrix(rnorm(n * n), n)))
condition <- function(a) { s <- svd(a)$d; max(s) / min(s) }

# ---- the inverse ----------------------------------------------------------------------------

for (n in c(2, 3, 4, 6, 8, 10, 13, 17, 22, 28, 35, 43, 52, 60)) {
  a <- matrix(rnorm(n * n), n)
  name <- sprintf("general_%d", n)
  add_case(name, "inverse", n, condition(a), "Gaussian entries")
  add_matrix(name, "A", a)
  add_matrix(name, "inv", solve(a))
}
for (n in c(2, 3, 5, 7, 9, 12, 16, 20, 26, 33, 41, 50, 60)) {
  a <- diag(n)
  for (i in 2:n) for (j in 1:(i - 1)) if (runif(1) < 0.4) a[i, j] <- -(runif(1) - 0.5)
  perm <- sample(n)
  a <- a[perm, perm]
  name <- sprintf("triangular_%d", n)
  add_case(name, "inverse", n, condition(a), "unit lower triangular, rows and columns shuffled alike")
  add_matrix(name, "A", a)
  add_matrix(name, "inv", solve(a))
}
for (n in c(2, 3, 4, 5, 7, 9, 12, 15, 20, 27, 35, 46, 60)) {
  s <- 10^seq(0, -6, length.out = n)
  a <- orthogonal(n) %*% diag(s) %*% t(orthogonal(n))
  name <- sprintf("near_singular_%d", n)
  add_case(name, "inverse", n, condition(a), "smallest singular value 1e-6 of the largest")
  add_matrix(name, "A", a)
  add_matrix(name, "inv", solve(a))
}

# ---- the rank -------------------------------------------------------------------------------

unit_diagonal <- function(m) { d <- 1 / sqrt(diag(m)); m * outer(d, d) }
rank_case <- function(name, n, r, repeated, note) {
  x <- matrix(rnorm(n * r), n, r)
  if (repeated >= 1) x[2, ] <- x[1, ]
  if (repeated >= 2) x[3, ] <- x[1, ]
  m <- unit_diagonal(x %*% t(x))
  m <- (m + t(m)) / 2
  diag(m) <- 1
  got <- qr(m, tol = 1e-9)$rank
  if (got != r) stop(sprintf("%s: qr's rank %d, built as %d", name, got, r))
  add_case(name, "rank", n, got, note)
  add_matrix(name, "M", m, lower = TRUE)
}
for (spec in list(c(2, 1), c(2, 2), c(3, 2), c(5, 5), c(8, 3), c(12, 12), c(20, 7), c(30, 29), c(45, 20), c(60, 60), c(60, 31), c(17, 1))) {
  rank_case(sprintf("generic_%d_%d", spec[1], spec[2]), spec[1], spec[2], 0, "X X' of known rank, scaled to a unit diagonal")
}
for (spec in list(c(3, 2, 1), c(4, 3, 1), c(6, 4, 1), c(9, 5, 2), c(14, 10, 1), c(20, 12, 2), c(27, 20, 1), c(33, 30, 2), c(40, 25, 1), c(50, 45, 2), c(60, 50, 1), c(60, 58, 2))) {
  rank_case(sprintf("leading_%d_%d", spec[1], spec[2]), spec[1], spec[2], spec[3],
            "X X' of known rank with its first rows of X repeated, so its first columns are dependent")
}

# ---- writing --------------------------------------------------------------------------------

header <- c("# Written by /usr/local/bin/Rscript make_sem_linalg_fixtures.R (decision 796).",
            paste0("# ", R.version.string, "; seed 20260796; base R only."))
con <- file("sem_cases.tsv", "w")
writeLines(header, con)
writeLines("# case\tfamily\tn\tmeasure (inverse: the condition number; rank: R's qr rank)\tnote", con)
all_cases <- do.call(rbind, cases)
for (k in seq_len(nrow(all_cases))) writeLines(paste(all_cases[k, ], collapse = "\t"), con)
close(con)
con <- file("sem_linalg.tsv", "w")
writeLines(header, con)
writeLines("# case\titem\ti\tj\tvalue (0-based i and j; A and inv in full, M as its lower triangle)", con)
all_rows <- do.call(rbind, rows)
writeLines(paste(all_rows$case, all_rows$item, all_rows$i, all_rows$j, all_rows$value, sep = "\t"), con)
close(con)
cat("cases", nrow(all_cases), "(inverse", sum(all_cases$family == "inverse"), ", rank", sum(all_cases$family == "rank"), "); rows", nrow(all_rows), "\n")
