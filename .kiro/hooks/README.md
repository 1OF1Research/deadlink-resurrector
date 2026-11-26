# Kiro hooks

Native Kiro hook files in the current v1 format (Kiro IDE 1.0 / CLI V3:
standalone JSON files at `.kiro/hooks/*.json`, `version: "v1"`, PascalCase
triggers — format read from Kiro's own hooks documentation on
2026-11-26). They replace the planned-hooks note this file carried
through v0.5.

- `test-on-save.json` — `PostFileSave`, matcher `.*\.rs$`, command
  action: run `cargo test`. Command actions run in the project root
  and use no agent credits.
- `spec-check-on-task.json` — `PostTaskExec`, agent action: re-check
  the spec's acceptance list against the tests and report gaps only.

Both hooks are the workflow actually used to build this project:
every version landed only after its test run passed, and each spec
requirement names the test that covers it.
