# statcheck as a second opinion on a Level 0 corpus run (Phase 1, Step 7).
#
# Run with the R that has statcheck installed, by its full path:
#
#   /usr/local/bin/Rscript compare_statcheck.R --set NAME --tsv FILE.tsv \
#       --text-dir DIR --csv statcheck.csv [--private]
#
# A reference for measurement only: never called by the product, never bundled.
#
# What it does, in order:
#
# 1. For each manuscript in the TSV (written by the l0_corpus_run example), runs
#    statcheck::statcheck, with its default arguments, on DIR/<sha12>.txt: the
#    text Gaply's extractor read. Every run of whitespace is collapsed to one
#    space first. PDF text arrives with double spaces ("F(1,  126)") that
#    statcheck's patterns do not accept, so without this statcheck misses units
#    for a reason that has nothing to do with statistics. Both counts, as
#    dumped and collapsed, are printed per manuscript.
# 2. Writes every statcheck result to --csv, replacing the rows of this set
#    and keeping other sets'. The statcheck columns are read from the result
#    object, not assumed, plus set, sha12 and statcheck_version.
# 3. Joins the two on the printed unit. Both sides are normalised the same way
#    the runner's in_xml check is: every whitespace character removed, every
#    minus sign and dash made "-", "²" made "2". A statcheck unit matches a
#    Gaply unit of the same manuscript when it equals it or is a prefix of it:
#    Gaply's unit carries an effect size or interval after the p, and
#    statcheck's stops at the p. Leading brackets on statcheck's unit are
#    dropped and it may start up to 3 characters in, because statcheck's raw
#    drops the chi symbol. One to one, in document order.
# 4. Adds statcheck_read, statcheck_error and statcheck_decision_error to the
#    TSV (before label) and rewrites it, keeping its row order.
# 5. Prints four counts: read by both and agreeing, read by both and
#    disagreeing, Gaply only, statcheck only. Agreement compares Gaply's
#    p_consistency verdict with statcheck's error flag: Consistent (with any
#    note) agrees with FALSE, Inconsistent with TRUE, and nothing else agrees.
#    Each disagreeing row gets exactly one reason from a closed list; a row
#    none explains is UNEXPLAINED and is listed.
#
# --private (a researcher's own manuscripts): prints counts only. No unit,
# reported p or statcheck output is printed for any row.
#
# statcheck is a second opinion, not the answer key. Nothing here changes a
# Gaply verdict.

suppressPackageStartupMessages(library(statcheck))

argv <- commandArgs(trailingOnly = TRUE)
opt <- function(name) {
  i <- match(name, argv)
  if (is.na(i) || i == length(argv)) stop("missing ", name)
  argv[i + 1]
}
set_name <- opt("--set")
tsv_path <- opt("--tsv")
text_dir <- opt("--text-dir")
csv_path <- opt("--csv")
private <- "--private" %in% argv
version <- as.character(packageVersion("statcheck"))
cat("command: /usr/local/bin/Rscript compare_statcheck.R", paste(shQuote(argv), collapse = " "), "\n")
cat("R:", R.version.string, " statcheck:", version, " private:", private, "\n")

norm <- function(x) {
  x <- gsub("(*UCP)\\s", "", x, perl = TRUE)
  x <- gsub("[−‐-―﹣－]", "-", x)
  gsub("²", "2", x)
}

tsv <- read.delim(tsv_path, colClasses = "character", quote = "", comment.char = "",
                  na.strings = character(0), check.names = FALSE, encoding = "UTF-8")
tsv <- tsv[, setdiff(names(tsv), c("statcheck_read", "statcheck_error", "statcheck_decision_error")), drop = FALSE]
stopifnot(all(c("sha12", "claim", "raw_unit", "reasons", "alpha", "label") %in% names(tsv)))

# Manuscripts come from the TSV's rows, so a manuscript with no Gaply claim is
# not here. Every sha12 the runner dumped for this set is passed by the caller
# through --sha12 when that matters; by default it is the TSV's.
shas <- if ("--sha12" %in% argv) strsplit(opt("--sha12"), ",")[[1]] else unique(tsv$sha12)

results <- list()
for (s in shas) {
  f <- file.path(text_dir, paste0(s, ".txt"))
  if (!file.exists(f)) stop("no dumped text for ", s)
  txt <- paste(readLines(f, warn = FALSE, encoding = "UTF-8"), collapse = "\n")
  as_dumped <- statcheck(setNames(txt, s), messages = FALSE)
  r <- statcheck(setNames(gsub("(*UCP)\\s+", " ", txt, perl = TRUE), s), messages = FALSE)
  cat(sprintf("statcheck %s as_dumped=%d whitespace_collapsed=%d\n", s,
              if (is.null(as_dumped)) 0L else nrow(as_dumped), if (is.null(r)) 0L else nrow(r)))
  if (!is.null(r) && nrow(r) > 0) {
    r <- as.data.frame(r)
    r$sha12 <- s
    results[[s]] <- r
  }
}
sc <- if (length(results)) do.call(rbind, results) else NULL
if (!is.null(sc)) {
  rownames(sc) <- NULL
  sc$set <- set_name
  sc$statcheck_version <- version
  sc$matched <- FALSE
}

# Keep the other sets' rows in the CSV.
if (file.exists(csv_path)) {
  old <- read.csv(csv_path, colClasses = "character", check.names = FALSE, encoding = "UTF-8")
  old <- old[old$set != set_name, , drop = FALSE]
} else {
  old <- NULL
}
out_sc <- if (is.null(sc)) NULL else sc[, setdiff(names(sc), "matched")]
if (!is.null(old) && nrow(old) && !is.null(out_sc)) {
  out_sc[] <- lapply(out_sc, as.character)
  all_cols <- union(names(old), names(out_sc))
  for (c in setdiff(all_cols, names(old))) old[[c]] <- ""
  for (c in setdiff(all_cols, names(out_sc))) out_sc[[c]] <- ""
  out_sc <- rbind(old[, all_cols], out_sc[, all_cols])
} else if (is.null(out_sc)) {
  out_sc <- old
}
if (!is.null(out_sc)) write.csv(out_sc, csv_path, row.names = FALSE, fileEncoding = "UTF-8")

# The join. statcheck's `raw` can start with the bracket that opened it
# ("(z = 17.886, ...") and drops a chi symbol ("2 (1) = 1.61, ..."), so its
# leading brackets are removed and it may start up to 3 characters into
# Gaply's unit: the width of "χ" or "chi". It must then run to the end of
# statcheck's unit, so it is still a prefix of Gaply's unit after that offset.
unit_matches <- function(g, s) {
  s <- sub("^[(\\[]+", "", norm(s))
  if (!nzchar(s)) return(FALSE)
  for (k in 0:3) if (startsWith(substring(g, k + 1), s)) return(TRUE)
  FALSE
}
tsv$statcheck_read <- "no"
tsv$statcheck_error <- ""
tsv$statcheck_decision_error <- ""
sc_row <- rep(NA_integer_, nrow(tsv))
order_in_doc <- order(tsv$sha12, as.integer(tsv$claim))
for (i in order_in_doc) {
  if (is.null(sc)) break
  g <- norm(tsv$raw_unit[i])
  cand <- which(sc$sha12 == tsv$sha12[i] & !sc$matched)
  hit <- cand[vapply(cand, function(j) unit_matches(g, sc$raw[j]), logical(1))]
  if (length(hit)) {
    j <- hit[1]
    sc$matched[j] <- TRUE
    sc_row[i] <- j
    tsv$statcheck_read[i] <- "yes"
    tsv$statcheck_error[i] <- as.character(sc$error[j])
    tsv$statcheck_decision_error[i] <- as.character(sc$decision_error[j])
  }
}

p_verdict <- sub(".*p_consistency:([^;]*).*", "\\1", tsv$reasons)
p_verdict[!grepl("p_consistency:", tsv$reasons)] <- ""
both <- tsv$statcheck_read == "yes"
agree <- both & (
  (startsWith(p_verdict, "Consistent") & tsv$statcheck_error == "FALSE") |
  (p_verdict == "Inconsistent" & tsv$statcheck_error == "TRUE")
)
disagree <- both & !agree

reason_of <- function(i) {
  v <- p_verdict[i]
  j <- sc_row[i]
  err <- tsv$statcheck_error[i] == "TRUE"
  p_comp <- as.character(sc$p_comp[j])
  reported <- suppressWarnings(as.numeric(sc$reported_p[j]))
  alphas <- strsplit(sub(" \\(default\\)$", "", tsv$alpha[i]), "|", fixed = TRUE)[[1]]
  if (v == "CouldNotCheck(AdjustedP)") return("ADJUSTED_P")
  if (v == "Concern(FTwoTailed)") return("TWO_TAILED_F")
  if (v %in% c("Concern(OneTailedUnstated)", "Consistent(OneTailedStated)")) return("ONE_TAILED")
  if (v == "Consistent(FloorPrintedAsEquality)" && err) return("FLOOR")
  if (identical(p_comp, "=") && isTRUE(reported == 0)) return("P_ZERO")
  if (identical(p_comp, "=") && isTRUE(abs(reported - 0.05) < 1e-12)) return("P_EQUALS_ALPHA")
  if (identical(p_comp, "ns") && !identical(alphas, "0.05")) return("ALPHA")
  "UNEXPLAINED"
}
reasons <- vapply(which(disagree), reason_of, character(1))

cat(sprintf("\n== SET %s\n", set_name))
cat(sprintf("   read by both and agreeing     %d\n", sum(agree)))
cat(sprintf("   read by both and disagreeing  %d\n", sum(disagree)))
cat(sprintf("   read by Gaply only            %d\n", sum(!both)))
cat(sprintf("   read by statcheck only        %d\n", if (is.null(sc)) 0L else sum(!sc$matched)))
if (length(reasons)) {
  tab <- table(reasons)
  cat("   disagreement reasons:", paste(names(tab), tab, sep = "=", collapse = " "), "\n")
} else {
  cat("   disagreement reasons: none\n")
}

list_rows <- function(idx, label) {
  for (i in idx) {
    j <- sc_row[i]
    if (private) {
      cat(sprintf("   %s %s claim=%s gaply=%s statcheck_error=%s\n", label, tsv$sha12[i], tsv$claim[i],
                  p_verdict[i], tsv$statcheck_error[i]))
    } else {
      cat(sprintf("   %s %s claim=%s gaply=%s | statcheck error=%s decision_error=%s reported=%s%s computed=%s | %s\n",
                  label, tsv$file[i], tsv$claim[i], p_verdict[i], tsv$statcheck_error[i],
                  tsv$statcheck_decision_error[i], sc$p_comp[j], sc$reported_p[j],
                  signif(as.numeric(sc$computed_p[j]), 4), tsv$raw_unit[i]))
    }
  }
}
dis_idx <- which(disagree)
list_rows(dis_idx[reasons == "UNEXPLAINED"], "UNEXPLAINED")
if (!private) {
  list_rows(dis_idx[reasons != "UNEXPLAINED"], "explained")
  if (!is.null(sc) && any(!sc$matched)) {
    for (j in which(!sc$matched)) {
      cat(sprintf("   statcheck_only %s error=%s | %s\n", sc$sha12[j], sc$error[j], sc$raw[j]))
    }
  }
}

cols <- names(tsv)
cols <- c(setdiff(cols, c("statcheck_read", "statcheck_error", "statcheck_decision_error", "label")),
          "statcheck_read", "statcheck_error", "statcheck_decision_error", "label")
write.table(tsv[, cols], tsv_path, sep = "\t", quote = FALSE, row.names = FALSE, na = "", fileEncoding = "UTF-8")
cat(sprintf("   rewrote %s (%d rows); statcheck rows for this set: %d\n", tsv_path, nrow(tsv),
            if (is.null(sc)) 0L else nrow(sc)))
