# Requirements — DeadLink Resurrector (Kiro spec)

1. Read URLs from a text file, one per line, `#` lines are comments.
2. Probe each http:// URL with a real GET; record status and Location.
3. Classify: 2xx alive, 3xx moved, 4xx dead-client, 5xx dead-server,
   no response unreachable, non-URL invalid.
4. (v0.2) Probe each https:// URL through the system `openssl s_client`
   when available, with the same classification. When no TLS route is
   available on the machine, the verdict stays `needs-tls-probe` —
   an https link is never called dead on evidence not gathered.
5. (v0.2) Redirect targets are captured with their original case.
6. (v0.2) Probes run concurrently in bounded chunks (8 threads); the
   report order matches the input order.
7. Write a Markdown report: URL, verdict, status, redirect/note, next step,
   plus a total line (checked / needing resurrection).
8. Exit 2 with a clear message on a missing/unreadable input file.
9. Unit tests cover classification boundaries, URL parsing (http + https),
   and response-head parsing (status + Location case preservation).
10. (v0.3) `--follow` follows exactly one redirect hop: the report shows
    the final verdict/status and keeps the hop in the note column.
    Without `--follow`, moved rows keep the v0.2 shape.
11. (v0.3) Dead-client rows carry a Wayback Machine lookup hint
    (`https://web.archive.org/web/*/<url>`), built deterministically
    with no extra network call.
12. (v0.4) `--json` writes the same rows as JSON: `checked` and
    `needing_resurrection` totals plus one object per URL in input
    order (`url`, `verdict`, `status` or null, `note` or null,
    `next_step`). Markdown remains the default, byte-identical.
    Strings are escaped per JSON rules; no external crates.

## Acceptance
- `cargo test` passes.
- A run over a fixture file containing a live http URL, a 404 URL, an
  https URL, and a garbage line produces a report whose verdicts match
  the observed statuses.
13. (v0.5) `--html` writes the same rows as one self-contained HTML
    page: totals line plus a table in input order, all values escaped,
    no external assets. `--json` and `--html` together is a usage
    error (exit 2). Markdown remains the default, byte-identical.
14. (v0.6) `--from-text` treats the input file as a document and
    extracts every http(s) URL from it: a URL runs until whitespace
    or < > " ', trailing sentence punctuation ( . , ; : ! ? ) ] } )
    is stripped, duplicates are removed, first-seen order is kept.
    Without the flag, line mode is unchanged and byte-identical.
15. (v0.6) Native Kiro hooks ship as v1 JSON files in `.kiro/hooks/`
    (test-on-save as a command action, spec-check as an agent
    action), in the format Kiro's current documentation defines.
