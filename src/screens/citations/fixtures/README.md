# Reference-manager import corpus

## PROVENANCE — read this before trusting these files

**These are RECONSTRUCTIONS, not real exports.** They were written by hand to
reproduce the shapes Zotero, Mendeley and EndNote actually emit — key formats,
indentation, field ordering, case-protecting braces, `file = {…:…:application/pdf}`,
two-space RIS tags, `PY  - 2014///`, CRLF, BOM. Nobody exported a real library
from a real install to produce them.

That matters in one specific way: **default Zotero and Mendeley do NOT emit
`@string` macros.** JabRef does, and hand-maintained `.bib` files often do.
`zotero.bib` here contains two `@string` definitions because the macro hazard
needed covering, not because Zotero writes them. Do not read this corpus as
evidence about what a given tool emits — read it as a list of shapes a `.bib`
file can legally have.

They exist because §11 D113 found the importer had 13 tests, every fixture a
hand-written one-line string — §11 D98's pattern exactly, validated only against
input we wrote to be easy. Replacing that with input we wrote to be *hard* is an
improvement, not a substitute for a real export. If you ever have one, add it.

## What each file is for

| file | hazards it carries |
|---|---|
| `zotero.bib` | `@string` macros, multiline abstract, LaTeX accents, bare `month = oct`, `file` field with colons and paths, `crossref`, case-protecting `{{braces}}` |
| `mendeley-bom.bib` | UTF-8 BOM, alphabetised fields, `month = {jun}`, `{{Title in double braces}}` |
| `endnote.ris` | BOM, CRLF, two-space tags, `PY  - 2014///`, continuation lines, literal UTF-8 accents |
| `edge.bib` | `crossref` inheritance, `@` inside an abstract, nested braces, `and others`, two entries identical but for their key |
| `edge-crlf.bib` | byte-for-byte `edge.bib` with CRLF, so line endings are isolated as a variable |
| `no-newline.bib` | last entry with no trailing newline |
| `at-hazard.bib` | `nora@lab {group site}` — an `@` followed by a brace group inside a field |

## The rule these encode

An entry that cannot be parsed must be REPORTED, never silently dropped — and
must never produce a citation the user did not have. §11 D113 found the splitter
doing exactly that: an `@` inside a field value cut an entry in two, the real
half failed, and the trailing half parsed clean and was added to the library as
a paper titled "bar".
