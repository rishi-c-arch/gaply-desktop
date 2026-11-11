# The R golden fixtures for gaply_core::stats_factor (Research OS, Phase 9B-1,
# decision 701), compared by tests/factor_r_golden.rs.
#
# Run only as
#
#   /usr/local/bin/Rscript make_factor_fixtures.R
#
# from anywhere: it writes, next to itself, cases.tsv, expected.tsv and the
# data_*.csv files. Base R only: cor, eigen, solve, chol, determinant(log =
# TRUE), pchisq, varimax and promax, with promax's factor correlations as
# solve(U) %*% t(solve(U)) on promax's rotmat. R is the reference, run outside
# Gaply (ADR-008): nothing of it is copied into Gaply. Synthetic data only; no
# manuscript or data of anyone's is read.
#
# Written by a different route from Rust's: the inverse by solve (LU), not a
# Cholesky factor; KMO, the MSAs and Bartlett's test from it; principal axis
# factoring by an R loop of its own on SPSS's rule (the largest change in a
# communality below the convergence); the sums of squares and percents.
#
# The rules of decisions 699 and 700 are applied to R's results, as Gaply
# applies them to its own:
#   - each eigenvector's sign: its sum made positive when |sum| is above 1e-10
#     times its 1-norm, otherwise its first entry above 1e-10 in size;
#   - after a rotation, each factor whose loadings' sum is below 0 (the same
#     rule) reversed, with its transformation and structure columns and its row
#     and column of the factor correlations;
#   - the Cholesky rule: the correlations are singular when R's chol stops, or
#     when some L_jj^2 is at most 1e-9 (each a_jj is 1).
#
# Two cases are built by an integer formula identical in R and Rust
# (formula_data below), so their data are not written: items_50 and items_128.

here <- local({
  a <- commandArgs(trailingOnly = FALSE)
  f <- sub("^--file=", "", a[grep("^--file=", a)])
  if (length(f)) dirname(normalizePath(f)) else getwd()
})
setwd(here)
set.seed(20260701)

g17 <- function(x) sprintf("%.17g", x)
expected <- list()
put <- function(case, key, i, j, v) {
  expected[[length(expected) + 1L]] <<- data.frame(case = case, key = key, i = i, j = j,
    value = if (is.character(v)) v else g17(v), stringsAsFactors = FALSE)
}
cases <- list()

sign_flip <- function(v) {
  s <- sum(v)
  if (abs(s) > 1e-10 * sum(abs(v))) return(s < 0)
  k <- which(abs(v) > 1e-10)[1]
  !is.na(k) && v[k] < 0
}
signed_vectors <- function(vectors) {
  for (k in seq_len(ncol(vectors))) if (sign_flip(vectors[, k])) vectors[, k] <- -vectors[, k]
  vectors
}

singular_at <- function(r) {
  ch <- tryCatch(chol(r), error = function(e) conditionMessage(e))
  if (is.character(ch)) return(as.integer(sub(".*order ([0-9]+).*", "\\1", ch)))
  bad <- which(diag(ch)^2 <= 1e-9 * diag(r))
  if (length(bad)) bad[1] else 0L
}

# The integer formula shared with Rust: K factors, item j on factor j %% K,
# every value an integer. n rows, p items.
formula_data <- function(n, p, K) {
  i <- seq_len(n)
  x <- matrix(0, n, p)
  for (j in seq_len(p)) {
    f <- (j - 1) %% K
    score <- ((i * 7907 + f * 15485863) %% 1013) - 506
    noise <- ((i * 104729 + j * 7919 + i * j * 31) %% 997) - 498
    x[, j] <- 2 * score + noise
  }
  x
}

write_data <- function(name, x) {
  header <- paste0("x", seq_len(ncol(x)))
  lines <- apply(x, 1, function(row) paste(ifelse(is.na(row), "", g17(row)), collapse = ","))
  writeLines(c(paste(header, collapse = ","), lines), name)
}

# One case: x the data (NA a missing cell), the spec's choices, and what R gives.
run_case <- function(case, x, extraction, factors, rotation, iterations = 25L, convergence = 0.001,
                     codes = numeric(0), data = NULL, refusal = "", note = "") {
  p <- ncol(x)
  if (is.null(data)) {
    data <- sprintf("data_%s.csv", case)
    write_data(data, x)
  }
  row <- function(expect, condition) data.frame(case = case, data = data, extraction = extraction,
    factors = if (is.na(factors)) "kaiser" else as.character(factors), rotation = rotation,
    iterations = iterations, convergence = g17(convergence),
    codes = if (length(codes)) paste(g17(codes), collapse = ";") else "", expect = expect,
    condition = g17(condition), note = note, stringsAsFactors = FALSE)
  if (nzchar(refusal)) {
    cases[[length(cases) + 1L]] <<- row(refusal, NA)
    return(invisible())
  }
  y <- x
  for (cd in codes) y[!is.na(y) & y == cd] <- NA
  y <- y[stats::complete.cases(y), , drop = FALSE]
  n <- nrow(y)
  r <- cor(y)
  e <- eigen(r, symmetric = TRUE)
  values <- e$values
  vectors <- signed_vectors(e$vectors)
  condition <- max(values) / max(min(values), .Machine$double.xmin)
  put(case, "rows_used", 0L, 0L, n)
  for (j in seq_len(p)) {
    put(case, "mean", j - 1L, 0L, mean(y[, j]))
    put(case, "sd", j - 1L, 0L, sd(y[, j]))
  }
  sing <- singular_at(r)
  if (sing == 0) {
    d <- determinant(r, logarithm = TRUE)
    put(case, "det", 0L, 0L, if (exp(d$modulus) < .Machine$double.xmin) "NC:too_small" else exp(as.numeric(d$modulus)))
    put(case, "logdet", 0L, 0L, as.numeric(d$modulus))
    inv <- solve(r)
    q <- -inv / sqrt(outer(diag(inv), diag(inv)))
    r2 <- r^2; diag(r2) <- 0
    q2 <- q^2; diag(q2) <- 0
    put(case, "kmo", 0L, 0L, sum(r2) / (sum(r2) + sum(q2)))
    for (j in seq_len(p)) put(case, "msa", j - 1L, 0L, sum(r2[j, ]) / (sum(r2[j, ]) + sum(q2[j, ])))
    chi <- -(n - 1 - (2 * p + 5) / 6) * min(as.numeric(d$modulus), 0)
    put(case, "bartlett_chi", 0L, 0L, chi)
    put(case, "bartlett_p", 0L, 0L, pchisq(chi, p * (p - 1) / 2, lower.tail = FALSE))
    smc <- 1 - 1 / diag(inv)
  } else {
    for (k in c("det", "logdet", "kmo", "bartlett_chi", "bartlett_p")) put(case, k, 0L, 0L, "NC:correlations_singular")
    for (j in seq_len(p)) put(case, "msa", j - 1L, 0L, "NC:correlations_singular")
  }
  put(case, "bartlett_df", 0L, 0L, p * (p - 1) / 2)
  cum <- 0
  for (k in seq_len(p)) {
    cum <- cum + values[k] / p * 100
    put(case, "eval", 0L, k - 1L, values[k]); put(case, "eval_pct", 0L, k - 1L, values[k] / p * 100); put(case, "eval_cum", 0L, k - 1L, cum)
  }
  m <- if (is.na(factors)) sum(values > 1) else factors
  if (m == 0) {
    cases[[length(cases) + 1L]] <<- row("no_eigenvalue_over_one", condition)
    return(invisible())
  }
  notes <- character(0)
  if (extraction == "principal_components") {
    if (any(values[1:m] <= 0)) { cases[[length(cases) + 1L]] <<- row("extraction_eigenvalue_not_positive", condition); return(invisible()) }
    loadings <- vectors[, 1:m, drop = FALSE] %*% diag(sqrt(values[1:m]), m)
    initial <- rep(1, p)
    final_values <- values
    iterations_used <- 0L
  } else {
    if (sing != 0) { cases[[length(cases) + 1L]] <<- row(sprintf("correlations_singular:%d", sing), condition); return(invisible()) }
    h <- smc
    initial <- smc
    done <- FALSE
    for (it in seq_len(iterations)) {
      rr <- r; diag(rr) <- h
      ee <- eigen(rr, symmetric = TRUE)
      if (any(ee$values[1:m] <= 0)) { cases[[length(cases) + 1L]] <<- row("extraction_eigenvalue_not_positive", condition); return(invisible()) }
      vv <- signed_vectors(ee$vectors)
      loadings <- vv[, 1:m, drop = FALSE] %*% diag(sqrt(ee$values[1:m]), m)
      nh <- rowSums(loadings^2)
      if (any(nh > 1)) notes <- union(notes, "communality_above_one")
      change <- max(abs(nh - h))
      h <- nh
      if (change < convergence) { done <- TRUE; iterations_used <- it; break }
    }
    if (!done) { cases[[length(cases) + 1L]] <<- row(sprintf("extraction_not_converged:%d", iterations), condition); return(invisible()) }
    final_values <- ee$values
  }
  if (m < p) {
    a <- final_values[m]; b <- final_values[m + 1]
    if (abs(a - b) <= 1e-8 * max(abs(a), abs(b))) notes <- union(notes, "eigenvalues_not_distinct")
  }
  put(case, "m", 0L, 0L, m)
  put(case, "iterations", 0L, 0L, iterations_used)
  comm <- rowSums(loadings^2)
  for (j in seq_len(p)) { put(case, "comm_init", j - 1L, 0L, initial[j]); put(case, "comm_ext", j - 1L, 0L, comm[j]) }
  ss <- colSums(loadings^2); cum <- 0
  for (k in seq_len(m)) {
    cum <- cum + ss[k] / p * 100
    put(case, "ext_ss", 0L, k - 1L, ss[k]); put(case, "ext_pct", 0L, k - 1L, ss[k] / p * 100); put(case, "ext_cum", 0L, k - 1L, cum)
  }
  for (j in seq_len(p)) for (k in seq_len(m)) put(case, "loading", j - 1L, k - 1L, loadings[j, k])
  if (rotation != "none" && m == 1) notes <- union(notes, "one_factor_not_rotated")
  if (rotation != "none" && m >= 2) {
    without <- which(apply(loadings, 1, function(v) all(v == 0)))
    if (length(without)) {
      got <- tryCatch({ v <- varimax(loadings); if (any(!is.finite(v$loadings))) "NaN" else "numbers" }, error = function(e) "error")
      cases[[length(cases) + 1L]] <<- row(sprintf("rotation_item_without_loadings:%d", without[1]), condition)
      put(case, "r_varimax_gives", 0L, 0L, got)
      return(invisible())
    }
    v <- varimax(loadings)
    if (rotation == "varimax") {
      a <- unclass(v$loadings); t <- v$rotmat
      for (k in seq_len(m)) if (sign_flip(a[, k])) { a[, k] <- -a[, k]; t[, k] <- -t[, k] }
      for (j in seq_len(p)) for (k in seq_len(m)) put(case, "rot_loading", j - 1L, k - 1L, a[j, k])
      for (j in seq_len(m)) for (k in seq_len(m)) put(case, "rot_t", j - 1L, k - 1L, t[j, k])
      ss <- colSums(a^2); cum <- 0
      for (k in seq_len(m)) {
        cum <- cum + ss[k] / p * 100
        put(case, "rot_ss", 0L, k - 1L, ss[k]); put(case, "rot_pct", 0L, k - 1L, ss[k] / p * 100); put(case, "rot_cum", 0L, k - 1L, cum)
      }
    } else {
      pm <- promax(loadings)
      pattern <- unclass(pm$loadings); w <- pm$rotmat
      phi <- solve(w) %*% t(solve(w))
      for (k in seq_len(m)) if (sign_flip(pattern[, k])) {
        pattern[, k] <- -pattern[, k]; w[, k] <- -w[, k]; phi[k, ] <- -phi[k, ]; phi[, k] <- -phi[, k]
      }
      structure <- pattern %*% phi
      for (j in seq_len(p)) for (k in seq_len(m)) { put(case, "rot_loading", j - 1L, k - 1L, pattern[j, k]); put(case, "structure", j - 1L, k - 1L, structure[j, k]) }
      for (j in seq_len(m)) for (k in seq_len(m)) put(case, "phi", j - 1L, k - 1L, phi[j, k])
    }
  }
  put(case, "notes", 0L, 0L, if (length(notes)) paste(sort(notes), collapse = ";") else "-")
  cases[[length(cases) + 1L]] <<- row("ok", condition)
}

# Structures with K factors: item j on factor (j - 1) %% K at loading lam.
structured <- function(n, p, K, lam = 0.7) {
  f <- matrix(rnorm(n * K), n, K)
  x <- matrix(0, n, p)
  for (j in seq_len(p)) x[, j] <- lam * f[, (j - 1) %% K + 1] + sqrt(1 - lam^2) * rnorm(n)
  round(x, 6)
}

two6 <- structured(200, 6, 2)
three12 <- structured(300, 12, 3)
two20 <- structured(500, 20, 2, 0.6)
for (spec in list(list("two_factor_6", two6), list("three_factor_12", three12), list("two_factor_20", two20))) {
  nm <- spec[[1]]; x <- spec[[2]]; d <- sprintf("data_%s.csv", nm); write_data(d, x)
  for (ex in c("principal_components", "principal_axis")) {
    short <- if (ex == "principal_components") "pc" else "pa"
    run_case(sprintf("%s_%s_kaiser_varimax", nm, short), x, ex, NA, "varimax", data = d)
    run_case(sprintf("%s_%s_kaiser_promax", nm, short), x, ex, NA, "promax", data = d)
    run_case(sprintf("%s_%s_fixed_none", nm, short), x, ex, 2L, "none", data = d)
  }
}
three <- structured(100, 3, 1, 0.8)
run_case("three_items_pc", three, "principal_components", NA, "none")
run_case("three_items_pa", three, "principal_axis", 1L, "none", data = "data_three_items_pc.csv")
run_case("items_50", formula_data(2000, 50, 5), "principal_components", 5L, "varimax", data = "formula:2000:50:5")
run_case("items_128", formula_data(1000, 128, 8), "principal_axis", 8L, "promax", data = "formula:1000:128:8")
# Near-singular, within the Cholesky rule.
b <- matrix(rnorm(300 * 3), 300, 3)
near <- round(cbind(b, b[, 1] + b[, 2] + rnorm(300, sd = 1e-3)), 9)
run_case("near_singular_pa", near, "principal_axis", 2L, "varimax", iterations = 100L)
# An item equal to the sum of two others, exactly (integers).
s <- matrix(sample(-20:20, 200 * 3, replace = TRUE), 200, 3)
sum5 <- cbind(s, s[, 1] + s[, 2])
run_case("sum_item_pc", sum5, "principal_components", 2L, "none")
run_case("sum_item_pa", sum5, "principal_axis", 2L, "none", data = "data_sum_item_pc.csv")
# Exactly uncorrelated items: three columns of a Hadamard matrix of order 8.
had <- cbind(c(1, -1, 1, -1, 1, -1, 1, -1), c(1, 1, -1, -1, 1, 1, -1, -1), c(1, 1, 1, 1, -1, -1, -1, -1))
run_case("uncorrelated_kaiser", had, "principal_components", NA, "none")
run_case("uncorrelated_tied_note", had, "principal_components", 1L, "none", data = "data_uncorrelated_kaiser.csv")
# An item exactly uncorrelated with every other: 32 rows twice, the fifth item
# +1 then -1, every mean a multiple of 1/64.
base <- matrix(sample(1:9, 32 * 4, replace = TRUE), 32, 4)
base[, 2] <- base[, 1] + sample(-1:1, 32, replace = TRUE)
base[, 4] <- base[, 3] + sample(-1:1, 32, replace = TRUE)
alone <- cbind(rbind(base, base), c(rep(1, 32), rep(-1, 32)))
run_case("uncorrelated_item_varimax", alone, "principal_components", 2L, "varimax")
# A Heywood case: few rows, high loadings; the first seed whose R loop has a
# communality above 1 and converges.
for (sd0 in 1:2000) {
  set.seed(sd0)
  f <- rnorm(12)
  hx <- round(cbind(f + rnorm(12, sd = 0.1), f + rnorm(12, sd = 0.15), f + rnorm(12, sd = 0.9), rnorm(12)), 6)
  ok <- tryCatch({
    r <- cor(hx); h <- 1 - 1 / diag(solve(r)); above <- FALSE; conv <- FALSE
    for (it in 1:25) { rr <- r; diag(rr) <- h; ee <- eigen(rr, symmetric = TRUE)
      if (any(ee$values[1:2] <= 0)) break
      L <- ee$vectors[, 1:2] %*% diag(sqrt(ee$values[1:2])); nh <- rowSums(L^2)
      above <- above || any(nh > 1); ch <- max(abs(nh - h)); h <- nh
      if (ch < 0.001) { conv <- TRUE; break } }
    above && conv }, error = function(e) FALSE)
  if (ok) break
}
cat("heywood seed", sd0, "\n")
set.seed(20260701 + 1)
run_case("heywood_pa", hx, "principal_axis", 2L, "none", note = sprintf("seed %d", sd0))
# One factor with a rotation asked.
run_case("one_factor_rotation", two6, "principal_components", 1L, "varimax", data = "data_two_factor_6.csv")
# Missing values and codes.
mx <- structured(150, 6, 2)
mx[sample(150, 10), 2] <- NA
mx[sample(150, 6), 4] <- -99
mx[sample(150, 3), 1] <- -99
run_case("missing_and_codes", mx, "principal_axis", NA, "promax", codes = -99)
# Items out of order: refused before any arithmetic.
run_case("items_out_of_order", two6, "principal_components", NA, "none", data = "data_two_factor_6.csv", refusal = "items_out_of_order")
# Extraction not converged at iterations 2.
run_case("not_converged_pa", three12, "principal_axis", 3L, "none", iterations = 2L, convergence = 1e-12, data = "data_three_factor_12.csv")

ex <- do.call(rbind, expected)
cs <- do.call(rbind, cases)
hdr <- c("# Written by /usr/local/bin/Rscript make_factor_fixtures.R (decision 701).",
         paste0("# ", R.version.string, "; seed 20260701; base R only."))
writeLines(c(hdr, "case\tkey\ti\tj\tvalue", do.call(paste, c(ex, sep = "\t"))), "expected.tsv")
writeLines(c(hdr, paste(names(cs), collapse = "\t"), do.call(paste, c(cs, sep = "\t"))), "cases.tsv")
cat("cases:", nrow(cs), " expected rows:", nrow(ex), "\n")
print(cs[, c("case", "expect", "condition")], row.names = FALSE)
