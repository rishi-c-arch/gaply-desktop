# R's check of gaply_core::stats_cfa on the bug hunt's h043 (Research OS, Phase 9C-2,
# decision 763), where Gaply's minimum is below lavaan 0.7-3's.
#
# Run by its full path, from this folder, with lavaan in R's library path, after cfa_cases.rs
# has written h043_gaply.tsv (GAPLY_WRITE_CFA_H043):
#
#   R_LIBS=<the folder holding lavaan 0.7-3> /usr/local/bin/Rscript check_h043.R
#
# Reads data/h043.csv and Gaply's estimates, and writes h043_r.tsv: R's F at those estimates
# (decision 743's discrepancy), R's gradient there (its largest component and its norm), the Newton
# decrement g' H^-1 g with decision 745's g and H (H inverted scaled to a unit diagonal, as the golden's refine does; its reciprocal condition is written beside it), and lavaan's own minimum F, its default cfa fit's estimates put into the same F, each
# to 17 significant digits. The functions below, model_of to spd_inverse_scaled and lavaan_model
# and theta_from_lavaan, are copied verbatim from make_cfa_fixtures.R. R and lavaan are the
# reference, run outside Gaply (ADR-008).

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

d <- read.csv("data/h043.csv", comment.char = "#")
N <- nrow(d)
S <- cov(d) * (N - 1) / N
factors <- list(c(1, 15), c(2, 9, 10, 14), c(3, 4, 8, 11, 12, 13), c(5, 6, 7))
cols <- unlist(factors)
S <- S[cols, cols]
dd <- d[, cols]
names(dd) <- paste0("v", seq_along(cols))
positions <- list(); at <- 0
for (f in seq_along(factors)) { k <- length(factors[[f]]); positions[[f]] <- at + seq_len(k); at <- at + k }
m <- model_of(positions, length(cols), TRUE)
g <- read.delim("h043_gaply.tsv", comment.char = "#")
theta <- g$value[startsWith(g$key, "theta.")]
stopifnot(length(theta) == m$q)
u <- unpack(m, theta)
r_f <- discrepancy(S, u$Sigma)
gh <- gradient_and_information(m, S, theta)
dec <- sum(gh$g * as.vector(spd_inverse_scaled(gh$H) %*% gh$g))
fit <- cfa(lavaan_model(positions), data = dd)
lavaan_f <- discrepancy(S, unpack(m, theta_from_lavaan(fit, m))$Sigma)
cat("N", N, "q", m$q, "R's F", sprintf("%.17g", r_f), "decrement", dec, "lavaan's F", sprintf("%.17g", lavaan_f),
    "lavaan converged", lavInspect(fit, "converged"), "\n")
con <- file("h043_r.tsv", "w")
writeLines(c("# R's check of Gaply's h043 (Research OS, Phase 9C-2, decision 763).",
             "# Generated by /usr/local/bin/Rscript check_h043.R (do not edit by hand).",
             paste0("# ", R.version.string, ", ", R.version$arch, "; lavaan ", as.character(packageVersion("lavaan")), "."),
             "key\tvalue",
             paste0("r_f\t", sprintf("%.17g", r_f)),
             paste0("r_decrement\t", sprintf("%.17g", dec)),
             paste0("r_gradient_max_abs\t", sprintf("%.17g", max(abs(gh$g)))),
             paste0("r_gradient_norm\t", sprintf("%.17g", sqrt(sum(gh$g^2)))),
             paste0("r_h_reciprocal_condition\t", sprintf("%.17g", rcond(gh$H))),
             paste0("lavaan_f\t", sprintf("%.17g", lavaan_f))), con)
close(con)
