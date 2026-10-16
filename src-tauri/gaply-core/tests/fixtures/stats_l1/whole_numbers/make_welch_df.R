# Phase 2 evaluation fixes, part C (B-1 re-run): two cases of the deep
# evaluation's scenario B (~/gaply-corpus/eval3/s3/gen_l1.R, seed 20261016;
# R's Welch t on unrounded values, the file stores whole numbers) that stayed
# false "Not reproduced" after the whole-number rule: Welch's df range was
# unbounded, and the printed df was still compared strictly.
# Run with /usr/local/bin/Rscript (R 4.6.1):
#   /usr/local/bin/Rscript make_welch_df.R <eval3 s3/cases_l1.jsonl> <out dir>
suppressMessages(library(jsonlite))
args <- commandArgs(trailingOnly = TRUE)
cases <- stream_in(file(args[1]), verbose = FALSE)
for (id in c("c005134", "c005115")) {
  c <- cases[cases$id == id, ]; rows <- c$rows[[1]]
  y <- as.numeric(rows[, 1]); g <- rows[, 2]
  we <- t.test(y[g == "ctl"], y[g == "trt"], var.equal = FALSE)
  con <- file(file.path(args[2], paste0("welch_df_", id, ".csv")), "w")
  writeLines(c("# /usr/local/bin/Rscript (R 4.6.1), make_welch_df.R",
               sprintf("# eval3 s3 case %s; printed, from R on the unrounded values: t(%s) = %s, p %s %s", id, unlist(c$df), c$stat, c$prel, c$p),
               sprintf("# R's Welch on the stored whole numbers: t = %.6f, df = %.6f, p = %.6f", we$statistic, we$parameter, we$p.value),
               paste(c$headers[[1]], collapse = ",")), con)
  for (i in seq_len(nrow(rows))) writeLines(paste(rows[i, ], collapse = ","), con)
  close(con)
}
cat("written\n")
