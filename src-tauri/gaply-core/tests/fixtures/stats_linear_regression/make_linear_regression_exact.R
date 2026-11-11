# The exact tier for gaply_core's linear regression (Research OS, Phase 9A-1,
# decision 671): each case's coefficients solved in exact rational arithmetic,
# rounded once to the nearest double, and R's lm coefficients' error against them.
#
# Run by its full path, from anywhere, with CRAN's gmp on the library path
# R_LIBS names (never the system library):
#
#   R_LIBS=<a library holding gmp> /usr/local/bin/Rscript make_linear_regression_exact.R
#
# R and gmp are the reference, run outside Gaply (ADR-008): nothing of either is
# copied into Gaply. Reads cases.tsv and the data_*.csv files that
# make_linear_regression_fixtures.R wrote, every case with a data file of up to
# 2,000 rows that is not a refusal, and writes exact.tsv next to this script.
#
# Every cell is taken as the double the file names, exactly: its %.17g text is
# parsed into an exact rational, and the double nearest to it is found by exact
# comparison among R's own parse and its two neighbours, since R's parser is not
# exact at 17 digits on this platform; the count of cells where R's parse was not
# that double is the header's last line. Each double is then an exact rational
# (as.bigq), the normal equations X'X b = X'y are solved exactly (solve on bigq
# matrices), and each coefficient is rounded once to the nearest double the same
# way. exact.tsv holds, per case and coefficient key, that double (%.17g) and R's
# lm coefficient's error against it, relative to the case's largest |coefficient|.
#
# Synthetic data only. No manuscript or data of anyone's is read.

suppressPackageStartupMessages(library(gmp))

here <- local({
  a <- commandArgs(trailingOnly = FALSE)
  f <- sub("^--file=", "", a[grep("^--file=", a)])
  if (length(f)) dirname(normalizePath(f)) else getwd()
})
setwd(here)

# The exact rational a decimal text names: "-1.25e-08" is -125 / 10^10.
decimal_q <- function(s) {
  m <- regmatches(s, regexec("^(-?)([0-9]*)\\.?([0-9]*)(?:[eE]([-+]?[0-9]+))?$", s))[[1]]
  stopifnot(length(m) == 5)
  # Leading zeros stripped: gmp's as.bigz reads "0..." as octal.
  digits <- sub("^0+", "", paste0(m[3], m[4]))
  e <- (if (nzchar(m[5])) as.integer(m[5]) else 0L) - nchar(m[4])
  q <- as.bigq(as.bigz(if (nzchar(digits)) digits else "0"))
  q <- if (e >= 0) q * as.bigq(as.bigz(10)^e) else q / as.bigq(as.bigz(10)^(-e))
  if (m[2] == "-") -q else q
}

# The neighbours of a finite double, one unit in the last place either side.
neighbours <- function(x) {
  if (x == 0) return(c(-2^-1074, 0, 2^-1074))
  e <- floor(log2(abs(x)))
  u <- 2^(max(e, -1022) - 52)
  c(x - u, x, x + u)
}

# The double nearest to the exact rational q, ties to an even mantissa not
# needed here: a tie is reported as a stop.
nearest <- function(q, guess) {
  repeat {
    cand <- neighbours(guess)
    dq <- lapply(cand, function(c) abs(as.bigq(c) - q))
    best <- 1
    for (i in 2:3) if (dq[[i]] < dq[[best]]) best <- i
    for (i in 1:3) if (i != best && dq[[i]] == dq[[best]]) stop("a tie between two doubles")
    # The nearest is the middle of three only when it is closer than both
    # neighbours; otherwise move towards q and look again.
    if (best == 2) return(cand[2])
    guess <- cand[best]
  }
}

fixed <- 0L
cell <- function(s) {
  r <- as.numeric(s)
  q <- decimal_q(s)
  x <- nearest(q, r)
  if (x != r) fixed <<- fixed + 1L
  x
}

tsv <- function(name) {
  lines <- readLines(name, encoding = "UTF-8")
  lines <- lines[!startsWith(lines, "#")]
  head <- strsplit(lines[1], "\t")[[1]]
  rows <- lapply(lines[-1], function(l) strsplit(l, "\t", fixed = TRUE)[[1]])
  setNames(as.data.frame(do.call(rbind, rows), stringsAsFactors = FALSE), head)
}

cases <- tsv("cases.tsv")
expected <- tsv("expected.tsv")
refused <- unique(expected$case[expected$key == "refused"])
out <- character()
for (i in seq_len(nrow(cases))) {
  cs <- cases[i, ]
  if (cs$file == "-" || cs$case %in% refused) next
  lines <- readLines(cs$file, encoding = "UTF-8")
  lines <- lines[!startsWith(lines, "#")]
  if (length(lines) - 1 > 2000) next
  head <- strsplit(lines[1], ",", fixed = TRUE)[[1]]
  cells <- lapply(lines[-1], function(l) strsplit(paste0(l, ","), ",", fixed = TRUE)[[1]])
  cols <- c(cs$outcome, strsplit(cs$predictors, " ", fixed = TRUE)[[1]])
  idx <- match(cols, head)
  codes <- if (cs$missing_codes == "-") numeric() else as.numeric(strsplit(cs$missing_codes, " ")[[1]])
  rows <- list()
  for (r in cells) {
    v <- r[idx]
    if (any(v == "")) next
    x <- vapply(v, cell, 0)
    if (any(x %in% codes)) next
    rows[[length(rows) + 1]] <- x
  }
  m <- do.call(rbind, rows)
  n <- nrow(m)
  k <- ncol(m) - 1
  Xq <- as.bigq(cbind(1, m[, -1, drop = FALSE]))
  yq <- as.bigq(m[, 1])
  b <- solve(t(Xq) %*% Xq, t(Xq) %*% yq)
  exact <- vapply(seq_len(k + 1), function(j) nearest(b[j], as.numeric(b[j])), 0)
  df <- data.frame(m)
  names(df) <- c("y", paste0("x", seq_len(k)))
  fit <- lm(as.formula(paste("y ~", paste(paste0("x", seq_len(k)), collapse = " + "))), data = df)
  rb <- unname(coef(fit))
  scale <- max(abs(exact))
  keys <- c("intercept.b", sprintf("predictor.%d.b", seq_len(k) - 1))
  for (j in seq_len(k + 1)) {
    out <- c(out, paste(cs$case, keys[j], sprintf("%.17g", exact[j]), sprintf("%.17g", abs(rb[j] - exact[j]) / scale), sep = "\t"))
  }
  cat(sprintf("%s: %d rows, %d coefficients, R's largest error %.3e\n", cs$case, n, k + 1,
              max(abs(rb - exact)) / scale))
}

writeLines(c(
  "# written by: R_LIBS=<a library holding gmp> /usr/local/bin/Rscript make_linear_regression_exact.R",
  sprintf("# %s, gmp %s (CRAN); exact = the exact solution rounded once to the nearest double; r_error = |lm - exact| / the case's largest |exact|",
          R.version.string, as.character(packageVersion("gmp"))),
  sprintf("# cells where R's parse was not the double the text names: %d", fixed),
  "case\tkey\texact\tr_error", out), "exact.tsv", useBytes = TRUE)
cat(sprintf("exact.tsv: %d coefficients; cells R parsed off by an ulp: %d\n", length(out), fixed))
