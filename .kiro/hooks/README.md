# Kiro hooks (planned, recorded in v0.1)

- On save of src/*.rs: run `cargo test` (Kiro agent hook, shell command).
- On spec edit: re-check requirements acceptance list against tests.

Hook definitions are added as native Kiro hook files when the project is
opened in Kiro; this note records the intended hooks so the /.kiro
directory reflects the real workflow from the first commit.
