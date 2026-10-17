# Phase 2 evaluation fixes, part C (m-10): data with a 1e9 offset and SD 10.
# Run with /usr/local/bin/Rscript (R 4.6.1):
#   /usr/local/bin/Rscript make_offset.R <eval3 s4/cases.jsonl> <out dir>
# Copies the deep evaluation's smallest "offset" datasets for Student's t,
# Welch's t and the one-way ANOVA (~/gaply-corpus/eval3/s4/gen_diff.R, seed
# 4444), with R's statistic, df and p AS THAT GENERATOR RECORDED THEM: computed
# by t.test / oneway.test on the generated doubles and written with %.17g,
# the cells written with 17 significant digits, which encode those doubles
# exactly.
#
# Why R is not re-run on the written cells here: on this build (aarch64, no
# long double) R's own text-to-number conversion is not correctly rounded at
# 17 digits. Measured: as.numeric("1000000035.5230229") prints back as
# 1000000035.5230228, one unit in the last place off, while a correctly
# rounded reader (Rust's, Python's) returns the generated double. A re-read
# in R moves the t by about 1e-8 relative and is not the reference.
suppressMessages(library(jsonlite))
args <- commandArgs(trailingOnly = TRUE)
cases <- stream_in(file(args[1]), verbose = FALSE)
for (key in list(c("independent_t", "student"), c("independent_t", "welch"), c("one_way_anova", "oneway"))) {
  s <- cases[cases$kind == "offset" & cases$method == key[1] & cases$truth_variant == key[2] & !cases$r_error, ]
  c <- s[which.min(vapply(s$rows, nrow, 0L)), ]
  rows <- c$rows[[1]]
  df <- paste(unlist(c$r_df), collapse = ", ")
  con <- file(file.path(args[2], paste0(key[1], "_", key[2], "_", c$id, ".csv")), "w")
  writeLines(c("# /usr/local/bin/Rscript (R 4.6.1), make_offset.R",
               paste0("# eval3 s4 case ", c$id, " (offset, ", key[1], ", ", key[2], ")"),
               paste0("# R (gen_diff.R, on the generated doubles): stat = ", c$r_stat, ", df = ", df, ", p = ", c$r_p),
               paste(c$headers[[1]], collapse = ",")), con)
  for (i in seq_len(nrow(rows))) writeLines(paste(trimws(rows[i, ]), collapse = ","), con)
  close(con)
}
cat("written\n")
