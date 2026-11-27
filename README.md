# DeadLink Resurrector

Bring dead links back to life. Point it at a list of URLs; it probes each
one, classifies what it actually observed, and writes a Markdown
resurrection report — for every dead or moved link, the exact status and
the concrete next step.

## Category
Kiroween 2026 — **Resurrection**: dead technology (the ordinary hyperlinks
the web is built on, rotting silently) brought back to working life.

## Use
```
cargo build --release
./target/release/deadlink-resurrector [--follow] [--from-text] [--json|--html] urls.txt report.md
```
`urls.txt`: one URL per line, `#` lines are comments.
With `--from-text`, the input is any document instead: every http(s)
URL in it is extracted and checked.

## v0.6
Optional `--from-text` reads a document (Markdown, notes, a copied
page), extracts its URLs — trailing sentence punctuation stripped,
duplicates removed, first-seen order kept — and checks those, so the
tool points straight at the document whose links may have rotted.
Native Kiro hooks now ship in `.kiro/hooks/` as v1 JSON files
(test-on-save, spec-check after a task). Earlier: `--html` below.

## v0.5
Optional `--html` writes the same report as one self-contained HTML
page (no external assets) — open it in a browser or serve it as a
static demo page. `--json` and `--html` are not combined; pick one.
Earlier: optional `--json` writes the same report as JSON (totals plus one
object per URL, `status`/`note` as null when absent) for machine use;
Markdown stays the default. Earlier: optional `--follow` follows
exactly one redirect hop and reports the final verdict with the hop
kept in the note. Dead-client rows carry a Wayback Machine lookup
hint. Earlier still: https probed for real through
the system `openssl s_client` (honest `needs-tls-probe` without it),
redirect targets in original case, bounded concurrency, input order.

## Verdicts
alive · moved · dead-client (4xx) · dead-server (5xx) · unreachable ·
needs-tls-probe (https, only when no TLS route exists on the machine) · invalid

## How Kiro was used
Spec-driven development: `.kiro/specs/deadlink-resurrector/` holds the
requirements, design, and task list Kiro implemented against;
`.kiro/steering/product.md`
holds the product rules (never call a link dead on evidence not gathered);
`.kiro/hooks/` holds native v1 hook files for the test-on-save and
spec-check workflow. Full write-up in the
Devpost submission at submission time.

## License
MIT — see LICENSE.
