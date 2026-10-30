// Gaply — every refusal the file ingest can give the descriptive summary's
// and the t-test's screens (Research OS, Phase 8B, decision 243), in the shape
// stats_desc_open throws it: each L1IngestRefusal variant as
// gaply-core/src/stats_l1/codes.rs's l1_ingest_refusal writes it, with
// too_wide and no_columns nested in dataset, and both cases of
// archive_too_large and of too_large. The screens' tests render each one.
// Phase 8F-1 (decision 282) adds too_many_cells, the cell cap's refusal.
// Phase 8F-3 (decision 295) adds sav_big_endian, a big-endian .sav's.
export const INGEST_REFUSALS: Array<{ name: string; code: Record<string, unknown> }> = [
  { name: 'not_readable', code: { code: 'not_readable' } },
  { name: 'too_large, the ingest cap', code: { code: 'too_large', bytes: 10_485_761, limit: 10_485_760 } },
  { name: 'too_large, import_guard', code: { code: 'too_large', bytes: 104_857_601, limit: 104_857_600 } },
  { name: 'unsupported_format', code: { code: 'unsupported_format' } },
  { name: 'malformed', code: { code: 'malformed' } },
  { name: 'no_table', code: { code: 'no_table' } },
  { name: 'file_too_large', code: { code: 'file_too_large', limit_rows: 5000 } },
  { name: 'file_changed_while_reading', code: { code: 'file_changed_while_reading' } },
  { name: 'xls_not_readable', code: { code: 'xls_not_readable' } },
  { name: 'dataset too_wide', code: { code: 'dataset', refusal: { code: 'too_wide', columns: 129, limit: 128 } } },
  { name: 'dataset no_columns', code: { code: 'dataset', refusal: { code: 'no_columns' } } },
  { name: 'not_a_regular_file', code: { code: 'not_a_regular_file' } },
  { name: 'archive_too_large, over the limit', code: { code: 'archive_too_large', declared: 1_572_865_485, limit: 67_108_864 } },
  { name: 'archive_too_large, past its own list', code: { code: 'archive_too_large', declared: 4097, limit: 67_108_864 } },
  { name: 'content_too_large', code: { code: 'content_too_large', limit_bytes: 2_097_152 } },
  { name: 'encoding_not_utf8', code: { code: 'encoding_not_utf8' } },
  { name: 'too_many_cells', code: { code: 'too_many_cells', limit_cells: 5_000_000 } },
  { name: 'sav_big_endian', code: { code: 'sav_big_endian' } },
];

/** A refusal as a command rejects with it: its code in JSON. */
export const rejection = (code: Record<string, unknown>) => new Error(JSON.stringify(code));
