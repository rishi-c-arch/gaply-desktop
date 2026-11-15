# R reference values for gaply_core::stats_sem, the structural equation models (Research OS,
# Phase 9C-4, decisions 795 to 800).
#
# Run by its full path, from this folder, with lavaan in R's library path:
#
#   R_LIBS=<the folder holding lavaan 0.7-3> /usr/local/bin/Rscript make_sem_fixtures.R
#
# R and lavaan are the reference, run outside Gaply (ADR-008): nothing of either is copied into
# Gaply. lavaan (CRAN, GPL >= 2) is named by decision 746 and decision 800, for fixtures only.
# Each case is fitted by lavaan() with every parameter written out, likelihood "normal",
# information "expected" and fixed.x FALSE, at its default rel.tol. This script then refines
# that solution by its own Fisher scoring on the RAM matrices, written from decisions 795 and
# 798's formulas (not Gaply's code): Sigma = F (I - A)^-1 Phi (I - A)^-T F', its derivative by
# each entry formed as a whole p x p matrix, g_a = tr(R Sigma_a) and H_ab = tr(Si Sigma_a Si
# Sigma_b), until the Newton decrement is at most 1e-20; it stops if lavaan's and the refined
# estimates differ by more than 1e-3 of a standard error. Every statistic is then computed here
# from decision 799's definitions, and every measure of fit from decision 745's formulas, the
# noncentral chi-square's tails and interval from the Poisson mixture of pgamma's tails.
#
# Checked against lavaan, and stopped at any disagreement: lavaan refitted at the refined
# estimates with optim.method "none" gives standard errors (times sqrt(N / (N - 1))),
# standardizedSolution's std.all and lavInspect's rsquare within 1e-10, and the total effects
# from its own matrices within 1e-10; a second fit with likelihood "wishart" gives fitMeasures
# within 1e-6 for those lavaan defines as AMOS does, and its RMSEA interval within 1e-4.
#
# The refusals are checked here by their own rules: the model's rank from the spec alone by the
# singular values of the Jacobian of vech(Sigma) at a random point (decision 795's research's
# check); and S's Cholesky rule for a singular S.
#
# Data: AMOS's Example 6 (Wheaton, the CFA golden's data/wheaton.csv, read where it is), Example
# 7 (data/felson.csv, written here), Example 8 (the CFA golden's data/example8.csv), lavaan's
# PoliticalDemocracy and HolzingerSwineford1939, and synthetic tables, each with its provenance.
# Files written: cases.tsv, expected.tsv, and data/*.csv.

suppressMessages(library(lavaan))
stopifnot(as.character(packageVersion("lavaan")) == "0.7.3")

CFA_DATA <- "../stats_cfa/data"

# ---- the model --------------------------------------------------------------------------
# A spec: k factors (list of column indices, in alias order), paths (rows to, from as variable
# indices: factors 1..k, columns k + 1 .. k + p), covariances (rows a, b), equal (list of groups,
# each a list of members: list(kind, i, j)), over p columns. The raw parameters in decision
# 798's layout: free loadings factor by factor, paths, covariances, variances 1..m.
model_of <- function(spec, p) {
  k <- length(spec$factors)
  m <- k + p
  raw <- list()
  fixed <- matrix(0L, 0, 2)
  endo <- logical(m)
  indicator <- logical(m)
  for (f in seq_len(k)) {
    cols <- spec$factors[[f]]
    for (t in seq_along(cols)) {
      v <- k + cols[t]
      endo[v] <- TRUE; indicator[v] <- TRUE
      if (t == 1) fixed <- rbind(fixed, c(v, f)) else raw[[length(raw) + 1]] <- list(kind = "A", i = v, j = f, type = "loading")
    }
  }
  for (r in seq_len(nrow(spec$paths))) {
    endo[spec$paths[r, 1]] <- TRUE
    raw[[length(raw) + 1]] <- list(kind = "A", i = spec$paths[r, 1], j = spec$paths[r, 2], type = "path")
  }
  for (r in seq_len(nrow(spec$covs))) raw[[length(raw) + 1]] <- list(kind = "P", i = spec$covs[r, 1], j = spec$covs[r, 2], type = "covariance")
  for (v in seq_len(m)) raw[[length(raw) + 1]] <- list(kind = "P", i = v, j = v, type = "variance")
  # Equality: each member matched to its raw parameter; the group takes the first's position.
  key <- function(x) paste(x$kind, x$i, x$j)
  raw_keys <- vapply(raw, key, "")
  group_of <- rep(NA_integer_, length(raw))
  for (g in seq_along(spec$equal)) for (mem in spec$equal[[g]]) {
    at <- match(key(mem), raw_keys)
    if (is.na(at)) stop("an equality member the model does not have")
    group_of[at] <- g
  }
  position <- integer(length(raw)); first_of <- integer(0); q <- 0
  for (r in seq_along(raw)) {
    g <- group_of[r]
    if (!is.na(g) && !is.na(first_of[as.character(g)])) position[r] <- first_of[as.character(g)]
    else { q <- q + 1; position[r] <- q; if (!is.na(g)) first_of[as.character(g)] <- q }
  }
  list(k = k, p = p, m = m, raw = raw, position = position, q = q, fixed = fixed, endo = endo,
       indicator = indicator, paths = spec$paths, covs = spec$covs, factors = spec$factors)
}

matrices <- function(md, theta) {
  A <- matrix(0, md$m, md$m); Phi <- matrix(0, md$m, md$m)
  for (r in seq_len(nrow(md$fixed))) A[md$fixed[r, 1], md$fixed[r, 2]] <- 1
  for (r in seq_along(md$raw)) {
    x <- md$raw[[r]]; v <- theta[md$position[r]]
    if (x$kind == "A") A[x$i, x$j] <- v else { Phi[x$i, x$j] <- v; Phi[x$j, x$i] <- v }
  }
  B <- solve(diag(md$m) - A)
  Omega <- B %*% Phi %*% t(B)
  cols <- md$k + seq_len(md$p)
  list(A = A, Phi = Phi, B = B, Omega = Omega, Sigma = Omega[cols, cols, drop = FALSE], cols = cols)
}

# The derivative of Sigma by each parameter, as whole p x p matrices: for A[t, s],
# F (B E_ts B Phi B' + B Phi B' E_st B') F'; for Phi[a, b], F B (E_ab + E_ba) B' F'.
dsigma <- function(md, u) {
  m <- md$m; cols <- u$cols
  out <- rep(list(matrix(0, md$p, md$p)), md$q)
  for (r in seq_along(md$raw)) {
    x <- md$raw[[r]]
    E <- matrix(0, m, m)
    if (x$kind == "A") {
      E[x$i, x$j] <- 1
      D <- u$B %*% E %*% u$B %*% u$Phi %*% t(u$B)
      D <- D + t(D)
    } else {
      E[x$i, x$j] <- 1; E[x$j, x$i] <- 1
      D <- u$B %*% E %*% t(u$B)
    }
    out[[md$position[r]]] <- out[[md$position[r]]] + D[cols, cols, drop = FALSE]
  }
  out
}

discrepancy <- function(S, Sigma) {
  ch <- tryCatch(chol(Sigma), error = function(e) NULL)
  if (is.null(ch)) return(Inf)
  2 * sum(log(diag(ch))) + sum(diag(S %*% chol2inv(ch))) - 2 * sum(log(diag(chol(S)))) - nrow(S)
}

gradient_and_information <- function(md, S, theta) {
  u <- matrices(md, theta)
  Si <- solve(u$Sigma)
  D <- dsigma(md, u)
  R <- Si %*% (u$Sigma - S) %*% Si
  g <- vapply(D, function(Da) sum(R * Da), 0)
  Aa <- vapply(D, function(Da) as.vector(Si %*% Da), numeric(md$p * md$p))
  Bb <- vapply(D, function(Da) as.vector(t(Si %*% Da)), numeric(md$p * md$p))
  H <- crossprod(Aa, Bb)
  list(g = g, H = (H + t(H)) / 2, u = u)
}

spd_inverse_scaled <- function(H) {
  D <- 1 / sqrt(diag(H))
  chol2inv(chol(H * outer(D, D))) * outer(D, D)
}

refine <- function(md, S, theta, cap = 400) {
  for (it in seq_len(cap)) {
    gh <- gradient_and_information(md, S, theta)
    delta <- -as.vector(spd_inverse_scaled(gh$H) %*% gh$g)
    dec <- -sum(gh$g * delta)
    if (dec <= 1e-20) return(list(theta = theta, iterations = it - 1, decrement = dec))
    F0 <- discrepancy(S, gh$u$Sigma)
    alpha <- 1
    repeat {
      nt <- theta + alpha * delta
      Fn <- tryCatch(discrepancy(S, matrices(md, nt)$Sigma), error = function(e) Inf)
      if (is.finite(Fn) && (dec <= 1e-10 || Fn <= F0)) break
      alpha <- alpha / 2
      if (alpha < 1e-12) stop("the reference's line search found no step")
    }
    theta <- nt
  }
  stop("the reference's Fisher scoring did not reach a decrement of 1e-20")
}

# The model's rank from the spec alone: the singular values of the Jacobian of vech(Sigma) at a
# random point (variances 1 + u, loadings 0.5 + u, paths u - 0.5, covariances small), a value
# above 1e-9 of the largest counting.
spec_rank <- function(md, seed) {
  set.seed(seed)
  theta <- vapply(seq_len(md$q), function(a) {
    r <- md$raw[[match(a, md$position)]]
    switch(r$type, loading = 0.5 + runif(1), path = runif(1) - 0.5, variance = 1 + runif(1), covariance = (runif(1) - 0.5) / 10)
  }, 0)
  u <- matrices(md, theta)
  D <- dsigma(md, u)
  J <- vapply(D, function(Da) Da[lower.tri(Da, diag = TRUE)], numeric(md$p * (md$p + 1) / 2))
  s <- svd(J)$d
  sum(s > 1e-9 * max(s))
}

# ---- the fit's rows (decision 745's formulas, as the CFA golden's script writes them) -------
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
amos_row <- function(C, q, d, rmr, gfi, Cb, db, p, N) {
  n <- N - 1
  pstar <- p * (p + 1) / 2
  dz <- function(x) if (d == 0) "df_zero" else x
  dL <- if (d == 0) "df_zero" else nc_ncp(C, d, 0.95)
  dU <- if (d == 0) "df_zero" else nc_ncp(C, d, 0.05)
  nfi <- 1 - C / Cb
  cfi_den <- max(Cb - db, C - d, 0)
  cfi <- if (cfi_den == 0) 1 else 1 - max(C - d, 0) / cfi_den
  pratio <- d / db
  bcc <- if (N <= p + 2) "too_few_rows_for_bcc" else C + 2 * q * (N - 1) / (N - p - 2)
  hoelter <- function(a) if (d == 0) "df_zero" else if (C == 0) "cmin_zero" else floor(n * qchisq(1 - a, d) / C + 1)
  list(
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
    hoelter_05 = hoelter(0.05), hoelter_01 = hoelter(0.01)
  )
}

# The variables reachable from each along the paths.
reach_of <- function(md) {
  R <- matrix(FALSE, md$m, md$m)
  for (r in seq_len(nrow(md$paths))) R[md$paths[r, 2], md$paths[r, 1]] <- TRUE
  for (v in seq_len(md$m)) R <- R | (R[, v] %o% R[v, ] > 0)
  R
}

# Every statistic of decision 799 at the refined theta; a value not computable is its code.
statistics_of <- function(md, S, theta, N) {
  gh <- gradient_and_information(md, S, theta)
  u <- gh$u
  n <- N - 1
  p <- md$p; k <- md$k; m <- md$m; q <- md$q
  V <- spd_inverse_scaled(n * gh$H / 2)
  se <- sqrt(diag(V))
  two <- function(z) 2 * pnorm(-abs(z))
  std <- function(x, a, b) if (a > 0 && b > 0) x * sqrt(a) / sqrt(b) else "negative_variance"
  tested <- function(e, s) list(estimate = e, se = s, cr = e / s, p = two(e / s))
  weights <- list()
  for (f in seq_len(k)) for (t in seq_along(md$factors[[f]])) {
    v <- k + md$factors[[f]][t]
    st <- std(u$A[v, f], u$Omega[f, f], u$Omega[v, v])
    if (t == 1) weights[[length(weights) + 1]] <- list(estimate = 1, se = "fixed_parameter", cr = "fixed_parameter", p = "fixed_parameter", standardized = st)
    else {
      a <- md$position[which(vapply(md$raw, function(x) x$type == "loading" && x$i == v && x$j == f, TRUE))]
      weights[[length(weights) + 1]] <- c(tested(u$A[v, f], se[a]), list(standardized = st))
    }
  }
  raw_at <- function(type, i, j) md$position[which(vapply(md$raw, function(x) x$type == type && x$i == i && x$j == j, TRUE))]
  for (r in seq_len(nrow(md$paths))) {
    t <- md$paths[r, 1]; s <- md$paths[r, 2]
    weights[[length(weights) + 1]] <- c(tested(u$A[t, s], se[raw_at("path", t, s)]), list(standardized = std(u$A[t, s], u$Omega[s, s], u$Omega[t, t])))
  }
  covs <- lapply(seq_len(nrow(md$covs)), function(r) {
    a <- md$covs[r, 1]; b <- md$covs[r, 2]
    corr <- if (u$Phi[a, a] > 0 && u$Phi[b, b] > 0) u$Phi[a, b] / sqrt(u$Phi[a, a] * u$Phi[b, b]) else "negative_variance"
    c(tested(u$Phi[a, b], se[raw_at("covariance", a, b)]), list(correlation = corr))
  })
  vars <- lapply(seq_len(m), function(v) tested(u$Phi[v, v], se[raw_at("variance", v, v)]))
  smc <- lapply(which(md$endo), function(v) if (u$Omega[v, v] > 0) 1 - u$Phi[v, v] / u$Omega[v, v] else "negative_variance")
  R <- reach_of(md)
  structural <- which(seq_len(m) <= k | !md$indicator)
  effects <- list()
  for (to in structural) for (from in structural) {
    if (to == from || !R[from, to]) next
    total <- u$B[to, from]; direct <- u$A[to, from]; indirect <- total - direct
    a <- u$Omega[from, from]; b <- u$Omega[to, to]
    effects[[length(effects) + 1]] <- list(to = to, from = from, total = total, direct = direct, indirect = indirect,
      total_standardized = std(total, a, b), direct_standardized = std(direct, a, b), indirect_standardized = std(indirect, a, b))
  }
  # The fit (decision 796's rows; decision 761's rule at d = 0).
  Fm <- discrepancy(S, u$Sigma)
  pstar <- p * (p + 1) / 2
  d <- pstar - q
  C <- n * (if (d == 0 && Fm <= 1e-12) 0 else max(Fm, 0))
  Cb <- n * max(sum(log(diag(S))) - 2 * sum(log(diag(chol(S)))), 0)
  db <- p * (p - 1) / 2
  Si <- solve(u$Sigma)
  M <- Si %*% S
  gfi <- 1 - sum(diag((M - diag(p)) %*% (M - diag(p)))) / sum(diag(M %*% M))
  rmr <- sqrt(sum(((S - u$Sigma)[upper.tri(S, diag = TRUE)])^2) / pstar)
  fit <- amos_row(C, q, d, rmr, gfi, Cb, db, p, N)
  saturated <- amos_row(0, pstar, 0, 0, 1, Cb, db, p, N)
  saturated$nfi <- 1; saturated$ifi <- 1
  Mb <- diag(1 / diag(S)) %*% S
  gfi_b <- 1 - sum(diag((Mb - diag(p)) %*% (Mb - diag(p)))) / sum(diag(Mb %*% Mb))
  independence <- amos_row(Cb, p, db, sqrt(sum(((S - diag(diag(S)))[upper.tri(S, diag = TRUE)])^2) / pstar), gfi_b, Cb, db, p, N)
  for (nm in c("nfi", "rfi", "ifi", "tli", "cfi", "pnfi", "pcfi")) independence[[nm]] <- 0
  independence$pratio <- 1
  block <- sort(unique(c(seq_len(k), as.vector(md$covs))))
  pd <- length(block) == 0 || (!is.null(tryCatch(chol(u$Phi[block, block, drop = FALSE]), error = function(e) NULL)) &&
                                all(eigen(u$Phi[block, block, drop = FALSE], symmetric = TRUE, only.values = TRUE)$values > 0))
  notes <- c(if (any(diag(u$Phi) < 0)) "negative_variance",
             if (!pd) "covariance_not_positive_definite",
             if (any(diag(R))) "nonrecursive",
             if (d == 0) "df_zero")
  list(weights = weights, covs = covs, vars = vars, smc = smc, effects = effects, fit = fit,
       saturated = saturated, independence = independence, notes = notes, F = Fm, u = u, se = se)
}

# ---- data -----------------------------------------------------------------------------------
num <- function(v) ifelse(is.na(v), "", sprintf("%.17g", v))
write_data <- function(name, x, provenance) {
  dir.create("data", showWarnings = FALSE)
  con <- file(file.path("data", name), "w")
  writeLines(paste0("# ", provenance), con)
  writeLines("# Generated by /usr/local/bin/Rscript make_sem_fixtures.R (do not edit by hand).", con)
  writeLines(paste(names(x), collapse = ","), con)
  for (i in seq_len(nrow(x))) writeLines(paste(vapply(x[i, ], num, ""), collapse = ","), con)
  close(con)
}
read_data <- function(path) read.csv(path, comment.char = "#")

# Rows whose covariance matrix with divisor N is S_u (N - 1) / N, S_u = D R D (decision 763's
# wheaton_exact), from seeded normals.
exact_cov <- function(R, sd, N, seed, names) {
  set.seed(seed)
  Z <- matrix(rnorm(N * ncol(R)), N)
  Z <- scale(Z, center = TRUE, scale = FALSE)
  Z <- qr.Q(qr(Z)) * sqrt(N)
  x <- as.data.frame(Z %*% chol(R) %*% diag(sd) * sqrt((N - 1) / N) + 50)
  names(x) <- names
  x
}

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

# Example 7 (decision 795): Felson and Bohrnstedt's girls, from correlations and standard
# deviations; S with divisor N is S_u (N - 1) / N.
felson_R <- diag(6)
felson_R[lower.tri(felson_R)] <- c(.50, .49, .10, .04, .09, .32, -.03, -.16, .43, .18, -.10, .15, .34, -.16, -.27)
felson_R[upper.tri(felson_R)] <- t(felson_R)[upper.tri(felson_R)]
felson <- exact_cov(felson_R, c(.16, .49, 3.49, 2.91, 19.32, 1.01), 209, 7955,
                    c("academic", "attract", "GPA", "height", "weight", "rating"))
write_data("felson.csv", felson, paste("Synthetic, reproduced (not read from Fels_fem.sav): 209 rows whose covariance matrix with divisor N",
  "is S_u (N - 1) / N, S_u = D R D from the correlations and standard deviations of AMOS's Example 7 (Felson and Bohrnstedt's",
  "academic, attract, GPA, height, weight and rating; the values of the R package MIIVsem's felson1979; decision 795), seed 7955."))

pd <- PoliticalDemocracy[, c("x1", "x2", "x3", paste0("y", 1:8))]
write_data("poldem.csv", pd, "lavaan's PoliticalDemocracy (Bollen, 1989), all 75 rows, x1 to x3 then y1 to y8.")
hs <- HolzingerSwineford1939
mimic <- hs[, c("x1", "x2", "x3", "sex", "ageyr", "grade")]
write_data("mimic.csv", mimic, "lavaan's HolzingerSwineford1939, all 301 rows, x1 to x3, sex, ageyr and grade (grade's one empty cell kept).")

# A loop between two factors, each with a column of its own pointing into it (seed 7956):
# eta1 = 0.4 eta2 + 0.6 x7 + z1, eta2 = 0.3 eta1 + 0.5 x8 + z2, three columns each.
set.seed(7956)
N_loop <- 400
x7 <- rnorm(N_loop); x8 <- 0.3 * x7 + rnorm(N_loop)
zz <- cbind(rnorm(N_loop, sd = 0.7), rnorm(N_loop, sd = 0.7))
Bl <- matrix(c(0, 0.3, 0.4, 0), 2)
eta <- t(solve(diag(2) - Bl) %*% t(cbind(0.6 * x7, 0.5 * x8) + zz))
loop <- data.frame(c1 = eta[, 1] + rnorm(N_loop, sd = .6), c2 = 0.8 * eta[, 1] + rnorm(N_loop, sd = .6), c3 = 1.2 * eta[, 1] + rnorm(N_loop, sd = .6),
                   c4 = eta[, 2] + rnorm(N_loop, sd = .6), c5 = 0.9 * eta[, 2] + rnorm(N_loop, sd = .6), c6 = 1.1 * eta[, 2] + rnorm(N_loop, sd = .6),
                   c7 = x7, c8 = x8)
write_data("loop.csv", loop, paste("Synthetic: 400 rows of two factors in a loop (eta1 = 0.4 eta2 + 0.6 c7 + z1, eta2 = 0.3 eta1 + 0.5 c8 + z2),",
  "each measured by three columns, c7 and c8 the columns pointing into them (seed 7956)."))

# ---- the cases --------------------------------------------------------------------------------
# A spec's variables: f(n) a factor, column(j) column j of the dataset (its alias Cj). Each case
# names its dataset's columns used by number; they are the model's columns, in alias order.
P <- function(...) { x <- rbind(...); if (is.null(x)) matrix(0L, 0, 2) else x }
none <- matrix(0L, 0, 2)
wh_cols <- 1:6
wheaton_paths <- P(list("f", 1, "f", 3), list("f", 2, "f", 1), list("f", 2, "f", 3))
eqC <- list(list(list("loading", 1, 2), list("loading", 2, 4)),
            list(list("variance", "c", 1), list("variance", "c", 3)),
            list(list("variance", "c", 2), list("variance", "c", 4)))

cases <- list(
  list(name = "wheaton_a", data = "../stats_cfa/data/wheaton.csv", factors = list(c(1, 2), c(3, 4), c(5, 6)), paths = wheaton_paths, covs = list(), equal = list(), codes = numeric(0)),
  list(name = "wheaton_b", data = "../stats_cfa/data/wheaton.csv", factors = list(c(1, 2), c(3, 4), c(5, 6)), paths = wheaton_paths, covs = list(list("c", 1, "c", 3)), equal = list(), codes = numeric(0)),
  list(name = "wheaton_c", data = "../stats_cfa/data/wheaton.csv", factors = list(c(1, 2), c(3, 4), c(5, 6)), paths = wheaton_paths, covs = list(list("c", 1, "c", 3)), equal = eqC, codes = numeric(0)),
  list(name = "wheaton_d", data = "../stats_cfa/data/wheaton.csv", factors = list(c(1, 2), c(3, 4), c(5, 6)), paths = wheaton_paths, covs = list(), equal = eqC, codes = numeric(0)),
  list(name = "example7", data = "felson.csv", factors = list(),
       paths = P(list("c", 1, "c", 2), list("c", 1, "c", 3), list("c", 2, "c", 1), list("c", 2, "c", 4), list("c", 2, "c", 5), list("c", 2, "c", 6)),
       covs = list(list("c", 1, "c", 2), list("c", 3, "c", 4), list("c", 3, "c", 5), list("c", 3, "c", 6), list("c", 4, "c", 5), list("c", 4, "c", 6), list("c", 5, "c", 6)),
       equal = list(), codes = numeric(0)),
  list(name = "example8", data = "../stats_cfa/data/example8.csv", factors = list(1:3, 4:6), paths = none, covs = list(list("f", 1, "f", 2)), equal = list(), codes = numeric(0)),
  list(name = "poldem", data = "poldem.csv", factors = list(1:3, 4:7, 8:11), paths = P(list("f", 2, "f", 1), list("f", 3, "f", 1), list("f", 3, "f", 2)),
       covs = list(list("c", 4, "c", 8), list("c", 5, "c", 7), list("c", 5, "c", 9), list("c", 6, "c", 10), list("c", 7, "c", 11), list("c", 9, "c", 11)), equal = list(), codes = numeric(0)),
  list(name = "poldem_equal", data = "poldem.csv", factors = list(1:3, 4:7, 8:11), paths = P(list("f", 2, "f", 1), list("f", 3, "f", 1), list("f", 3, "f", 2)),
       covs = list(list("c", 4, "c", 8), list("c", 5, "c", 7), list("c", 5, "c", 9), list("c", 6, "c", 10), list("c", 7, "c", 11), list("c", 9, "c", 11)),
       equal = list(list(list("loading", 2, 5), list("loading", 3, 9)), list(list("loading", 2, 6), list("loading", 3, 10)), list(list("loading", 2, 7), list("loading", 3, 11))),
       codes = numeric(0)),
  list(name = "mimic", data = "mimic.csv", factors = list(1:3), paths = P(list("f", 1, "c", 4), list("f", 1, "c", 5), list("f", 1, "c", 6)),
       covs = list(list("c", 4, "c", 5), list("c", 4, "c", 6), list("c", 5, "c", 6)), equal = list(), codes = numeric(0)),
  list(name = "path_equal", data = "../stats_cfa/data/hs301.csv", factors = list(), paths = P(list("c", 3, "c", 1), list("c", 3, "c", 2), list("c", 4, "c", 3)),
       covs = list(list("c", 1, "c", 2)), equal = list(list(list("path", 3, 1), list("path", 3, 2))), codes = numeric(0)),
  list(name = "loop", data = "loop.csv", factors = list(1:3, 4:6), paths = P(list("f", 1, "f", 2), list("f", 1, "c", 7), list("f", 2, "f", 1), list("f", 2, "c", 8)),
       covs = list(list("c", 7, "c", 8)), equal = list(), codes = numeric(0)),
  list(name = "heywood", data = "../stats_cfa/data/heywood.csv", factors = list(1:4), paths = none, covs = list(), equal = list(), codes = numeric(0)),
  list(name = "phi_not_pd", data = "../stats_cfa/data/phi_not_pd.csv", factors = list(1:2, 3:4), paths = none, covs = list(list("f", 1, "f", 2)), equal = list(), codes = numeric(0)),
  list(name = "df_zero", data = "../stats_cfa/data/hs301.csv", factors = list(1:3), paths = none, covs = list(), equal = list(), codes = numeric(0)),
  list(name = "missing_and_codes", data = "../stats_cfa/data/missing.csv", factors = list(1:3, 4:6), paths = P(list("f", 2, "f", 1)), covs = list(), equal = list(), codes = c(-99)),
  list(name = "score", data = "../stats_cfa/data/score.csv", factors = list(1:3), paths = P(list("f", 1, "c", 9)), covs = list(), equal = list(), codes = numeric(0)),
  list(name = "formula_30", data = "formula:30:6:500", factors = formula_factors(30, 6), paths = do.call(P, lapply(2:6, function(f) list("f", f, "f", f - 1))), covs = list(), equal = list(), codes = numeric(0)),
  list(name = "formula_128", data = "formula:128:16:600", factors = formula_factors(128, 16), paths = do.call(P, lapply(2:16, function(f) list("f", f, "f", f - 1))), covs = list(), equal = list(), codes = numeric(0)),
  list(name = "two_column_factor", data = "../stats_cfa/data/hs301.csv", factors = list(1:2), paths = none, covs = list(), equal = list(), codes = numeric(0), refusal = "model_not_identified"),
  list(name = "more_than_moments", data = "../stats_cfa/data/hs301.csv", factors = list(), paths = P(list("c", 1, "c", 3), list("c", 2, "c", 1), list("c", 3, "c", 2)),
       covs = list(list("c", 1, "c", 2), list("c", 1, "c", 3), list("c", 2, "c", 3)), equal = list(), codes = numeric(0), refusal = "model_not_identified"),
  list(name = "singular", data = "../stats_cfa/data/singular.csv", factors = list(1:4), paths = none, covs = list(), equal = list(), codes = numeric(0), refusal = "sample_covariance_singular")
)

# ---- the spec as Gaply's JSON, and as the RAM model -----------------------------------------
var_json <- function(kind, n) if (kind == "f") sprintf('{"factor":%d}', as.integer(n)) else sprintf('{"column":"C%d"}', as.integer(n))
spec_json <- function(case) {
  ind <- paste0("[", paste(vapply(case$factors, function(f) paste0("[", paste0('"C', f, '"', collapse = ","), "]"), ""), collapse = ","), "]")
  paths <- if (nrow(case$paths)) paste(vapply(seq_len(nrow(case$paths)), function(r) {
    x <- case$paths[r, ]; sprintf('{"to":%s,"from":%s}', var_json(x[[1]], x[[2]]), var_json(x[[3]], x[[4]]))
  }, ""), collapse = ",") else ""
  covs <- paste(vapply(case$covs, function(x) sprintf('{"a":%s,"b":%s}', var_json(x[[1]], x[[2]]), var_json(x[[3]], x[[4]])), ""), collapse = ",")
  member <- function(x) switch(x[[1]],
    loading = sprintf('{"loading":{"factor":%d,"column":"C%d"}}', as.integer(x[[2]]), as.integer(x[[3]])),
    path = sprintf('{"path":{"to":{"column":"C%d"},"from":{"column":"C%d"}}}', as.integer(x[[2]]), as.integer(x[[3]])),
    variance = sprintf('{"variance":{"variable":%s}}', var_json(x[[2]], x[[3]])))
  eq <- paste(vapply(case$equal, function(g) paste0("[", paste(vapply(g, member, ""), collapse = ","), "]"), ""), collapse = ",")
  sprintf('{"indicators":%s,"paths":[%s],"covariances":[%s],"equal":[%s],"missingCodes":[%s]}', ind, paths, covs, eq, paste(case$codes, collapse = ","))
}

# The model's columns (by dataset number, ascending) and the RAM spec over them.
ram_spec <- function(case) {
  ends <- c(unlist(case$factors),
            if (nrow(case$paths)) unlist(lapply(seq_len(nrow(case$paths)), function(r) { x <- case$paths[r, ]; c(if (x[[1]] == "c") x[[2]], if (x[[3]] == "c") x[[4]]) })),
            unlist(lapply(case$covs, function(x) c(if (x[[1]] == "c") x[[2]], if (x[[3]] == "c") x[[4]]))))
  cols <- sort(unique(as.integer(ends)))
  k <- length(case$factors)
  v <- function(kind, n) if (kind == "f") as.integer(n) else k + match(as.integer(n), cols)
  paths <- if (nrow(case$paths)) t(vapply(seq_len(nrow(case$paths)), function(r) { x <- case$paths[r, ]; c(v(x[[1]], x[[2]]), v(x[[3]], x[[4]])) }, c(0L, 0L))) else matrix(0L, 0, 2)
  covs <- if (length(case$covs)) t(vapply(case$covs, function(x) c(v(x[[1]], x[[2]]), v(x[[3]], x[[4]])), c(0L, 0L))) else matrix(0L, 0, 2)
  equal <- lapply(case$equal, function(g) lapply(g, function(x) switch(x[[1]],
    loading = list(kind = "A", i = v("c", x[[3]]), j = as.integer(x[[2]])),
    path = list(kind = "A", i = v("c", x[[2]]), j = v("c", x[[3]])),
    variance = { w <- v(x[[2]], x[[3]]); list(kind = "P", i = w, j = w) })))
  list(cols = cols, factors = lapply(case$factors, function(f) match(as.integer(f), cols)), paths = paths, covs = covs, equal = equal)
}

# lavaan's syntax with every parameter written out: factors F1..Fk, columns v1..vp (the model's).
lavaan_syntax <- function(md) {
  name <- function(x) if (x <= md$k) paste0("F", x) else paste0("v", x - md$k)
  label <- character(length(md$raw))
  for (r in seq_along(md$raw)) {
    shared <- sum(md$position == md$position[r]) > 1
    label[r] <- if (shared) paste0("e", md$position[r], "*") else ""
  }
  lines <- character(0)
  for (f in seq_len(md$k)) {
    terms <- character(0)
    for (t in seq_along(md$factors[[f]])) {
      v <- md$k + md$factors[[f]][t]
      if (t == 1) terms <- c(terms, paste0("1*", name(v)))
      else { r <- which(vapply(md$raw, function(x) x$type == "loading" && x$i == v, TRUE)); terms <- c(terms, paste0(label[r], name(v))) }
    }
    lines <- c(lines, paste0("F", f, " =~ ", paste(terms, collapse = " + ")))
  }
  for (r in seq_along(md$raw)) {
    x <- md$raw[[r]]
    if (x$type == "path") lines <- c(lines, paste0(name(x$i), " ~ ", label[r], name(x$j)))
    if (x$type %in% c("covariance", "variance")) lines <- c(lines, paste0(name(x$i), " ~~ ", label[r], name(x$j)))
  }
  paste(lines, collapse = "\n")
}

theta_from_lavaan <- function(fit, md) {
  pe <- parameterEstimates(fit)
  name <- function(x) if (x <= md$k) paste0("F", x) else paste0("v", x - md$k)
  vapply(seq_len(md$q), function(a) {
    x <- md$raw[[match(a, md$position)]]
    if (x$type == "loading") pe$est[pe$lhs == name(x$j) & pe$op == "=~" & pe$rhs == name(x$i)]
    else if (x$type == "path") pe$est[pe$lhs == name(x$i) & pe$op == "~" & pe$rhs == name(x$j)]
    else pe$est[pe$lhs == name(x$i) & pe$op == "~~" & pe$rhs == name(x$j)]
  }, 0)
}

# The model's columns, listwise with the codes as missing; a row left out under its first column
# (alias order) without a value.
prepare <- function(case, rs) {
  x <- if (startsWith(case$data, "formula:")) { a <- as.integer(strsplit(case$data, ":")[[1]][2:4]); formula_data(a[1], a[2], a[3]) }
       else if (startsWith(case$data, "../")) read_data(case$data) else read_data(file.path("data", case$data))
  raw <- x[, rs$cols, drop = FALSE]
  excluded <- list(); keep <- logical(nrow(raw))
  for (r in seq_len(nrow(raw))) {
    keep[r] <- TRUE
    for (j in seq_along(rs$cols)) {
      v <- raw[r, j]
      code <- if (is.na(v)) "missing_value" else if (v %in% case$codes) "missing_code" else NULL
      if (!is.null(code)) {
        key <- paste(code, rs$cols[j], sep = ":")
        excluded[[key]] <- (if (is.null(excluded[[key]])) 0 else excluded[[key]]) + 1
        keep[r] <- FALSE
        break
      }
    }
  }
  d <- raw[keep, , drop = FALSE]
  names(d) <- paste0("v", seq_along(rs$cols))
  N <- nrow(d)
  ex_keys <- names(excluded)
  if (length(ex_keys)) {
    col <- as.integer(sub(".*:", "", ex_keys)); kind <- ifelse(startsWith(ex_keys, "missing_value"), 0, 1)
    ex_keys <- ex_keys[order(col, kind)]
  }
  list(d = d, N = N, S = cov(d) * (N - 1) / N, exclusions = vapply(ex_keys, function(k) paste0(k, ":", excluded[[k]]), ""))
}

fail <- function(...) stop(paste0(...), call. = FALSE)
expected <- list()
add <- function(case, key, value) expected[[length(expected) + 1]] <<- c(case, key, if (is.character(value)) value else sprintf("%.17g", value))
vname <- function(md, rs, v) if (v <= md$k) paste0("f", v) else paste0("C", rs$cols[v - md$k])
checked <- list()

for (case in cases) {
  rs <- ram_spec(case)
  pr <- prepare(case, rs)
  md <- model_of(list(factors = rs$factors, paths = rs$paths, covs = rs$covs, equal = rs$equal), length(rs$cols))
  cat(case$name, ": N", pr$N, "p", md$p, "q", md$q, "\n")
  if (!is.null(case$refusal)) {
    if (case$refusal == "sample_covariance_singular") {
      S <- pr$S; at <- NA
      for (j in 2:md$p) {
        A <- S[1:(j - 1), 1:(j - 1), drop = FALSE]; b <- S[1:(j - 1), j]
        if (S[j, j] - sum(b * solve(A, b)) <= 1e-9 * S[j, j]) { at <- j; break }
      }
      if (is.na(at)) fail(case$name, ": S is not singular by the 1e-9 rule")
      add(case$name, "refusal", paste0("sample_covariance_singular:column=", rs$cols[at]))
      cat("  refused: column", rs$cols[at], "\n")
    } else {
      ranks <- c(spec_rank(md, 7957), spec_rank(md, 7958))
      if (ranks[1] != ranks[2]) fail(case$name, ": the two points' ranks differ")
      if (ranks[1] >= md$q) fail(case$name, ": identified by R's rank")
      add(case$name, "refusal", paste0("model_not_identified:count=", md$q - ranks[1]))
      cat("  refused: q", md$q, "rank", ranks[1], "\n")
    }
    next
  }
  syntax <- lavaan_syntax(md)
  fit <- lavaan(syntax, data = pr$d, likelihood = "normal", information = "expected", fixed.x = FALSE)
  if (!lavInspect(fit, "converged")) fail(case$name, ": lavaan did not converge")
  th0 <- theta_from_lavaan(fit, md)
  ref <- refine(md, pr$S, th0)
  st <- statistics_of(md, pr$S, ref$theta, pr$N)
  gap <- max(abs(th0 - ref$theta) / st$se)
  if (gap > 1e-3) fail(case$name, ": lavaan's estimates and the refined differ by ", gap, " standard errors")
  cat("  refined in", ref$iterations, "iterations, decrement", ref$decrement, "; lavaan within", signif(gap, 3), "SE\n")
  # lavaan at the refined estimates.
  pt <- parTable(fit)
  free <- pt$free > 0
  rows_of <- function(a) {
    x <- md$raw[[match(a, md$position)]]
    nm <- function(v) if (v <= md$k) paste0("F", v) else paste0("v", v - md$k)
    if (x$type == "loading") which(pt$lhs == nm(x$j) & pt$op == "=~" & pt$rhs == nm(x$i))
    else if (x$type == "path") which(pt$lhs == nm(x$i) & pt$op == "~" & pt$rhs == nm(x$j))
    else which(pt$lhs == nm(x$i) & pt$op == "~~" & pt$rhs == nm(x$j))
  }
  for (r in seq_along(md$raw)) {
    x <- md$raw[[r]]
    nm <- function(v) if (v <= md$k) paste0("F", v) else paste0("v", v - md$k)
    row <- if (x$type == "loading") which(pt$lhs == nm(x$j) & pt$op == "=~" & pt$rhs == nm(x$i))
           else if (x$type == "path") which(pt$lhs == nm(x$i) & pt$op == "~" & pt$rhs == nm(x$j))
           else which(pt$lhs == nm(x$i) & pt$op == "~~" & pt$rhs == nm(x$j))
    pt$start[row] <- ref$theta[md$position[r]]
    pt$est[row] <- ref$theta[md$position[r]]
  }
  at_ref <- lavaan(pt, data = pr$d, likelihood = "normal", information = "expected", fixed.x = FALSE, optim.method = "none", se = "standard")
  pe <- parameterEstimates(at_ref)
  lav_se <- vapply(seq_len(md$q), function(a) pe$se[rows_of(a)[1]], 0) * sqrt(pr$N / (pr$N - 1))
  d_se <- max(abs(lav_se - st$se) / st$se)
  ss <- standardizedSolution(at_ref)
  nm <- function(v) if (v <= md$k) paste0("F", v) else paste0("v", v - md$k)
  # std.all for each weight (=~ and ~) and each covariance.
  d_std <- 0; wi <- 0
  for (f in seq_len(md$k)) for (t in seq_along(md$factors[[f]])) {
    wi <- wi + 1; v <- md$k + md$factors[[f]][t]
    mine <- st$weights[[wi]]$standardized
    if (!is.character(mine)) d_std <- max(d_std, abs(ss$est.std[ss$lhs == nm(f) & ss$op == "=~" & ss$rhs == nm(v)] - mine))
  }
  for (r in seq_len(nrow(md$paths))) {
    wi <- wi + 1
    mine <- st$weights[[wi]]$standardized
    if (!is.character(mine)) d_std <- max(d_std, abs(ss$est.std[ss$lhs == nm(md$paths[r, 1]) & ss$op == "~" & ss$rhs == nm(md$paths[r, 2])] - mine))
  }
  for (r in seq_len(nrow(md$covs))) {
    mine <- st$covs[[r]]$correlation
    if (!is.character(mine)) d_std <- max(d_std, abs(ss$est.std[ss$lhs == nm(md$covs[r, 1]) & ss$op == "~~" & ss$rhs == nm(md$covs[r, 2])] - mine))
  }
  r2 <- lavInspect(at_ref, "rsquare")
  d_r2 <- 0; r2_skipped <- 0
  for (t in seq_along(st$smc)) {
    v <- which(md$endo)[t]; lav <- r2[nm(v)]
    if (is.na(lav)) { r2_skipped <- r2_skipped + 1; next }
    if (!is.character(st$smc[[t]])) d_r2 <- max(d_r2, abs(lav - st$smc[[t]]))
  }
  if (r2_skipped != sum(diag(st$u$Phi)[which(md$endo)] < 0)) fail(case$name, ": lavaan's R2 is NA beside a variance that is not negative")
  # Total effects from lavaan's own matrices, where both ends are in its beta.
  est <- lavInspect(at_ref, "est")
  beta <- est$beta
  d_eff <- 0; eff_compared <- 0
  if (!is.null(beta)) {
    total <- solve(diag(nrow(beta)) - beta) - diag(nrow(beta))
    for (e in st$effects) {
      a <- nm(e$to); b <- nm(e$from)
      if (a %in% rownames(total) && b %in% colnames(total)) { d_eff <- max(d_eff, abs(total[a, b] - e$total)); eff_compared <- eff_compared + 1 }
    }
  }
  if (d_se > 1e-10 || d_std > 1e-10 || d_r2 > 1e-10 || d_eff > 1e-10) fail(case$name, ": lavaan at the refined estimates differs: se ", d_se, " std ", d_std, " r2 ", d_r2, " effects ", d_eff)
  wish <- lavaan(syntax, data = pr$d, likelihood = "wishart", information = "expected", fixed.x = FALSE)
  fm <- fitMeasures(wish, c("chisq", "df", "pvalue", "cfi", "tli", "nfi", "rfi", "ifi", "pnfi", "agfi", "pgfi", "rmsea", "rmsea.pvalue", "ecvi", "rmsea.ci.lower", "rmsea.ci.upper"))
  mine <- st$fit
  pairs <- list(chisq = mine$cmin, df = mine$df, pvalue = mine$p, cfi = mine$cfi, tli = mine$tli, nfi = mine$nfi,
                rfi = mine$rfi, ifi = mine$ifi, pnfi = mine$pnfi, agfi = mine$agfi, pgfi = mine$pgfi,
                rmsea = mine$rmsea, rmsea.pvalue = mine$pclose, ecvi = mine$ecvi)
  worst_fm <- 0; worst_name <- ""
  for (nmx in names(pairs)) {
    a <- pairs[[nmx]]; b <- fm[[nmx]]
    if (is.character(a) || is.na(b)) next
    e <- abs(a - b)
    if (e > worst_fm) { worst_fm <- e; worst_name <- nmx }
    if (e > 1e-6) fail(case$name, ": lavaan's wishart ", nmx, " ", b, " against ", a)
  }
  if (!is.character(mine$rmsea_lo90)) {
    ci <- c(fm[["rmsea.ci.lower"]], fm[["rmsea.ci.upper"]]); ours <- c(mine$rmsea_lo90, mine$rmsea_hi90)
    if (max(abs(ci - ours)) > 1e-4) fail(case$name, ": lavaan's RMSEA interval ", ci[1], " ", ci[2], " against ", ours[1], " ", ours[2])
  }
  checked[[case$name]] <- c(se = d_se, std = d_std, r2 = d_r2, effects = d_eff, effects_compared = eff_compared, fit = worst_fm)
  cat("  lavaan agrees: se", signif(d_se, 3), "std", signif(d_std, 3), "r2", signif(d_r2, 3), "effects", signif(d_eff, 3), "(", eff_compared, ") fit", signif(worst_fm, 3), worst_name, "\n")
  add(case$name, "rows_used", pr$N)
  for (i in seq_along(st$weights)) for (fld in c("estimate", "se", "cr", "p", "standardized")) add(case$name, paste0("weight.", i - 1, ".", fld), st$weights[[i]][[fld]])
  for (i in seq_along(st$covs)) for (fld in c("estimate", "se", "cr", "p", "correlation")) add(case$name, paste0("cov.", i - 1, ".", fld), st$covs[[i]][[fld]])
  for (i in seq_along(st$vars)) for (fld in c("estimate", "se", "cr", "p")) add(case$name, paste0("var.", i - 1, ".", fld), st$vars[[i]][[fld]])
  for (i in seq_along(st$smc)) add(case$name, paste0("smc.", i - 1), st$smc[[i]])
  for (i in seq_along(st$effects)) {
    e <- st$effects[[i]]
    add(case$name, paste0("effect.", i - 1, ".ends"), paste0(vname(md, rs, e$to), "<", vname(md, rs, e$from)))
    for (fld in c("total", "direct", "indirect", "total_standardized", "direct_standardized", "indirect_standardized")) add(case$name, paste0("effect.", i - 1, ".", fld), e[[fld]])
  }
  for (nmx in names(st$fit)) add(case$name, paste0("fit.", nmx), st$fit[[nmx]])
  for (nmx in names(st$saturated)) add(case$name, paste0("fit_saturated.", nmx), st$saturated[[nmx]])
  for (nmx in names(st$independence)) add(case$name, paste0("fit_independence.", nmx), st$independence[[nmx]])
  for (k in seq_along(st$notes)) add(case$name, paste0("note.", k - 1), st$notes[k])
  for (k in seq_along(pr$exclusions)) add(case$name, paste0("exclusion.", k - 1), pr$exclusions[[k]])
  cat("  CMIN", mine$cmin, "DF", mine$df, "effects", length(st$effects), "notes", paste(st$notes, collapse = " "), "\n")
}

header <- c("# R's reference for gaply_core::stats_sem (Research OS, Phase 9C-4, decision 800).",
            "# Generated by /usr/local/bin/Rscript make_sem_fixtures.R (do not edit by hand).",
            paste0("# ", R.version.string, ", ", R.version$arch, "; lavaan ", as.character(packageVersion("lavaan")), "."))
con <- file("cases.tsv", "w")
writeLines(header, con)
writeLines("case\tdata\tspec", con)
for (case in cases) writeLines(paste(case$name, case$data, spec_json(case), sep = "\t"), con)
close(con)
con <- file("expected.tsv", "w")
writeLines(header, con)
writeLines("case\tkey\tvalue", con)
for (e in expected) writeLines(paste(e, collapse = "\t"), con)
close(con)
cat("cases", length(cases), "expected rows", length(expected), "\n")
cat("lavaan's largest disagreements: se", max(sapply(checked, `[`, "se")), "std", max(sapply(checked, `[`, "std")),
    "r2", max(sapply(checked, `[`, "r2")), "effects", max(sapply(checked, `[`, "effects")), "fit", max(sapply(checked, `[`, "fit")), "\n")
