# Synthetic, seeded datasets for gaply_core's precision sweep
# (tests/l1_precision_sweep.rs). First written for the joint evaluation of the
# Statistical Analysis Verifier (15 Oct 2026); copied here so the repository
# can rebuild its fixtures. Run from this folder:
#   /usr/local/bin/Rscript make_eval_datasets.R
# Every case of the original script is still drawn, in the same order, so the
# seeded draws are unchanged; only the five the sweep uses are written, each
# as <case>.csv beside this script.
out <- "."
HDR <- "written by /usr/local/bin/Rscript make_eval_datasets.R (src-tauri/gaply-core/tests/fixtures/stats_l1/precision_sweep/)"
KEEP <- c("c02_t_welch_1dec", "c06_paired_2dec", "c08_anova3_1dec", "c11_pearson_neg_2dec", "c18_ols1_2dec")
fmt <- function(x, sig = 15) {
  if (x == 0) return("0")
  d <- max(0, sig - 1 - floor(log10(abs(x))))
  s <- sprintf(paste0("%.", d, "f"), x)
  s
}
fmtp <- function(p) { d <- min(18, max(0, 14 - floor(log10(abs(p))))); s <- sprintf(paste0("%.", d, "f"), p); sub("^0\\.", ".", s) }  # at most 18 decimals: the reader keeps exact rationals in 64 bits
num_str <- function(v, d) ifelse(is.na(v), "", sprintf(paste0("%.", d, "f"), v))
truth <- data.frame()
add_truth <- function(case, claim, method, variant, stat, df1, df2, p) {
  truth <<- rbind(truth, data.frame(case = case, claim = claim, method = method, variant = variant,
    stat = sprintf("%.17g", stat), df1 = sprintf("%.17g", df1), df2 = ifelse(is.na(df2), "", sprintf("%.17g", df2)), p = sprintf("%.17g", p)))
}
write_case <- function(case, df, sentences, binds) {
  if (!(case %in% KEEP)) return(invisible(NULL))
  f <- file.path(out, paste0(case, ".csv"))
  writeLines(c(paste0("# The precision sweep's dataset ", case, " (synthetic, seed 20261015),"), paste0("# ", HDR, ".")), f)
  suppressWarnings(write.table(df, f, sep = ",", append = TRUE, row.names = FALSE, quote = FALSE, na = ""))
}
# Round on write, then read back: R computes on exactly what the file holds.
rt <- function(v, d) as.numeric(num_str(v, d))
tsent <- function(t, df, p, dfs = fmt(df)) sprintf("The difference was t(%s) = %s, p = %s.", dfs, fmt(t), fmtp(p))

set.seed(20261015)
## 1 independent t, 2 decimals, Student printed
a <- rt(rnorm(15, 10, 2), 2); b <- rt(rnorm(15, 11.5, 2), 2)
tt <- t.test(a, b, var.equal = TRUE)
write_case("c01_t_student_2dec", data.frame(score = num_str(c(a, b), 2), grp = rep(c("A", "B"), c(15, 15))),
  list(tsent(tt$statistic, tt$parameter, tt$p.value)),
  '[{"claim":0,"method":"independent_t","roles":{"outcome":["score"],"group":["grp"]},"levels":{"group":["A","B"]}}]')
add_truth("c01_t_student_2dec", 0, "independent_t", "student_pooled", tt$statistic, tt$parameter, NA, tt$p.value)
## 2 Welch, unequal n and variance, 1 decimal, fractional df printed
a <- rt(rnorm(8, 20, 1), 1); b <- rt(rnorm(25, 18, 6), 1)
tt <- t.test(a, b, var.equal = FALSE)
write_case("c02_t_welch_1dec", data.frame(score = num_str(c(a, b), 1), grp = rep(c("ctl", "trt"), c(8, 25))),
  list(tsent(tt$statistic, tt$parameter, tt$p.value)),
  '[{"claim":0,"method":"independent_t","roles":{"outcome":["score"],"group":["grp"]},"levels":{"group":["ctl","trt"]}}]')
add_truth("c02_t_welch_1dec", 0, "independent_t", "welch", tt$statistic, tt$parameter, NA, tt$p.value)
## 3 Welch, df printed rounded to a whole number, 4 decimals
a <- rt(rnorm(12, 5, 0.5), 4); b <- rt(rnorm(30, 5.6, 2), 4)
tt <- t.test(a, b, var.equal = FALSE)
write_case("c03_t_welch_wholedf_4dec", data.frame(score = num_str(c(a, b), 4), grp = rep(c("x", "y"), c(12, 30))),
  list(tsent(tt$statistic, tt$parameter, tt$p.value, dfs = sprintf("%d", round(tt$parameter)))),
  '[{"claim":0,"method":"independent_t","roles":{"outcome":["score"],"group":["grp"]},"levels":{"group":["x","y"]}}]')
add_truth("c03_t_welch_wholedf_4dec", 0, "independent_t", "welch", tt$statistic, round(tt$parameter), NA, tt$p.value)
## 4 group of 2, 4 decimals, Student
a <- rt(c(3.1, 3.9), 4) + rt(rnorm(2, 0, 0.01), 4); b <- rt(rnorm(10, 4.2, 0.6), 4)
a <- rt(a, 4)
tt <- t.test(a, b, var.equal = TRUE)
write_case("c04_t_group_of_2", data.frame(score = num_str(c(a, b), 4), grp = rep(c("g1", "g2"), c(2, 10))),
  list(tsent(tt$statistic, tt$parameter, tt$p.value)),
  '[{"claim":0,"method":"independent_t","roles":{"outcome":["score"],"group":["grp"]},"levels":{"group":["g1","g2"]}}]')
add_truth("c04_t_group_of_2", 0, "independent_t", "student_pooled", tt$statistic, tt$parameter, NA, tt$p.value)
## 5 integer data with ties, Student, integer-coded group
a <- sample(1:7, 20, TRUE); b <- sample(2:7, 18, TRUE)
tt <- t.test(a, b, var.equal = TRUE)
write_case("c05_t_ties_int", data.frame(score = c(a, b), arm = rep(c(1, 2), c(20, 18))),
  list(tsent(tt$statistic, tt$parameter, tt$p.value)),
  '[{"claim":0,"method":"independent_t","roles":{"outcome":["score"],"group":["arm"]},"levels":{"group":["1","2"]}}]')
add_truth("c05_t_ties_int", 0, "independent_t", "student_pooled", tt$statistic, tt$parameter, NA, tt$p.value)
## 6 paired, 2 decimals, one missing pair
x <- rt(rnorm(12, 50, 5), 2); y <- rt(x + rnorm(12, 1.5, 2), 2); y[5] <- NA
tt <- t.test(x, y, paired = TRUE)
write_case("c06_paired_2dec", data.frame(pre = num_str(x, 2), post = num_str(y, 2)),
  list(tsent(tt$statistic, tt$parameter, tt$p.value)),
  '[{"claim":0,"method":"paired_t","roles":{"first":["pre"],"second":["post"]}}]')
add_truth("c06_paired_2dec", 0, "paired_t", "paired", tt$statistic, tt$parameter, NA, tt$p.value)
## 7 paired, integers with ties, n = 9
x <- c(3, 4, 4, 5, 2, 6, 3, 4, 5); y <- c(4, 6, 5, 5, 4, 7, 5, 4, 7)
tt <- t.test(x, y, paired = TRUE)
write_case("c07_paired_int", data.frame(t1 = x, t2 = y),
  list(tsent(tt$statistic, tt$parameter, tt$p.value)),
  '[{"claim":0,"method":"paired_t","roles":{"first":["t1"],"second":["t2"]}}]')
add_truth("c07_paired_int", 0, "paired_t", "paired", tt$statistic, tt$parameter, NA, tt$p.value)
## 8 one-way ANOVA, 3 groups unequal n, 1 decimal, 2 missing outcomes
y <- rt(c(rnorm(7, 10, 2), rnorm(12, 11, 2), rnorm(9, 13, 2)), 1); g <- rep(c("p", "q", "r"), c(7, 12, 9)); y[c(3, 15)] <- NA
s <- summary(aov(y ~ factor(g)))[[1]]
write_case("c08_anova3_1dec", data.frame(y = num_str(y, 1), g = g),
  list(sprintf("The groups differed, F(%d, %d) = %s, p = %s.", s$Df[1], s$Df[2], fmt(s$`F value`[1]), fmtp(s$`Pr(>F)`[1]))),
  '[{"claim":0,"method":"one_way_anova","roles":{"outcome":["y"],"group":["g"]},"levels":{"group":["p","q","r"]}}]')
add_truth("c08_anova3_1dec", 0, "one_way_anova", "one_way", s$`F value`[1], s$Df[1], s$Df[2], s$`Pr(>F)`[1])
## 9 ANOVA 5 groups, 4 decimals
y <- rt(rnorm(40, rep(c(1, 1.2, 1.1, 1.6, 1.3), each = 8), 0.4), 4); g <- rep(paste0("L", 1:5), each = 8)
s <- summary(aov(y ~ factor(g)))[[1]]
write_case("c09_anova5_4dec", data.frame(val = num_str(y, 4), lvl = g),
  list(sprintf("The groups differed, F(%d, %d) = %s, p = %s.", s$Df[1], s$Df[2], fmt(s$`F value`[1]), fmtp(s$`Pr(>F)`[1]))),
  '[{"claim":0,"method":"one_way_anova","roles":{"outcome":["val"],"group":["lvl"]},"levels":{"group":["L1","L2","L3","L4","L5"]}}]')
add_truth("c09_anova5_4dec", 0, "one_way_anova", "one_way", s$`F value`[1], s$Df[1], s$Df[2], s$`Pr(>F)`[1])
## 10 ANOVA with a group of 2, whole numbers, integer-coded group
y <- c(5, 7, sample(3:9, 10, TRUE), sample(5:12, 9, TRUE)); g <- rep(c(1, 2, 3), c(2, 10, 9))
s <- summary(aov(y ~ factor(g)))[[1]]
write_case("c10_anova_group2_int", data.frame(y = y, cond = g),
  list(sprintf("The groups differed, F(%d, %d) = %s, p = %s.", s$Df[1], s$Df[2], fmt(s$`F value`[1]), fmtp(s$`Pr(>F)`[1]))),
  '[{"claim":0,"method":"one_way_anova","roles":{"outcome":["y"],"group":["cond"]},"levels":{"group":["1","2","3"]}}]')
add_truth("c10_anova_group2_int", 0, "one_way_anova", "one_way", s$`F value`[1], s$Df[1], s$Df[2], s$`Pr(>F)`[1])
rsent <- function(r, df, p) sprintf("The variables were associated, r(%d) = %s, p = %s.", df, sub("^(-?)0\\.", "\\1.", fmt(r)), fmtp(p))
## 11 Pearson negative, 2 decimals
x <- rt(rnorm(25), 2); y <- rt(-0.6 * x + rnorm(25, 0, 0.8), 2)
ct <- cor.test(x, y)
write_case("c11_pearson_neg_2dec", data.frame(xa = num_str(x, 2), yb = num_str(y, 2)),
  list(rsent(ct$estimate, ct$parameter, ct$p.value)),
  '[{"claim":0,"method":"pearson","roles":{"x":["xa"],"y":["yb"]}}]')
add_truth("c11_pearson_neg_2dec", 0, "pearson", "pearson", ct$estimate, ct$parameter, NA, ct$p.value)
## 12 Pearson positive, integers, n = 40
x <- sample(1:10, 40, TRUE); y <- x + sample(-12:12, 40, TRUE)
ct <- cor.test(x, y)
write_case("c12_pearson_int", data.frame(u = x, v = y),
  list(rsent(ct$estimate, ct$parameter, ct$p.value)),
  '[{"claim":0,"method":"pearson","roles":{"x":["u"],"y":["v"]}}]')
add_truth("c12_pearson_int", 0, "pearson", "pearson", ct$estimate, ct$parameter, NA, ct$p.value)
## 13 Spearman, ties, negative, integers
x <- sample(1:5, 30, TRUE); y <- 6 - x + sample(-4:4, 30, TRUE)
ct <- suppressWarnings(cor.test(x, y, method = "spearman", exact = FALSE))
r <- unname(ct$estimate); tstat <- r * sqrt(28 / (1 - r^2))
write_case("c13_spearman_ties_neg", data.frame(a = x, b = y),
  list(rsent(r, 28, ct$p.value)),
  '[{"claim":0,"method":"spearman","roles":{"x":["a"],"y":["b"]}}]')
add_truth("c13_spearman_ties_neg", 0, "spearman", "spearman_t_approximation", r, 28, NA, ct$p.value)
## 14 Spearman, no ties, 1 decimal... use 4 decimals to avoid ties; n = 30
x <- rt(runif(30, 0, 100), 4); y <- rt(x + rnorm(30, 0, 70), 4)
stopifnot(!any(duplicated(x)), !any(duplicated(y)))
ct <- cor.test(x, y, method = "spearman", exact = FALSE)
r <- unname(ct$estimate)
write_case("c14_spearman_noties", data.frame(a = num_str(x, 4), b = num_str(y, 4)),
  list(rsent(r, 28, ct$p.value)),
  '[{"claim":0,"method":"spearman","roles":{"x":["a"],"y":["b"]}}]')
add_truth("c14_spearman_noties", 0, "spearman", "spearman_t_approximation", r, 28, NA, ct$p.value)
csent <- function(x2, df, p) sprintf("The association was, χ2(%d) = %s, p = %s.", df, fmt(x2), fmtp(p))
## 15 chi 2x2, both variants printed as two claims
r1 <- rep(c("yes", "no"), c(30, 25)); c1 <- c(rep(c("m", "f"), c(19, 11)), rep(c("m", "f"), c(9, 16)))
u <- chisq.test(table(r1, c1), correct = FALSE); y <- chisq.test(table(r1, c1), correct = TRUE)
write_case("c15_chi2x2_both", data.frame(resp = r1, sex = c1),
  list(csent(u$statistic, 1, u$p.value), csent(y$statistic, 1, y$p.value)),
  '[{"claim":0,"method":"chi_square_independence","roles":{"row_variable":["resp"],"column_variable":["sex"]},"levels":{"row_variable":["yes","no"],"column_variable":["m","f"]}},{"claim":1,"method":"chi_square_independence","roles":{"row_variable":["resp"],"column_variable":["sex"]},"levels":{"row_variable":["yes","no"],"column_variable":["m","f"]}}]')
add_truth("c15_chi2x2_both", 0, "chi_square_independence", "chi_square_uncorrected", u$statistic, 1, NA, u$p.value)
add_truth("c15_chi2x2_both", 1, "chi_square_independence", "chi_square_yates", y$statistic, 1, NA, y$p.value)
## 16 chi 2x2, small counts, Yates
r1 <- rep(c("a", "b"), c(9, 8)); c1 <- c(rep(c("p", "q"), c(7, 2)), rep(c("p", "q"), c(2, 6)))
y <- suppressWarnings(chisq.test(table(r1, c1), correct = TRUE))
write_case("c16_chi2x2_small_yates", data.frame(rv = r1, cv = c1),
  list(csent(y$statistic, 1, y$p.value)),
  '[{"claim":0,"method":"chi_square_independence","roles":{"row_variable":["rv"],"column_variable":["cv"]},"levels":{"row_variable":["a","b"],"column_variable":["p","q"]}}]')
add_truth("c16_chi2x2_small_yates", 0, "chi_square_independence", "chi_square_yates", y$statistic, 1, NA, y$p.value)
## 17 chi 3x4
r1 <- sample(c("lo", "mid", "hi"), 120, TRUE, prob = c(.3, .4, .3)); c1 <- ifelse(r1 == "hi", sample(c("w", "x", "y", "z"), 120, TRUE, prob = c(.1, .2, .3, .4)), sample(c("w", "x", "y", "z"), 120, TRUE))
u <- chisq.test(table(r1, c1))
write_case("c17_chi3x4", data.frame(band = r1, cat = c1),
  list(csent(u$statistic, u$parameter, u$p.value)),
  '[{"claim":0,"method":"chi_square_independence","roles":{"row_variable":["band"],"column_variable":["cat"]},"levels":{"row_variable":["hi","lo","mid"],"column_variable":["w","x","y","z"]}}]')
add_truth("c17_chi3x4", 0, "chi_square_independence", "chi_square_uncorrected", u$statistic, u$parameter, NA, u$p.value)
## 18 OLS one predictor, 2 decimals, a missing predictor: overall F and coefficient t
x <- rt(rnorm(20, 5, 2), 2); y <- rt(2 + 0.4 * x + rnorm(20, 0, 1), 2); x[7] <- NA
m <- summary(lm(y ~ x)); f <- m$fstatistic; pf_ <- pf(f[1], f[2], f[3], lower.tail = FALSE); co <- m$coefficients["x", ]
write_case("c18_ols1_2dec", data.frame(y = num_str(y, 2), x = num_str(x, 2)),
  list(sprintf("The model fit, F(%d, %d) = %s, p = %s.", f[2], f[3], fmt(f[1]), fmtp(pf_)), tsent(co[3], f[3], co[4])),
  '[{"claim":0,"method":"ols","roles":{"outcome":["y"],"predictors":["x"]},"ols_overall_f":true},{"claim":1,"method":"ols","roles":{"outcome":["y"],"predictors":["x"]},"ols_predictor":"x"}]')
add_truth("c18_ols1_2dec", 0, "ols", "ols_overall_f", f[1], f[2], f[3], pf_)
add_truth("c18_ols1_2dec", 1, "ols", "ols_coefficient", co[3], f[3], NA, co[4])
## 19 OLS three predictors, near-collinear, 4 decimals
x1 <- rt(rnorm(30), 4); x2 <- rt(rnorm(30), 4); x3 <- rt(x1 + x2 + rnorm(30, 0, 0.01), 4); y <- rt(1 + 0.3 * x1 - 0.2 * x2 + 0.2 * x3 + rnorm(30, 0, 1.5), 4)
m <- summary(lm(y ~ x1 + x2 + x3)); f <- m$fstatistic; pf_ <- pf(f[1], f[2], f[3], lower.tail = FALSE); co <- m$coefficients["x3", ]
cat("c19 kappa:", kappa(cbind(1, x1, x2, x3), exact = TRUE), "\n")
write_case("c19_ols3_collinear", data.frame(y = num_str(y, 4), x1 = num_str(x1, 4), x2 = num_str(x2, 4), x3 = num_str(x3, 4)),
  list(sprintf("The model fit, F(%d, %d) = %s, p = %s.", f[2], f[3], fmt(f[1]), fmtp(pf_)), tsent(co[3], f[3], co[4])),
  '[{"claim":0,"method":"ols","roles":{"outcome":["y"],"predictors":["x1","x2","x3"]},"ols_overall_f":true},{"claim":1,"method":"ols","roles":{"outcome":["y"],"predictors":["x1","x2","x3"]},"ols_predictor":"x3"}]')
add_truth("c19_ols3_collinear", 0, "ols", "ols_overall_f", f[1], f[2], f[3], pf_)
add_truth("c19_ols3_collinear", 1, "ols", "ols_coefficient_x3", co[3], f[3], NA, co[4])
## 20 OLS two predictors, integers, negative coefficient
x1 <- sample(1:9, 35, TRUE); x2 <- sample(0:4, 35, TRUE); y <- 20 - 1.2 * x1 + x2 + sample(-4:4, 35, TRUE)
m <- summary(lm(y ~ x1 + x2)); f <- m$fstatistic; pf_ <- pf(f[1], f[2], f[3], lower.tail = FALSE); co <- m$coefficients["x1", ]
write_case("c20_ols2_int_neg", data.frame(y = y, x1 = x1, x2 = x2),
  list(sprintf("The model fit, F(%d, %d) = %s, p = %s.", f[2], f[3], fmt(f[1]), fmtp(pf_)), tsent(co[3], f[3], co[4])),
  '[{"claim":0,"method":"ols","roles":{"outcome":["y"],"predictors":["x1","x2"]},"ols_overall_f":true},{"claim":1,"method":"ols","roles":{"outcome":["y"],"predictors":["x1","x2"]},"ols_predictor":"x1"}]')
add_truth("c20_ols2_int_neg", 0, "ols", "ols_overall_f", f[1], f[2], f[3], pf_)
add_truth("c20_ols2_int_neg", 1, "ols", "ols_coefficient_x1", co[3], f[3], NA, co[4])
