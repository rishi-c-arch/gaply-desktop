# R reference for gaply_core::stats_cfa's second attempt from the triad starts (Research OS,
# Phase 9C-2, decision 761): an uncorrelated model that decision 745's starts refuse and the
# triad starts solve.
#
# Run by its full path, from this folder, with lavaan in R's library path:
#
#   R_LIBS=<the folder holding lavaan 0.7-3> /usr/local/bin/Rscript make_cfa_rescue_fixtures.R
#
# R and lavaan are the reference, run outside Gaply (ADR-008): nothing of either is copied
# into Gaply. The data are drawn from the model by rescue_data(114): 157 rows of 27 columns on
# five uncorrelated factors (seed 114 of a FIND FIRST search over seeds 1 to 300, kept under
# ~/gaply-corpus/os/phase9/9c2/hunt/, where 7bafd6df refused 66 of 300 and the triad attempt
# solved 64). lavaan's cfa (likelihood "normal", information "expected", h1.information
# "structured", std.lv FALSE, orthogonal TRUE) is refined by make_cfa_fixtures.R's own Fisher
# scoring until the Newton decrement is at most 1e-20; its functions below, model_of to
# refine and lavaan_model and theta_from_lavaan, are copied verbatim from that script.
#
# Files written: data/uncorrelated_rescued.csv and uncorrelated_rescued_expected.tsv.

suppressMessages(library(lavaan))
stopifnot(as.character(packageVersion("lavaan")) == "0.7.3")

# ---- copied verbatim from make_cfa_fixtures.R ----------------------------------------------
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
# ---- end of the copy -----------------------------------------------------------------------

# The generator: an uncorrelated model of 4 to 8 factors of 3 to 8 columns, at most 40 columns
# and 100 to 500 rows, drawn from the model with standardized loadings in .3 to .9, each column
# given its own scale and rounded to 4 decimals.
rescue_data <- function(seed) {
  set.seed(seed)
  k <- sample(4:8, 1)
  sizes <- sample(3:8, k, replace = TRUE)
  while (sum(sizes) > 40) sizes[which.max(sizes)] <- sizes[which.max(sizes)] - 1
  N <- sample(100:500, 1)
  p <- sum(sizes)
  f_of <- rep(seq_len(k), sizes)
  lambda <- runif(p, 0.3, 0.9)
  scale <- exp(runif(p, log(0.5), log(20)))
  F <- matrix(rnorm(N * k), N, k)
  E <- matrix(rnorm(N * p), N, p)
  x <- sapply(seq_len(p), function(j) round(scale[j] * (lambda[j] * F[, f_of[j]] + sqrt(1 - lambda[j]^2) * E[, j]), 4))
  x <- as.data.frame(x)
  names(x) <- paste0("v", seq_len(p))
  ends <- cumsum(sizes)
  factors <- lapply(seq_len(k), function(f) (ends[f] - sizes[f] + 1):ends[f])
  list(x = x, factors = factors, N = N, p = p, k = k)
}

SEED <- 114
g <- rescue_data(SEED)
num <- function(v) ifelse(is.na(v), "", sprintf("%.17g", v))
dir.create("data", showWarnings = FALSE)
con <- file(file.path("data", "uncorrelated_rescued.csv"), "w")
writeLines(paste0("# Synthetic: rescue_data(", SEED, ") of make_cfa_rescue_fixtures.R, ", g$N, " rows of ", g$p,
                  " columns drawn from ", g$k, " uncorrelated factors (", paste(vapply(g$factors, function(v) paste0(v[1], "-", v[length(v)]), ""), collapse = ", "),
                  "); decision 745's starts refuse it and the triad starts solve it (decision 761)."), con)
writeLines("# Generated by /usr/local/bin/Rscript make_cfa_rescue_fixtures.R (do not edit by hand).", con)
writeLines(paste(names(g$x), collapse = ","), con)
for (i in seq_len(nrow(g$x))) writeLines(paste(vapply(g$x[i, ], num, ""), collapse = ","), con)
close(con)

d <- g$x
N <- nrow(d)
S <- cov(d) * (N - 1) / N
m <- model_of(g$factors, g$p, FALSE)
fit <- cfa(lavaan_model(g$factors), data = d, likelihood = "normal", information = "expected",
           h1.information = "structured", std.lv = FALSE, orthogonal = TRUE)
if (!lavInspect(fit, "converged")) stop("lavaan did not converge")
th0 <- theta_from_lavaan(fit, m)
lav_cmin <- (N - 1) * discrepancy(S, unpack(m, th0)$Sigma)
ref <- refine(m, S, th0)
u <- unpack(m, ref$theta)
cmin <- (N - 1) * max(discrepancy(S, u$Sigma), 0)
cat("N", N, "p", g$p, "k", g$k, "lavaan CMIN", sprintf("%.17g", lav_cmin), "refined CMIN", sprintf("%.17g", cmin),
    "in", ref$iterations, "iterations, decrement", ref$decrement, "\n")

out <- list()
add <- function(key, value) out[[length(out) + 1]] <<- c("uncorrelated_rescued", key, sprintf("%.17g", value))
add("rows_used", N)
add("lavaan_cmin", lav_cmin)
add("fit.cmin", cmin)
for (i in seq_len(g$p)) add(paste0("loading.", i - 1, ".estimate"), u$L[i, m$f_of[i]])
for (f in seq_len(g$k)) add(paste0("fvar.", f, ".estimate"), u$Phi[f, f])
for (i in seq_len(g$p)) add(paste0("evar.", i - 1, ".estimate"), u$Theta[i, i])
con <- file("uncorrelated_rescued_expected.tsv", "w")
writeLines(c("# R's reference for gaply_core::stats_cfa's second attempt (Research OS, Phase 9C-2, decision 761).",
             "# Generated by /usr/local/bin/Rscript make_cfa_rescue_fixtures.R (do not edit by hand).",
             paste0("# ", R.version.string, ", ", R.version$arch, "; lavaan ", as.character(packageVersion("lavaan")),
                    "; lavaan's solution refined to a Newton decrement of ", signif(ref$decrement, 3), " in ", ref$iterations, " iterations."),
             paste0("# factors ", paste(vapply(g$factors, function(v) paste(v, collapse = ","), ""), collapse = ";"), " uncorrelated")), con)
writeLines("case\tkey\tvalue", con)
for (e in out) writeLines(paste(e, collapse = "\t"), con)
close(con)
cat("expected rows", length(out), "\n")
