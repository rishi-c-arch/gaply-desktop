# Phase 2 evaluation fixes, part C (B-1): fixtures for whole-number columns.
# Run with /usr/local/bin/Rscript (R 4.6.1):
#   /usr/local/bin/Rscript make_whole_numbers.R <eval3 cases_l1_scale.jsonl> <out dir>
#
# rt_*: three cases of the deep evaluation's own scale generator
# (~/gaply-corpus/eval3/s3/gen_l1_scale.R, seed 161026): reaction times in ms,
# R computed on the unrounded values, the file stores them as whole numbers.
# Each was a false "Not reproduced" at 5163aec. Copied verbatim (rows and the
# printed numbers); R is re-run here on the STORED whole numbers so the header
# shows what the file alone gives.
#
# likert_*: 7-point items, 12 per arm, the first seed from 20261017 with a
# moderate t (named in the fixture), written here. R computes
# on exactly the stored integers (they are exact). The printed t carries a
# planted last-digit typo (+0.01): with "Exact whole numbers" it must stay a
# catch.
suppressMessages(library(jsonlite))
args <- commandArgs(trailingOnly = TRUE)
cases <- stream_in(file(args[1]), verbose = FALSE)
out <- args[2]
hdr <- "# /usr/local/bin/Rscript (R 4.6.1), make_whole_numbers.R"
write_case <- function(name, headers, rows, lines) {
  con <- file(file.path(out, paste0(name, ".csv")), "w")
  writeLines(c(hdr, paste0("# ", lines), paste(headers, collapse = ",")), con)
  for (r in rows) writeLines(paste(r, collapse = ","), con)
  close(con)
}
for (id in c("s00186", "s00418", "s00234")) {
  c <- cases[cases$id == id, ]
  rows <- c$rows[[1]]; m <- c$method_key; hd <- c$headers[[1]]
  num <- function(j) as.numeric(rows[, j])
  if (m == "indt") {
    y <- num(1); g <- rows[, 2]
    tt <- t.test(y[g == "a"], y[g == "b"], var.equal = TRUE)
    on_file <- sprintf("t = %.6f, df = %d, p = %.6f", tt$statistic, tt$parameter, tt$p.value)
  } else if (m == "paired") {
    tt <- t.test(num(1), num(2), paired = TRUE)
    on_file <- sprintf("t = %.6f, df = %d, p = %.6f", tt$statistic, tt$parameter, tt$p.value)
  } else {
    ct <- cor.test(num(1), num(2))
    on_file <- sprintf("r = %.6f, df = %d, p = %.6f", ct$estimate, ct$parameter, ct$p.value)
  }
  printed <- sprintf("%s(%s) = %s, p %s %s", c$family, paste(unlist(c$df), collapse = ", "), c$stat, c$prel, c$p)
  write_case(paste0("rt_", m, "_", id), hd, lapply(seq_len(nrow(rows)), function(i) rows[i, ]),
             c(paste0("eval3 scale case ", id, " (", m, "); printed, from R on the unrounded values: ", printed),
               paste0("R on the stored whole numbers: ", on_file)))
}
# The first seed from 20261017 up whose t lies between 1.5 and 3.
for (seed in 20261017:20261116) {
  set.seed(seed)
  a <- sample(1:7, 12, replace = TRUE, prob = c(1, 2, 3, 4, 3, 2, 1))
  b <- sample(1:7, 12, replace = TRUE, prob = c(1, 1, 2, 3, 4, 3, 2))
  tt <- t.test(a, b, var.equal = TRUE)
  if (abs(tt$statistic) >= 1.5 && abs(tt$statistic) <= 3) break
}
honest <- sprintf("%.2f", abs(tt$statistic))
typo <- sprintf("%.2f", abs(tt$statistic) + 0.01)
pp <- sub("^0\\.", ".", sprintf("%.3f", tt$p.value))
write_case("likert_indt_typo", c("item", "arm"), c(lapply(a, function(v) c(v, "a")), lapply(b, function(v) c(v, "b"))),
           c(sprintf("seed %d; R on the stored items: t = %.6f, df = %d, p = %.6f", seed, tt$statistic, tt$parameter, tt$p.value),
             sprintf("honest print: t(22) = %s, p = %s; planted typo print: t(22) = %s, p = %s", honest, pp, typo, pp)))
cat("written\n")
