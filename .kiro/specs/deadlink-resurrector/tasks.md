# Tasks — DeadLink Resurrector (Kiro spec)

The build ran as six tasks, in spec order. Each task started as new
numbered requirements in requirements.md, was implemented against
them, and landed only after its check passed; the spec-check hook
re-read the acceptance list after each task.

1. [x] v0.1 — scaffold: line reader (one URL per line, `#` comments),
   http probe, classification as a pure function, Markdown report.
   Check: `cargo test` 4/4; live fixture matched spec (200 alive,
   404 dead-client, https `needs-tls-probe`, garbage invalid).
2. [x] v0.2 — https probed for real through the system `openssl
   s_client`; redirect target captured in original case; bounded
   concurrency (chunks of 8, input order kept).
   Check: `cargo test` 6/6; https 200 and https 404 are real
   verdicts; the 6-URL fixture ran in 0.58 s.
3. [x] v0.3 — `--follow` follows exactly one redirect hop and keeps
   the hop in the note; dead-client rows carry a Wayback Machine
   lookup hint built as a pure string.
   Check: `cargo test` 9/9; fixture matched spec in both modes
   (github.com 301 → alive 200 under `--follow`).
4. [x] v0.4 — `--json` as a second renderer over the same row data.
   Check: `cargo test` 11/11; Markdown byte-identical to the v0.3
   fixture; JSON parsed cleanly with the same verdicts.
5. [x] v0.5 — `--html` as a third renderer: one self-contained page,
   no external assets; combined `--json --html` is a usage error.
   A demo page was generated from the fixture report.
   Check: `cargo test` 13/13; Markdown and JSON byte-identical to
   the v0.4 fixture; HTML carried all 6 rows with totals and hints.
6. [x] v0.6 — `--from-text` document extraction (dedupe, first-seen
   order, trailing sentence punctuation stripped) feeding the same
   probe; native Kiro v1 hook files in `.kiro/hooks/`, written from
   Kiro's current hooks documentation.
   Check: `cargo test` 14/14; line mode byte-identical to the v0.5
   fixture; document fixture 5 checked / 2 needing resurrection,
   verdicts matching line mode.
