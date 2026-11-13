# R reference values for gaply_core::stats_cfa, the confirmatory factor analysis (Research
# OS, Phase 9C-1, decisions 743 to 746).
#
# Run by its full path, from this folder, with lavaan in R's library path:
#
#   R_LIBS=<the folder holding lavaan 0.7-3> /usr/local/bin/Rscript make_cfa_fixtures.R
#
# R and lavaan are the reference, run outside Gaply (ADR-008): nothing of either is copied
# into Gaply. lavaan (CRAN, GPL >= 2) is named by decision 746, for fixtures only. Each case
# is fitted by lavaan's cfa with likelihood "normal", information "expected",
# h1.information "structured" and std.lv FALSE, at its default rel.tol; this script then
# refines that solution by its own Fisher scoring, written from decision 743's formulas (not
# Gaply's code), until the Newton decrement is at most 1e-20, and stops if lavaan's and the
# refined estimates differ by more than 1e-3 of a standard error. Every statistic and fit
# measure is then computed here from decision 745's formulas, the noncentral chi-square's
# tails and interval from the Poisson mixture of pgamma's tails, each tail direct.
#
# Checked against lavaan, and stopped at any disagreement: lavaan refitted at the refined
# estimates with optim.method "none" gives standard errors (times sqrt(N / (N - 1))),
# standardizedSolution's std.all and lavInspect's rsquare within 1e-10; a second fit with
# likelihood "wishart" gives fitMeasures within 1e-6 for those lavaan defines as AMOS does,
# and its RMSEA interval within 1e-4.
#
# Data: lavaan's HolzingerSwineford1939 (Holzinger and Swineford, 1939; as distributed with
# lavaan), AMOS's Example 8 being its 73 Grant-White girls in AMOS's units (decision 743),
# and synthetic tables, each written here with its provenance; two cases are built by a
# formula identical in R and in cfa_r_golden.rs and have no file.
#
# Files written: cases.tsv, expected.tsv, and data/*.csv.

suppressMessages(library(lavaan))
stopifnot(as.character(packageVersion("lavaan")) == "0.7.3")

# The model: `factors` a list of integer vectors, each factor's items (columns of S), in order.
model_of <- function(factors, p, correlated) {
  k <- length(factors)
  f_of <- integer(p)
  for (f in seq_len(k)) f_of[factors[[f]]] <- f
  marker <- as.integer(vapply(factors, function(v) v[1], 0))
  free_loadings <- setdiff(seq_len(p), marker)       # in item order
  pairs <- if (correlated && k > 1) t(combn(k, 2)) else matrix(0L, 0, 2)
  list(p = p, k = k, f_of = f_of, marker = marker, free = free_loadings, pairs = pairs, correlated = correlated,
       q = length(free_loadings) + k + nrow(pairs) + p)
}

# theta = c(free loadings in item order, factor variances 1..k, covariances in pair order, error variances 1..p).
unpack <- function(m, theta) {
  nl <- length(m$free)
  L <- matrix(0, m$p, m$k)
  for (i in seq_len(m$p)) L[i, m$f_of[i]] <- 1
  if (nl) for (t in seq_len(nl)) L[m$free[t], m$f_of[m$free[t]]] <- theta[t]
  Phi <- diag(theta[nl + seq_len(m$k)], m$k, m$k)
  if (nrow(m$pairs)) for (t in seq_len(nrow(m$pairs))) {
    f <- m$pairs[t, 1]; g <- m$pairs[t, 2]
    Phi[f, g] <- Phi[g, f] <- theta[nl + m$k + t]
  }
  Theta <- diag(theta[nl + m$k + nrow(m$pairs) + seq_len(m$p)], m$p, m$p)
  list(L = L, Phi = Phi, Theta = Theta, Sigma = L %*% Phi %*% t(L) + Theta)
}

# The derivative of Sigma with respect to each parameter, as a list of p x p matrices.
dsigma <- function(m, u) {
  out <- list()
  C <- u$L %*% u$Phi                                  # p x k, column f is Lambda Phi[, f]
  for (i in m$free) {
    f <- m$f_of[i]; e <- numeric(m$p); e[i] <- 1
    out[[length(out) + 1]] <- e %o% C[, f] + C[, f] %o% e
  }
  for (f in seq_len(m$k)) out[[length(out) + 1]] <- u$L[, f] %o% u$L[, f]
  if (nrow(m$pairs)) for (t in seq_len(nrow(m$pairs))) {
    f <- m$pairs[t, 1]; g <- m$pairs[t, 2]
    out[[length(out) + 1]] <- u$L[, f] %o% u$L[, g] + u$L[, g] %o% u$L[, f]
  }
  for (i in seq_len(m$p)) { E <- matrix(0, m$p, m$p); E[i, i] <- 1; out[[length(out) + 1]] <- E }
  out
}

discrepancy <- function(S, Sigma) {
  ch <- tryCatch(chol(Sigma), error = function(e) NULL)
  if (is.null(ch)) return(Inf)
  2 * sum(log(diag(ch))) + sum(diag(S %*% chol2inv(ch))) - 2 * sum(log(diag(chol(S)))) - nrow(S)
}

# g and H at theta: g_a = tr(Si (Sigma - S) Si D_a), H_ab = tr(Si D_a Si D_b).
gradient_and_information <- function(m, S, theta) {
  u <- unpack(m, theta)
  Si <- solve(u$Sigma)
  D <- dsigma(m, u)
  R <- Si %*% (u$Sigma - S) %*% Si
  g <- vapply(D, function(Da) sum(R * Da), 0)
  A <- vapply(D, function(Da) as.vector(Si %*% Da), numeric(m$p * m$p))
  B <- vapply(D, function(Da) as.vector(t(Si %*% Da)), numeric(m$p * m$p))
  H <- crossprod(A, B)
  list(g = g, H = (H + t(H)) / 2, u = u)
}

# H scaled to a unit diagonal before its Cholesky factor, so the variances' and the loadings'
# scales (1e7 and 1 in the 128-column case) do not make it look singular to R's solve, whose
# reciprocal-condition test depends on them; the result is the same matrix inverse.
spd_inverse_scaled <- function(H) {
  D <- 1 / sqrt(diag(H))
  chol2inv(chol(H * outer(D, D))) * outer(D, D)
}

# Fisher scoring from theta until the Newton decrement g' H^-1 g is at most 1e-20.
refine <- function(m, S, theta, cap = 300) {
  for (it in seq_len(cap)) {
    gh <- gradient_and_information(m, S, theta)
    delta <- -as.vector(spd_inverse_scaled(gh$H) %*% gh$g)
    dec <- -sum(gh$g * delta)
    if (dec <= 1e-20) return(list(theta = theta, iterations = it - 1, decrement = dec))
    F0 <- discrepancy(S, gh$u$Sigma)
    alpha <- 1
    repeat {
      nt <- theta + alpha * delta
      Fn <- discrepancy(S, unpack(m, nt)$Sigma)
      if (is.finite(Fn) && (dec <= 1e-10 || Fn <= F0)) break
      alpha <- alpha / 2
      if (alpha < 1e-12) stop("the reference's line search found no step")
    }
    theta <- nt
  }
  stop("the reference's Fisher scoring did not reach a decrement of 1e-20")
}

# The noncentral chi-square's tails from the Poisson mixture of pgamma's, each tail direct.
nc_tails <- function(x, df, ncp) {
  if (ncp == 0) return(c(lower = pchisq(x, df), upper = pchisq(x, df, lower.tail = FALSE)))
  lambda <- ncp / 2; mode <- floor(lambda)
  span <- ceiling(60 * sqrt(lambda + 1)) + 50
  j <- seq(max(0, mode - span), mode + span)
  w <- dpois(j, lambda)
  k <- order(abs(j - mode), j)
  c(lower = sum((w * pgamma(x / 2, df / 2 + j))[k]), upper = sum((w * pgamma(x / 2, df / 2 + j, lower.tail = FALSE))[k]))
}
nc_ncp <- function(x, df, target) {
  if (nc_tails(x, df, 0)[["lower"]] <= target) return(0)
  hi <- max(x, 1)
  while (nc_tails(x, df, hi)[["lower"]] > target) hi <- 2 * hi
  uniroot(function(n) nc_tails(x, df, n)[["lower"]] - target, c(0, hi), tol = 1e-13)$root
}

# Every statistic of decision 745 at the refined theta. Returns a named list; a value not
# computable is the code string. N rows used.
statistics_of <- function(m, S, theta, N) {
  gh <- gradient_and_information(m, S, theta)
  u <- gh$u
  n <- N - 1
  p <- m$p; k <- m$k; q <- m$q
  V <- spd_inverse_scaled(n * gh$H / 2)
  se <- sqrt(diag(V))
  two <- function(z) 2 * pnorm(-abs(z))
  sig <- diag(u$Sigma)
  nl <- length(m$free)
  idx_var <- nl + seq_len(k)
  idx_cov <- nl + k + seq_len(nrow(m$pairs))
  idx_err <- nl + k + nrow(m$pairs) + seq_len(p)
  neg <- function(x) if (x <= 0) "negative_variance" else x
  loadings <- lapply(seq_len(p), function(i) {
    f <- m$f_of[i]; t <- match(i, m$free)
    lam <- u$L[i, f]
    std <- if (u$Phi[f, f] <= 0 || sig[i] <= 0) "negative_variance" else lam * sqrt(u$Phi[f, f]) / sqrt(sig[i])
    if (is.na(t)) list(factor = f, item = i, estimate = 1, se = "fixed_parameter", cr = "fixed_parameter", p = "fixed_parameter", standardized = std)
    else list(factor = f, item = i, estimate = lam, se = se[t], cr = lam / se[t], p = two(lam / se[t]), standardized = std)
  })
  fv <- lapply(seq_len(k), function(f) { t <- idx_var[f]; e <- theta[t]; list(factor = f, estimate = e, se = se[t], cr = e / se[t], p = two(e / se[t])) })
  fc <- lapply(seq_len(nrow(m$pairs)), function(t) {
    f <- m$pairs[t, 1]; g <- m$pairs[t, 2]; a <- idx_cov[t]; e <- theta[a]
    corr <- if (u$Phi[f, f] <= 0 || u$Phi[g, g] <= 0) "negative_variance" else e / sqrt(u$Phi[f, f] * u$Phi[g, g])
    list(f = f, g = g, estimate = e, se = se[a], cr = e / se[a], p = two(e / se[a]), correlation = corr)
  })
  ev <- lapply(seq_len(p), function(i) { t <- idx_err[i]; e <- theta[t]; list(item = i, estimate = e, se = se[t], cr = e / se[t], p = two(e / se[t])) })
  smc <- vapply(seq_len(p), function(i) 1 - theta[idx_err[i]] / sig[i], 0)
  # The fit.
  Fm <- discrepancy(S, u$Sigma)
  C <- n * max(Fm, 0)
  pstar <- p * (p + 1) / 2
  d <- pstar - q
  Cb <- n * (sum(log(diag(S))) - 2 * sum(log(diag(chol(S)))))
  db <- p * (p - 1) / 2
  Si <- solve(u$Sigma)
  M <- Si %*% S
  gfi <- 1 - sum(diag((M - diag(p)) %*% (M - diag(p)))) / sum(diag(M %*% M))
  resid <- (S - u$Sigma)[upper.tri(S, diag = TRUE)]
  rmr <- sqrt(sum(resid^2) / pstar)
  dz <- function(x) if (d == 0) "df_zero" else x
  dL <- if (d == 0) "df_zero" else nc_ncp(C, d, 0.95)
  dU <- if (d == 0) "df_zero" else nc_ncp(C, d, 0.05)
  nfi <- 1 - C / Cb
  cfi_den <- max(Cb - db, C - d, 0)
  cfi <- if (cfi_den == 0) 1 else 1 - max(C - d, 0) / cfi_den
  pratio <- d / db
  bcc <- if (N <= p + 2) "too_few_rows_for_bcc" else C + 2 * q * (N - 1) / (N - p - 2)
  hoelter <- function(a) if (d == 0) "df_zero" else if (C == 0) "cmin_zero" else floor(n * qchisq(1 - a, d) / C + 1)
  fit <- list(
    npar = q, cmin = C, df = d, p = dz(if (d > 0) pchisq(C, d, lower.tail = FALSE) else 0), cmin_df = dz(C / d),
    rmr = rmr, gfi = gfi, agfi = dz(1 - (pstar / d) * (1 - gfi)), pgfi = dz((d / pstar) * gfi),
    nfi = nfi, rfi = dz(1 - (C / d) / (Cb / db)), ifi = (Cb - C) / (Cb - d), tli = dz((Cb / db - C / d) / (Cb / db - 1)), cfi = cfi,
    pratio = dz(pratio), pnfi = dz(nfi * pratio), pcfi = dz(cfi * pratio),
    ncp = max(C - d, 0), ncp_lo90 = dL, ncp_hi90 = dU,
    fmin = C / n, f0 = max(C - d, 0) / n, f0_lo90 = if (d == 0) "df_zero" else dL / n, f0_hi90 = if (d == 0) "df_zero" else dU / n,
    rmsea = dz(sqrt(max(C - d, 0) / n / d)), rmsea_lo90 = if (d == 0) "df_zero" else sqrt(dL / (n * d)), rmsea_hi90 = if (d == 0) "df_zero" else sqrt(dU / (n * d)),
    pclose = if (d == 0) "df_zero" else nc_tails(C, d, 0.0025 * n * d)[["upper"]],
    aic = C + 2 * q, bcc = bcc, bic = C + q * log(N), caic = C + q * (log(N) + 1),
    ecvi = (C + 2 * q) / n, ecvi_lo90 = if (d == 0) "df_zero" else (dL + d + 2 * q) / n, ecvi_hi90 = if (d == 0) "df_zero" else (dU + d + 2 * q) / n,
    mecvi = if (is.character(bcc)) bcc else bcc / n,
    hoelter_05 = hoelter(0.05), hoelter_01 = hoelter(0.01),
    independence_npar = p, independence_cmin = Cb, independence_df = db
  )
  notes <- c(if (any(c(theta[idx_var], theta[idx_err]) < 0)) "negative_variance",
             if (is.null(tryCatch(chol(u$Phi), error = function(e) NULL)) || any(eigen(u$Phi, symmetric = TRUE, only.values = TRUE)$values <= 0)) "factor_covariance_not_positive_definite",
             if (d == 0) "df_zero")
  list(loadings = loadings, factor_variances = fv, factor_covariances = fc, error_variances = ev, smc = smc, fit = fit, notes = notes, F = Fm, Sigma = u$Sigma)
}

# ---- data -----------------------------------------------------------------------------

num <- function(v) ifelse(is.na(v), "", sprintf("%.17g", v))

write_data <- function(name, x, provenance) {
  dir.create("data", showWarnings = FALSE)
  con <- file(file.path("data", name), "w")
  writeLines(paste0("# ", provenance), con)
  writeLines("# Generated by /usr/local/bin/Rscript make_cfa_fixtures.R (do not edit by hand).", con)
  writeLines(paste(names(x), collapse = ","), con)
  for (i in seq_len(nrow(x))) writeLines(paste(vapply(x[i, ], num, ""), collapse = ","), con)
  close(con)
}

# K factors over N rows and p items, every value an integer: item j on factor (j - 1) mod K,
# with a term every factor shares so that they correlate. Each factor's score multiplies the row
# by its own number, distinct and nonzero mod the prime 1013, so no two factors' scores are
# shifts of one sequence (a shift put two of 16 factors at a correlation of 0.90; these lie in
# 0.44 to 0.56 at 600 rows). cfa_r_golden.rs builds the same.
formula_data <- function(p, k, N) {
  i <- seq_len(N)
  x <- sapply(seq_len(p), function(j) {
    f <- (j - 1) %% k
    score <- ((i * (7907 + 104729 * f)) %% 1013) - 506
    shared <- ((i * 2654435761) %% 1009) - 504
    noise <- ((i * 104729 + j * 7919 + i * j * 31) %% 997) - 498
    (2 + (j %% 3)) * (score + shared) + noise
  })
  x <- as.data.frame(x)
  names(x) <- paste0("v", seq_len(p))
  x
}
formula_factors <- function(p, k) lapply(seq_len(k), function(f) seq(f, p, by = k))

# Rows whose correlation matrix is exactly `R` (to rounding), from seeded normals.
exact <- function(R, N, seed) {
  set.seed(seed)
  Z <- matrix(rnorm(N * ncol(R)), N)
  Z <- scale(Z, center = TRUE, scale = FALSE)
  Z <- qr.Q(qr(Z)) * sqrt(N)
  x <- as.data.frame(Z %*% chol(R) * 10 + 50)
  names(x) <- paste0("v", seq_len(ncol(R)))
  x
}

hs <- HolzingerSwineford1939
hs9 <- hs[, paste0("x", 1:9)]
g <- hs[hs$school == "Grant-White" & hs$sex == 2, ]
ex8 <- data.frame(visperc = 6 * g$x1, cubes = 4 * g$x2, lozenges = 8 * g$x3,
                  paragrap = 3 * g$x4, sentence = 4 * g$x5, wordmean = 7 * g$x6)
write_data("example8.csv", ex8, paste("AMOS's Example 8 (IBM SPSS Amos 22 User's Guide): lavaan's HolzingerSwineford1939,",
  "school Grant-White and sex 2, 73 rows, in AMOS's units visperc = 6 x1, cubes = 4 x2, lozenges = 8 x3,",
  "paragrap = 3 x4, sentence = 4 x5, wordmean = 7 x6 (decision 743)."))
write_data("hs301.csv", hs9, "lavaan's HolzingerSwineford1939, all 301 rows, x1 to x9.")

heywood_R <- matrix(c(1, .8, .8, .5, .8, 1, .5, .4, .8, .5, 1, .4, .5, .4, .4, 1), 4)
write_data("heywood.csv", exact(heywood_R, 200, 7451), "Synthetic: 200 rows of four columns whose correlations are exactly r12 = r13 = .8, r14 = .5, r23 = .5, r24 = r34 = .4 (seed 7451); one factor gives the first column a negative error variance.")
phi_R <- matrix(c(1, .3, .5, .5, .3, 1, .5, .5, .5, .5, 1, .3, .5, .5, .3, 1), 4)
write_data("phi_not_pd.csv", exact(phi_R, 200, 7452), "Synthetic: 200 rows of four columns whose correlations are exactly .3 within the pairs (1, 2) and (3, 4) and .5 across them (seed 7452), so two correlated factors correlate above 1.")
singular <- hs9[1:120, 1:3]
singular$x4 <- singular$x1 + singular$x2
write_data("singular.csv", singular, "lavaan's HolzingerSwineford1939, rows 1 to 120, x1 to x3, and a fourth column the sum of x1 and x2.")
set.seed(7453)
loose <- as.data.frame(matrix(round(rnorm(200 * 6, 50, 10), 2), 200))
names(loose) <- paste0("v", 1:6)
write_data("loose.csv", loose, "Synthetic: 200 rows of six independent normal columns rounded to 2 decimals (seed 7453), so one factor fits them worse than the independence model by more than its degrees of freedom.")
holes <- hs9[1:150, 1:6]
holes[c(3, 17, 40), 2] <- NA
holes[c(5, 41), 5] <- -99
holes[c(9, 60), 1] <- NA
holes[60, 4] <- -99
write_data("missing.csv", holes, "lavaan's HolzingerSwineford1939, rows 1 to 150, x1 to x6, with seven cells blanked and three set to the code -99.")
sc <- hs9[, 1:8]
sc$score <- (hs9$x7 + hs9$x8 + hs9$x9) / 3
write_data("score.csv", sc, "lavaan's HolzingerSwineford1939, all 301 rows, x1 to x8, and a ninth column the mean of x7, x8 and x9, as a scale score would be.")

# ---- the cases -----------------------------------------------------------------------------

cases <- list(
  list(name = "example8", data = "example8.csv", x = ex8, factors = list(1:3, 4:6), correlated = TRUE, codes = numeric(0)),
  list(name = "hs_correlated", data = "hs301.csv", x = hs9, factors = list(1:3, 4:6, 7:9), correlated = TRUE, codes = numeric(0)),
  list(name = "hs_uncorrelated", data = "hs301.csv", x = hs9, factors = list(1:3, 4:6, 7:9), correlated = FALSE, codes = numeric(0)),
  list(name = "one_factor_three", data = "hs301.csv", x = hs9, factors = list(1:3), correlated = TRUE, codes = numeric(0)),
  list(name = "two_by_two", data = "hs301.csv", x = hs9, factors = list(c(1, 2), c(4, 5)), correlated = TRUE, codes = numeric(0)),
  list(name = "heywood", data = "heywood.csv", x = NULL, factors = list(1:4), correlated = TRUE, codes = numeric(0)),
  list(name = "phi_not_pd", data = "phi_not_pd.csv", x = NULL, factors = list(1:2, 3:4), correlated = TRUE, codes = numeric(0)),
  list(name = "not_identified", data = "hs301.csv", x = hs9, factors = list(c(1, 2), c(4, 5)), correlated = FALSE, codes = numeric(0), refusal = "model_not_identified"),
  list(name = "singular", data = "singular.csv", x = singular, factors = list(c(1, 2), c(3, 4)), correlated = TRUE, codes = numeric(0), refusal = "sample_covariance_singular"),
  list(name = "worse_than_independence", data = "loose.csv", x = loose, factors = list(1:6), correlated = TRUE, codes = numeric(0)),
  list(name = "missing_and_codes", data = "missing.csv", x = holes, factors = list(1:3, 4:6), correlated = TRUE, codes = c(-99)),
  list(name = "score_as_item", data = "score.csv", x = sc, factors = list(1:3, c(4, 5, 9)), correlated = TRUE, codes = numeric(0)),
  list(name = "formula_30", data = "formula:30:6:500", x = formula_data(30, 6, 500), factors = formula_factors(30, 6), correlated = TRUE, codes = numeric(0)),
  list(name = "formula_128", data = "formula:128:16:600", x = formula_data(128, 16, 600), factors = formula_factors(128, 16), correlated = TRUE, codes = numeric(0))
)
cases[[6]]$x <- read.csv("data/heywood.csv", comment.char = "#")
cases[[7]]$x <- read.csv("data/phi_not_pd.csv", comment.char = "#")

# ---- fitting ------------------------------------------------------------------------------

# The model's columns in the spec's order, listwise over them with the codes as missing, and
# the rows left out under the first column, in the dataset's order, without a value.
prepare <- function(case) {
  cols <- unlist(case$factors)
  raw <- case$x[, cols, drop = FALSE]
  ordered <- order(cols)
  excluded <- list()
  keep <- logical(nrow(raw))
  for (r in seq_len(nrow(raw))) {
    keep[r] <- TRUE
    for (j in ordered) {
      v <- raw[r, j]
      code <- if (is.na(v)) "missing_value" else if (v %in% case$codes) "missing_code" else NULL
      if (!is.null(code)) {
        key <- paste(code, cols[j], sep = ":")
        excluded[[key]] <- (if (is.null(excluded[[key]])) 0 else excluded[[key]]) + 1
        keep[r] <- FALSE
        break
      }
    }
  }
  d <- raw[keep, , drop = FALSE]
  names(d) <- paste0("v", seq_along(cols))
  N <- nrow(d)
  S <- cov(d) * (N - 1) / N
  positions <- list(); at <- 0
  for (f in seq_along(case$factors)) { n <- length(case$factors[[f]]); positions[[f]] <- at + seq_len(n); at <- at + n }
  # Exclusions in the dataset's column order, missing_value before missing_code.
  ex_keys <- names(excluded)
  if (length(ex_keys)) {
    col <- as.integer(sub(".*:", "", ex_keys)); kind <- ifelse(startsWith(ex_keys, "missing_value"), 0, 1)
    ex_keys <- ex_keys[order(col, kind)]
  }
  list(d = d, N = N, S = S, positions = positions, cols = cols,
       exclusions = vapply(ex_keys, function(k) paste0(k, ":", excluded[[k]]), ""))
}

lavaan_model <- function(positions) paste(vapply(seq_along(positions), function(f)
  paste0("F", f, " =~ ", paste0("v", positions[[f]], collapse = " + ")), ""), collapse = "\n")

# lavaan's estimates in this file's theta order.
theta_from_lavaan <- function(fit, m) {
  pe <- parameterEstimates(fit)
  get <- function(l, o, r) pe$est[pe$lhs == l & pe$op == o & pe$rhs == r]
  c(vapply(m$free, function(i) get(paste0("F", m$f_of[i]), "=~", paste0("v", i)), 0),
    vapply(seq_len(m$k), function(f) get(paste0("F", f), "~~", paste0("F", f)), 0),
    if (nrow(m$pairs)) vapply(seq_len(nrow(m$pairs)), function(t) get(paste0("F", m$pairs[t, 1]), "~~", paste0("F", m$pairs[t, 2])), 0) else numeric(0),
    vapply(seq_len(m$p), function(i) get(paste0("v", i), "~~", paste0("v", i)), 0))
}

fail <- function(...) stop(paste0(...), call. = FALSE)

expected <- list()
add <- function(case, key, value) expected[[length(expected) + 1]] <<- c(case, key, if (is.character(value)) value else sprintf("%.17g", value))
checked <- list()

for (case in cases) {
  pr <- prepare(case)
  m <- model_of(pr$positions, ncol(pr$S), case$correlated)
  cat(case$name, ": N", pr$N, "p", m$p, "q", m$q, "\n")
  if (!is.null(case$refusal)) {
    if (case$refusal == "sample_covariance_singular") {
      # Gaply's rule: the value left at column j is at most 1e-9 of s_jj.
      S <- pr$S; left <- NA; at <- NA
      for (j in seq_len(m$p)) {
        if (j == 1) next
        A <- S[1:(j - 1), 1:(j - 1), drop = FALSE]; b <- S[1:(j - 1), j]
        left_j <- S[j, j] - sum(b * solve(A, b))
        if (left_j <= 1e-9 * S[j, j]) { left <- left_j; at <- j; break }
      }
      if (is.na(at)) fail(case$name, ": S is not singular by the 1e-9 rule")
      add(case$name, "refusal", paste0("sample_covariance_singular:column=", pr$cols[at]))
      cat("  refused: column", pr$cols[at], "left", left, "\n")
    } else {
      # H at the starts: its smallest pivot ratio.
      S <- pr$S
      th <- c(vapply(m$free, function(i) { mk <- m$marker[m$f_of[i]]; S[i, mk] / (S[mk, mk] / 2) }, 0),
              vapply(seq_len(m$k), function(f) S[m$marker[f], m$marker[f]] / 2, 0),
              if (nrow(m$pairs)) vapply(seq_len(nrow(m$pairs)), function(t) S[m$marker[m$pairs[t, 1]], m$marker[m$pairs[t, 2]]] / 2, 0) else numeric(0),
              diag(S) / 2)
      H <- gradient_and_information(m, S, th)$H
      ev <- eigen(H, symmetric = TRUE, only.values = TRUE)$values
      ratio <- min(ev) / max(ev)
      if (ratio > 1e-9) fail(case$name, ": H at the starts is not singular (ratio ", ratio, ")")
      add(case$name, "refusal", "model_not_identified")
      cat("  refused: H's eigenvalue ratio", ratio, "\n")
    }
    next
  }
  fit <- cfa(lavaan_model(pr$positions), data = pr$d, likelihood = "normal", information = "expected",
             h1.information = "structured", std.lv = FALSE, orthogonal = !case$correlated)
  th0 <- theta_from_lavaan(fit, m)
  ref <- refine(m, pr$S, th0)
  st <- statistics_of(m, pr$S, ref$theta, pr$N)
  se <- sqrt(diag(spd_inverse_scaled((pr$N - 1) * gradient_and_information(m, pr$S, ref$theta)$H / 2)))
  gap <- max(abs(th0 - ref$theta) / se)
  if (gap > 1e-3) fail(case$name, ": lavaan's estimates and the refined differ by ", gap, " standard errors")
  cat("  refined in", ref$iterations, "iterations, decrement", ref$decrement, "; lavaan within", signif(gap, 3), "SE\n")
  # lavaan at the refined estimates.
  pt <- parTable(fit)
  free <- pt$free > 0
  pt$est[free] <- NA
  th_lav <- theta_from_lavaan(fit, m)
  order_free <- match(paste(pt$lhs, pt$op, pt$rhs)[free], c(
    vapply(m$free, function(i) paste0("F", m$f_of[i], " =~ v", i), ""),
    vapply(seq_len(m$k), function(f) paste0("F", f, " ~~ F", f), ""),
    if (nrow(m$pairs)) vapply(seq_len(nrow(m$pairs)), function(t) paste0("F", m$pairs[t, 1], " ~~ F", m$pairs[t, 2]), "") else character(0),
    vapply(seq_len(m$p), function(i) paste0("v", i, " ~~ v", i), "")))
  pt$start[free] <- ref$theta[order_free]
  pt$est[free] <- ref$theta[order_free]
  at_ref <- lavaan(pt, data = pr$d, likelihood = "normal", information = "expected", h1.information = "structured",
                   optim.method = "none", se = "standard")
  pe <- parameterEstimates(at_ref)
  pe_free <- pe[match(paste(pt$lhs, pt$op, pt$rhs)[free], paste(pe$lhs, pe$op, pe$rhs)), ]
  lav_se <- pe_free$se * sqrt(pr$N / (pr$N - 1))
  d_se <- max(abs(lav_se - se[order_free]) / se[order_free])
  ss <- standardizedSolution(at_ref)
  lav_std <- vapply(seq_len(m$p), function(i) ss$est.std[ss$op == "=~" & ss$rhs == paste0("v", i)], 0)
  my_std <- vapply(st$loadings, function(l) if (is.character(l$standardized)) NA else l$standardized, 0)
  d_std <- max(abs(lav_std - my_std), na.rm = TRUE)
  lav_r2 <- as.numeric(lavInspect(at_ref, "rsquare"))
  # lavaan 0.7-3 gives NA where an error variance is below 0; decision 745 gives 1 - theta / sigma
  # there, above 1 (measured 13 Nov 2026 on the Heywood case). Those are counted, not compared.
  r2_skipped <- sum(is.na(lav_r2))
  if (r2_skipped != sum(vapply(st$error_variances, function(e) e$estimate < 0, TRUE))) fail(case$name, ": lavaan's R2 is NA beside a variance that is not negative")
  d_r2 <- max(c(0, abs(lav_r2 - st$smc)), na.rm = TRUE)
  if (d_se > 1e-10 || d_std > 1e-10 || d_r2 > 1e-10) fail(case$name, ": lavaan at the refined estimates differs: se ", d_se, " std ", d_std, " r2 ", d_r2)
  wish <- cfa(lavaan_model(pr$positions), data = pr$d, likelihood = "wishart", information = "expected",
              h1.information = "structured", std.lv = FALSE, orthogonal = !case$correlated)
  fm <- fitMeasures(wish, c("chisq", "df", "pvalue", "cfi", "tli", "nfi", "rfi", "ifi", "pnfi", "agfi", "pgfi", "rmsea", "rmsea.pvalue", "ecvi", "rmsea.ci.lower", "rmsea.ci.upper"))
  mine <- st$fit
  # lavaan 0.7-3's fitMeasures gives a gfi of 1.000 on Example 8 beside an agfi of 0.910 and a pgfi
  # of 0.368, which are 1 - (p* / d)(1 - GFI) and (d / p*) GFI at the formula's GFI of 0.9659
  # (measured 13 Nov 2026), so its gfi entry is not compared; agfi and pgfi carry GFI.
  pairs <- list(chisq = mine$cmin, df = mine$df, pvalue = mine$p, cfi = mine$cfi, tli = mine$tli, nfi = mine$nfi,
                rfi = mine$rfi, ifi = mine$ifi, pnfi = mine$pnfi, agfi = mine$agfi, pgfi = mine$pgfi,
                rmsea = mine$rmsea, rmsea.pvalue = mine$pclose, ecvi = mine$ecvi)
  worst_fm <- 0; worst_name <- ""
  for (nm in names(pairs)) {
    a <- pairs[[nm]]; b <- fm[[nm]]
    if (is.character(a) || is.na(b)) next
    e <- abs(a - b)
    if (e > worst_fm) { worst_fm <- e; worst_name <- nm }
    if (e > 1e-6) fail(case$name, ": lavaan's wishart ", nm, " ", b, " against ", a)
  }
  ci <- c(fm[["rmsea.ci.lower"]], fm[["rmsea.ci.upper"]])
  if (!is.character(mine$rmsea_lo90)) {
    ours <- c(mine$rmsea_lo90, mine$rmsea_hi90)
    if (length(ours) != 2 || anyNA(ci) || anyNA(ours)) fail(case$name, ": the RMSEA interval is not two numbers on both sides")
    if (max(abs(ci - ours)) > 1e-4) fail(case$name, ": lavaan's RMSEA interval ", ci[1], " ", ci[2], " against ", ours[1], " ", ours[2])
    worst_fm <- max(worst_fm, max(abs(ci - ours)) / 100)
  }
  checked[[case$name]] <- c(se = d_se, std = d_std, r2 = d_r2, fit = worst_fm, r2_skipped = r2_skipped)
  cat("  lavaan agrees: se", signif(d_se, 3), "std", signif(d_std, 3), "r2", signif(d_r2, 3), "fit", signif(worst_fm, 3), worst_name, "\n")
  # The expected rows.
  add(case$name, "rows_used", pr$N)
  for (i in seq_along(st$loadings)) {
    l <- st$loadings[[i]]
    for (fld in c("estimate", "se", "cr", "p", "standardized")) add(case$name, paste0("loading.", i - 1, ".", fld), l[[fld]])
  }
  for (f in seq_along(st$factor_variances)) { v <- st$factor_variances[[f]]; for (fld in c("estimate", "se", "cr", "p")) add(case$name, paste0("fvar.", f, ".", fld), v[[fld]]) }
  for (cv in st$factor_covariances) for (fld in c("estimate", "se", "cr", "p", "correlation")) add(case$name, paste0("fcov.", cv$f, ".", cv$g, ".", fld), cv[[fld]])
  for (i in seq_along(st$error_variances)) { v <- st$error_variances[[i]]; for (fld in c("estimate", "se", "cr", "p")) add(case$name, paste0("evar.", i - 1, ".", fld), v[[fld]]) }
  for (i in seq_along(st$smc)) add(case$name, paste0("smc.", i - 1), st$smc[i])
  for (nm in names(st$fit)) add(case$name, paste0("fit.", nm), st$fit[[nm]])
  for (k in seq_along(st$notes)) add(case$name, paste0("note.", k - 1), st$notes[k])
  for (k in seq_along(pr$exclusions)) add(case$name, paste0("exclusion.", k - 1), pr$exclusions[[k]])
  cat("  CMIN", mine$cmin, "DF", mine$df, "notes", paste(st$notes, collapse = " "), "\n")
}

# ---- writing ------------------------------------------------------------------------------

header <- c("# R's reference for gaply_core::stats_cfa (Research OS, Phase 9C-1, decision 746).",
            "# Generated by /usr/local/bin/Rscript make_cfa_fixtures.R (do not edit by hand).",
            paste0("# ", R.version.string, ", ", R.version$arch, "; lavaan ", as.character(packageVersion("lavaan")), "."))
con <- file("cases.tsv", "w")
writeLines(header, con)
writeLines("case\tdata\tfactors\tcorrelated\tcodes", con)
for (case in cases) writeLines(paste(case$name, case$data, paste(vapply(case$factors, function(v) paste(v, collapse = ","), ""), collapse = ";"),
                                     if (case$correlated) "true" else "false", paste(case$codes, collapse = ","), sep = "\t"), con)
close(con)
con <- file("expected.tsv", "w")
writeLines(header, con)
writeLines("case\tkey\tvalue", con)
for (e in expected) writeLines(paste(e, collapse = "\t"), con)
close(con)
cat("cases", length(cases), "expected rows", length(expected), "\n")
cat("lavaan's largest disagreements: se", max(sapply(checked, `[`, "se")), "std", max(sapply(checked, `[`, "std")),
    "r2", max(sapply(checked, `[`, "r2")), "fit", max(sapply(checked, `[`, "fit")), "\n")
