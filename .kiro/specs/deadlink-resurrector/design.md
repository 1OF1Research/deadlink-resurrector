# Design

- Single Rust binary, standard library only (no crates): TCP probe for
  http, classification as a pure function, report as string rendering.
  Zero dependencies keeps the build reproducible anywhere cargo runs.
- TLS (v0.2): std has no TLS, so https probes go through the system
  `openssl s_client` subprocess (bounded by `timeout 15`). If openssl is
  absent (exit 127 / spawn failure), the verdict stays `needs-tls-probe`
  so the report never overclaims. The report format did not change when
  the TLS probe landed — the verdict was reserved in v0.1 for exactly this.
- Concurrency (v0.2): scoped std threads in chunks of 8; results are
  zipped back in input order, so concurrency never reorders the report.
- Spec-driven: requirements.md is the source; Kiro implements against it;
  steering/product.md holds the product rules above.

- Follow (v0.3): opt-in `--follow`, exactly one hop, resolved against
  the original URL (absolute, root-relative, path-relative). One hop
  only — chains are reported, not chased silently.
- Wayback hint (v0.3): pure string construction for dead-client rows;
  no archive network call is made, so the hint never fails a run.
- JSON mode (v0.4): `--json` is a second renderer over the same row
  data — one source of truth, two outputs. Hand-rolled escaping keeps
  the zero-crate rule; `status`/`note` render as JSON null when absent,
  never as a placeholder string a machine reader would have to guess at.
- HTML mode (v0.5): `--html` is a third renderer over the same row
  data. One file, inline style only, no external assets — so the page
  works opened from disk and as a static hosted demo page. Escaping
  covers &, <, >, double and single quotes.
