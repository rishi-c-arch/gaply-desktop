# The R golden fixtures for gaply_core::stats_factor (Research OS, Phase 9B-1,
# decision 701; the rotations as SPSS computes them since Phase 9B-2, decisions
# 715 to 719), compared by tests/factor_r_golden.rs.
#
# Run only as
#
#   R_LIBS=~/gaply-corpus/os/phase9/9b2/rlib /usr/local/bin/Rscript make_factor_fixtures.R
#
# from anywhere: it writes, next to itself, cases.tsv, expected.tsv and the
# data_*.csv files. Base R (cor, eigen, solve, chol, determinant(log = TRUE),
# pchisq, qr) and EFAtools 1.1.0 from CRAN, installed outside R's own library
# (ADR-008; decision 718): its SPSS route, EFAtools:::.VARIMAX_SPSS,
# .reflect_and_order and .rotate_promax with the arguments .rotate_model passes
# for type = "SPSS", and .PAF with type = "SPSS". R and EFAtools are the
# reference, run outside Gaply: nothing of them is copied into Gaply. Synthetic
# data only; no manuscript or data of anyone's is read.
#
# Written by a different route from Rust's: the inverse by solve (LU), not a
# Cholesky factor; KMO, the MSAs and Bartlett's test from it; principal axis
# factoring by an R loop of its own on SPSS's rule (the largest change in a
# communality below the convergence, the absolute value of any of the m largest
# eigenvalues below 0, decision 717), checked against EFAtools' .PAF within
# 1e-10; the sums of squares and percents.
#
# The rotations' expected values are EFAtools' SPSS route, at precision 1e-4 / p^2,
# which is decision 715's criterion of 1e-4 on SV not divided by p^2. Each case is
# also rotated by a second route in base R written from IBM's FACTOR algorithms
# (14.0, pp. 10-12, 18-19) with atan2, sin and cos, and the script stops unless the
# two agree within 1e-12 with equal cycles; decision 716's promax (IBM's D and C)
# must give EFAtools' pattern and factor correlations within 1e-13.
#
# The rules of decisions 699 and 700 are applied to R's results, as Gaply
# applies them to its own:
#   - each eigenvector's sign: its sum made positive when |sum| is above 1e-10
#     times its 1-norm, otherwise its first entry above 1e-10 in size;
#   - the Cholesky rule: the correlations are singular when R's chol stops, or
#     when some L_jj^2 is at most 1e-9 (each a_jj is 1).
#
# Three cases are built by an integer formula identical in R and Rust
# (formula_data below), so their data are not written: items_50, items_128 and
# the review's formula(2000, 20, 2).
ef <- asNamespace("EFAtools")
stopifnot(as.character(packageVersion("EFAtools")) == "1.1.0")

# The second route (IBM, pp. 10-12): SV not divided by p^2; one cycle of
# Kaiser's pairwise rotations with atan2, sin and cos; the stop at |dSV| <= 1e-4
# within 1,000 cycles (Inf when not met); rows back by their weights; step f (a
# sum below 0) and step g (largest sum of squares first, ties in order).
ibm_sv <- function(z) { p <- nrow(z); s <- 0; for (j in seq_len(ncol(z))) { q <- z[, j] * z[, j]; s <- s + (p * sum(q * q) - sum(q) * sum(q)) }; s }
ibm_cycle <- function(z, t) {
  p <- nrow(z); m <- ncol(z)
  for (j in 1:(m - 1)) for (k in (j + 1):m) {
    u <- z[, j] * z[, j] - z[, k] * z[, k]; v <- 2 * z[, j] * z[, k]
    A <- sum(u); B <- sum(v); C <- sum(u * u - v * v); D <- sum(2 * u * v)
    P <- 0.25 * atan2(D - 2 * A * B / p, C - (A * A - B * B) / p)
    if (abs(sin(P)) > 1e-15) {
      cp <- cos(P); sp <- sin(P)
      zj <- z[, j]; zk <- z[, k]; z[, j] <- zj * cp + zk * sp; z[, k] <- -zj * sp + zk * cp
      tj <- t[, j]; tk <- t[, k]; t[, j] <- tj * cp + tk * sp; t[, k] <- -tj * sp + tk * cp
    }
  }
  list(z = z, t = t)
}
ibm_varimax <- function(L) {
  w <- sqrt(rowSums(L * L)); w[w < 1e-15] <- 1
  z <- L / w; t <- diag(ncol(L)); s0 <- ibm_sv(z); cycles <- Inf
  for (c in 1:1000) { r <- ibm_cycle(z, t); z <- r$z; t <- r$t; s1 <- ibm_sv(z)
    if (abs(s1 - s0) <= 1e-4) { cycles <- c; break }; s0 <- s1 }
  l <- z * w; s <- ifelse(colSums(l) < 0, -1, 1)
  raw_reflect <- any(s < 0)
  l <- sweep(l, 2, s, `*`); t <- sweep(t, 2, s, `*`)
  o <- order(-colSums(l * l), seq_len(ncol(l)))
  list(loadings = l[, o, drop = FALSE], t = t[, o, drop = FALSE], cycles = cycles,
       reflected = raw_reflect, reordered = !identical(o, seq_len(ncol(l))))
}
# Decision 716 (IBM, pp. 18-19): the row-normalised target, L by least squares,
# D, Q = L D, C = diag((Q'Q)^-1)^(-1/2), the pattern A Q C^-1, Phi = C (Q'Q)^-1 C.
ibm_promax <- function(a) {
  h <- sqrt(rowSums(a * a)); h[h < 1e-15] <- 1
  s <- a / h; target <- s * abs(s) * abs(s) * abs(s)
  L <- qr.coef(qr(a), target)
  Q <- L %*% diag(1 / sqrt(colSums(L * L)), ncol(L))
  QtQi <- solve(crossprod(Q))
  C <- diag(1 / sqrt(diag(QtQi)), ncol(L))
  pattern <- a %*% Q %*% solve(C); phi <- C %*% QtQi %*% C
  list(pattern = pattern, phi = phi, structure = pattern %*% phi)
}
gap <- function(a, b) max(abs(unclass(a) - unclass(b)))
# The worst gap of each check, printed at the end; and whether each rotated
# case's raw varimax was reflected (step f) or reordered (step g).
route_worst <- list(); route_gaps <- list()
track <- function(k, v) { route_worst[[k]] <<- max(c(route_worst[[k]], v)); v }

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
      # Decision 717: the absolute value of any of the m largest below 0, as SPSS.
      if (any(ee$values[1:m] < 0)) notes <- union(notes, "extraction_eigenvalue_negative")
      vv <- signed_vectors(ee$vectors)
      loadings <- vv[, 1:m, drop = FALSE] %*% diag(sqrt(abs(ee$values[1:m])), m)
      nh <- rowSums(loadings^2)
      if (any(nh > 1)) notes <- union(notes, "communality_above_one")
      change <- max(abs(nh - h))
      h <- nh
      if (change < convergence) { done <- TRUE; iterations_used <- it; break }
    }
    if (!done) { cases[[length(cases) + 1L]] <<- row(sprintf("extraction_not_converged:%d", iterations), condition); return(invisible()) }
    final_values <- ee$values
    # EFAtools' SPSS route for principal axis (decision 719): its loadings, after
    # the sign rule, and its communalities within 1e-10 of this loop's.
    pf <- suppressWarnings(ef$.PAF(r, n_factors = m, max_iter = iterations, type = "SPSS", criterion = convergence))
    lp <- signed_vectors(pf$L)
    if (pf$iter != iterations_used || track("paf_loadings", gap(lp, loadings)) > 1e-10 || track("paf_communalities", gap(pf$h2, rowSums(loadings^2))) > 1e-10)
      stop(sprintf("%s: EFAtools' .PAF differs: iterations %d against %d, loadings %.3e, communalities %.3e",
                   case, pf$iter, iterations_used, gap(lp, loadings), gap(pf$h2, rowSums(loadings^2))))
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
    # EFAtools' SPSS route (decision 719), at decision 715's criterion.
    precision <- 1e-4 / p^2
    vm <- suppressWarnings(ef$.VARIMAX_SPSS(loadings, normalize = TRUE, precision = precision))
    second <- ibm_varimax(loadings)
    if (is.finite(second$cycles) && second$cycles != vm$iter) stop(sprintf("%s: cycles %d (EFAtools) against %d (IBM's route)", case, vm$iter, second$cycles))
    if (second$cycles > iterations) {
      # Not met within the spec's iterations: a note, no rotation (decision 717).
      notes <- union(notes, "rotation_not_converged")
    } else if (rotation == "varimax") {
      ro <- ef$.reflect_and_order(vm$loadings, rotmat = vm$rotmat, L_unrot = loadings, order_type = "ss_factors")
      a <- unclass(ro$rot_loadings); t <- ro$rotmat
      if (track("varimax_loadings", gap(a, second$loadings)) > 1e-12 || track("varimax_t", gap(t, second$t)) > 1e-12)
        stop(sprintf("%s: varimax, EFAtools against IBM's route: loadings %.3e, T %.3e", case, gap(a, second$loadings), gap(t, second$t)))
      put(case, "rot_iterations", 0L, 0L, vm$iter)
      for (j in seq_len(p)) for (k in seq_len(m)) put(case, "rot_loading", j - 1L, k - 1L, a[j, k])
      for (j in seq_len(m)) for (k in seq_len(m)) put(case, "rot_t", j - 1L, k - 1L, t[j, k])
      ss <- colSums(a^2); cum <- 0
      for (k in seq_len(m)) {
        cum <- cum + ss[k] / p * 100
        put(case, "rot_ss", 0L, k - 1L, ss[k]); put(case, "rot_pct", 0L, k - 1L, ss[k] / p * 100); put(case, "rot_cum", 0L, k - 1L, cum)
      }
    } else {
      pm <- ef$.rotate_promax(loadings, TRUE, "norm", precision, "ss_factors", "kaiser", 4)
      pattern <- unclass(pm$rot_loadings); phi <- unclass(pm$Phi); structure <- unclass(pm$Structure)
      # Decision 716's D and C against EFAtools' scaling, on EFAtools' own varimax.
      base <- ef$.reflect_and_order(vm$loadings, rotmat = vm$rotmat, L_unrot = loadings, order_type = "ss_factors")
      dc <- ibm_promax(unclass(base$rot_loadings))
      if (track("dc_pattern", gap(dc$pattern, pattern)) > 1e-13 || track("dc_phi", gap(dc$phi, phi)) > 1e-13)
        stop(sprintf("%s: promax, IBM's D and C against EFAtools' scaling: pattern %.3e, phi %.3e", case, gap(dc$pattern, pattern), gap(dc$phi, phi)))
      two <- ibm_promax(second$loadings)
      if (track("promax_pattern", gap(two$pattern, pattern)) > 1e-12 || track("promax_phi", gap(two$phi, phi)) > 1e-12 || track("promax_structure", gap(two$structure, structure)) > 1e-12)
        stop(sprintf("%s: promax, EFAtools against IBM's route: pattern %.3e, phi %.3e", case, gap(two$pattern, pattern), gap(two$phi, phi)))
      put(case, "rot_iterations", 0L, 0L, vm$iter)
      for (j in seq_len(p)) for (k in seq_len(m)) { put(case, "rot_loading", j - 1L, k - 1L, pattern[j, k]); put(case, "structure", j - 1L, k - 1L, structure[j, k]) }
      for (j in seq_len(m)) for (k in seq_len(m)) put(case, "phi", j - 1L, k - 1L, phi[j, k])
    }
    route_gaps[[case]] <<- c(reflected = second$reflected, reordered = second$reordered)
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

# Decision 719's cases, after every earlier one, each with its own seed so no
# earlier case's draws move.
# The review's dataset (decision 713, M1): formula(2000, 20, 2) at three factors.
for (ex in c("principal_components", "principal_axis")) for (rot in c("varimax", "promax")) {
  short <- if (ex == "principal_components") "pc" else "pa"
  run_case(sprintf("review_m1_%s_%s", short, rot), formula_data(2000, 20, 2), ex, 3L, rot, data = "formula:2000:20:2")
}
# A structure whose SPSS varimax takes 3 cycles and whose raw factors are both
# reflected (step f) and reordered (step g), from loadings under the sign rule:
# seed 4 of ~/gaply-corpus/os/phase9/9b2/find_cases.R.
set.seed(4)
n1 <- sample(c(150, 300, 500), 1); p1 <- sample(6:16, 1); k1 <- sample(2:4, 1); l1 <- sample(c(0.5, 0.6, 0.7), 1)
sorted <- structured(n1, p1, k1, l1)
d_sorted <- "data_sorted_reflected.csv"; write_data(d_sorted, sorted)
run_case("sorted_reflected_pc_varimax", sorted, "principal_components", k1, "varimax", data = d_sorted)
run_case("sorted_reflected_pc_promax", sorted, "principal_components", k1, "promax", data = d_sorted)
stopifnot(route_gaps$sorted_reflected_pc_varimax[["reflected"]], route_gaps$sorted_reflected_pc_varimax[["reordered"]])
# The same structure at iterations 2: varimax not converged, a note (decision 717).
run_case("rotation_not_converged_pc", sorted, "principal_components", k1, "varimax", iterations = 2L, data = d_sorted)
# A structure whose cycles differ under a rule on SV's relative change: seed 3.
set.seed(3)
n3 <- sample(c(150, 300, 500), 1); p3 <- sample(6:16, 1); k3 <- sample(2:4, 1); l3 <- sample(c(0.5, 0.6, 0.7), 1)
relative <- structured(n3, p3, k3, l3)
run_case("relative_rule_pc_varimax", relative, "principal_components", k3, "varimax")
# Principal axis using an eigenvalue below 0 (decision 717): factor_cases.rs's
# six weakly related items, five factors.
weak <- sapply(1:6, function(j) sapply(1:40, function(i) (i * 7919 + j * 104729 + i * j * 13) %% 101))
run_case("negative_eigenvalue_pa", weak, "principal_axis", 5L, "none")
printed <- function(x) paste(names(x), sprintf("%.3e", unlist(x)), collapse = " ")
cat("routes, worst gaps:", printed(route_worst), "\n")

ex <- do.call(rbind, expected)
cs <- do.call(rbind, cases)
hdr <- c("# Written by /usr/local/bin/Rscript make_factor_fixtures.R (decisions 701 and 719).",
         paste0("# ", R.version.string, "; seed 20260701; base R and EFAtools ", packageVersion("EFAtools"), "."))
writeLines(c(hdr, "case\tkey\ti\tj\tvalue", do.call(paste, c(ex, sep = "\t"))), "expected.tsv")
writeLines(c(hdr, paste(names(cs), collapse = "\t"), do.call(paste, c(cs, sep = "\t"))), "cases.tsv")
cat("cases:", nrow(cs), " expected rows:", nrow(ex), "\n")
print(cs[, c("case", "expect", "condition")], row.names = FALSE)
