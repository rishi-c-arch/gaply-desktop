# The precision sweep's two witness datasets (tests/l1_precision_sweep.rs,
# tests/l1_precision_witness.rs). First written for the joint evaluation of
# the Statistical Analysis Verifier (15 Oct 2026); copied here so the
# repository can rebuild its fixtures. Run from this folder:
#   /usr/local/bin/Rscript precision_witness.R
# The stored files are fixed, hand-chosen stress values: a one-way ANOVA
# whose within-group spread is about the rounding step, and a Pearson r near
# 1 at n = 6, both stored to 1 decimal. For each, the script then finds
# unrounded cells (seed 7) that round to EXACTLY the stored file, and prints
# their statistic: the witness quoted in tests/l1_precision_witness.rs.
HDR <- "written by /usr/local/bin/Rscript precision_witness.R (src-tauri/gaply-core/tests/fixtures/stats_l1/precision_sweep/)"
stored <- function(name, df) {
  f <- paste0(name, ".csv")
  writeLines(c(paste0("# The precision sweep's witness ", name, " (the stored file; synthetic),"), paste0("# ", HDR, ".")), f)
  suppressWarnings(write.table(df, f, sep = ",", append = TRUE, row.names = FALSE, quote = FALSE))
}
stored("s1_anova_tiny_ssw", data.frame(y = c("1.0", "1.1", "1.0", "1.2", "1.1", "1.2", "1.1", "1.3", "1.2"), g = rep(c("a", "b", "c"), each = 3)))
stored("s2_pearson_near1", data.frame(xa = c("1.0", "1.1", "1.3", "1.4", "1.6", "1.7"), yb = c("2.0", "2.2", "2.5", "2.8", "3.2", "3.3")))
set.seed(7)
# s1: stored y (1 dec), three groups of 3
ys <- c(1.0, 1.1, 1.0, 1.2, 1.1, 1.2, 1.1, 1.3, 1.2); g <- factor(rep(c("a", "b", "c"), each = 3))
best <- NULL; bF <- -Inf
for (i in 1:20000) { y <- ys + runif(9, -0.0499, 0.0499); F <- summary(aov(y ~ g))[[1]]$`F value`[1]; if (F > bF) { bF <- F; best <- y } }
stopifnot(all(sprintf("%.1f", best) == sprintf("%.1f", ys)))
s <- summary(aov(best ~ g))[[1]]
cat(sprintf("s1 unrounded F = %.6f, df = %d, %d, p = %.6f; cells round to the stored file: TRUE\n", s$`F value`[1], s$Df[1], s$Df[2], s$`Pr(>F)`[1]))
# s2: stored x, y (1 dec), n = 6
xs <- c(1.0, 1.1, 1.3, 1.4, 1.6, 1.7); yv <- c(2.0, 2.2, 2.5, 2.8, 3.2, 3.3)
best <- NULL; br <- Inf
for (i in 1:20000) { x <- xs + runif(6, -0.0499, 0.0499); y <- yv + runif(6, -0.0499, 0.0499); r <- cor(x, y); if (r < br) { br <- r; best <- cbind(x, y) } }
stopifnot(all(sprintf("%.1f", best[, 1]) == sprintf("%.1f", xs)), all(sprintf("%.1f", best[, 2]) == sprintf("%.1f", yv)))
ct <- cor.test(best[, 1], best[, 2])
cat(sprintf("s2 unrounded r = %.6f, df = 4, p = %.6f; cells round to the stored file: TRUE\n", ct$estimate, ct$p.value))
