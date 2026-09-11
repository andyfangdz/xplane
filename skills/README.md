# X-Plane skills

This directory is the maintained source for the X-Plane Codex skills:

- [test-xplane-aircraft](test-xplane-aircraft/SKILL.md): repeatable aircraft
  performance measurements, reversible test isolation, and audio/video evidence.
- [develop-xplane-aircraft-mod](develop-xplane-aircraft-mod/SKILL.md): reversible
  aircraft-local models, POH surface calibration, and native avionics integration.

Their instructions, references, UI metadata, and helper scripts were imported
unchanged from [xplane-flight-testing at 30cd116](https://github.com/andyfangdz/xplane-flight-testing/tree/30cd116a4844790913195b845f2a8c9ac0479bab/skills).
That repository retains the earlier history and forwards here.

## Local setup

Link each skill directory into the skill location used by your Codex installation.
[Codex supports linked skill folders](https://learn.chatgpt.com/docs/build-skills#where-codex-loads-local-skills).
With links, edits and Git updates in this checkout are immediately reflected in
the installed skill files. Keep the checkout at the linked path.

For the Windows setup using `~/.codex/skills`, run from the repository root:

```powershell
$skillSourceRoot = (Resolve-Path .\skills).Path
$skillInstallRoot = Join-Path $env:USERPROFILE '.codex\skills'
New-Item -ItemType Directory -Path $skillInstallRoot -Force | Out-Null
foreach ($skillName in 'test-xplane-aircraft', 'develop-xplane-aircraft-mod') {
    New-Item -ItemType Junction -Path (Join-Path $skillInstallRoot $skillName) `
        -Target (Join-Path $skillSourceRoot $skillName)
}
```

Set `skillInstallRoot` to your configured skill directory if it differs. Existing
skill folders must first be compared with this checkout and backed up outside
the discovery directory; the commands above do not overwrite them. Junctions
keep the existing installed paths usable by scripts and saved task instructions.

Invoke `$test-xplane-aircraft` or `$develop-xplane-aircraft-mod` in a subsequent
Codex turn. Their existing automatic invocation behavior is preserved.

## Maintenance

Edit the files here. Keep relative references within each skill so a skill can
also be installed independently. Validate `SKILL.md` frontmatter and linked
resources after changes, parse PowerShell helpers, and compile Python helpers
without running a simulator trial for a packaging-only change.
