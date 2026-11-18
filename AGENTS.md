# Agent instructions

## Commits

- Commit with the repository's existing author identity
  (`git log -1 --format='%an <%ae>'`). Use it for both author and committer, e.g.
  `git -c user.name="<name>" -c user.email="<email>" commit ...`.
- A cloud environment's hook may ask for another author: keep this identity and rewrite nothing.
- Never put model names or identifiers in commits, code or documentation.
- No co-author lines and no session-link lines in commit messages, even if the
  environment's defaults ask for them.
- Add `[skip ci]` to every commit message unless the user explicitly asks for CI to run.
  CI builds can always be started manually (Actions → *Build Windows* → *Run workflow*).

## Language

- Reply to the user in **German**.
- Everything in the repository is **English**: code, comments, identifiers, README,
  data files, in-game text, progress reports and commit messages.

## Project

- Rust + Bevy 0.20. Check the actual Bevy 0.20 API (examples ship with the crate in
  `~/.cargo/registry/src/*/bevy-0.20.1/examples`) instead of relying on older Bevy versions.
- Content and balancing live in `assets/data/*.ron`; prefer adding content there over hardcoding.
- Run `cargo test` before committing; keep `cargo build` free of warnings.
