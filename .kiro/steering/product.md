# Product steering — DeadLink Resurrector

Problem: links rot. A page, listing, or document that points at a dead URL
quietly stops working for every reader, and nobody notices until a reader
complains. Maintainers need a plain report: which links are dead, which
moved, what was actually observed, and what to do next for each one.

Product rules:
- Every verdict carries the observed status (or the honest absence of one).
- A link is never called dead on evidence the probe did not gather:
  https links are probed over TLS where openssl exists, and stay
  `needs-tls-probe` on machines where no TLS route is available.
- The report is the product: a Markdown table a maintainer can act on
  without re-running anything.
- No accounts, no tracking, no hosted service required: a local CLI.
