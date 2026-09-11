# Standard-unit flight evidence

See [the validation report](../../standard-units-validation.md) for code changes,
offline comparisons and the live results. `report.md` is the original harness
report; its generic evidence list describes the complete original run.

This retained subset includes each native trace as `trace.csv.gz`, results,
assessments, setup and terminal readbacks, exact configurations, plots, source
and build hashes, and restoration verification. Full supervision polls, raw
simulator logs, events, process manifests and installation backups remain in
the original run directory named in `validation.json`. No preference contents,
aircraft files or installed binaries are copied into this evidence package.

`evidence-sha256.json` indexes these files. `validation.json` records a fresh
assessment of every decompressed trace, terminal authority-release checks and
matching source/build hashes. The historical 14-flight migration evidence is
retained separately.
