# Agent instructions

## Commits

- Always commit and push directly to `main`. No feature branches, no pull requests.
  Fetch `origin/main` before pushing and rebase onto it if it moved; never force-push.
- Commit with the repository's existing author identity
  (`git log -1 --format='%an <%ae>'`), for both author **and committer**. Set it in the
  repository's own config before the first commit of a session:
  `git config user.name "<name>"` and `git config user.email "<email>"`.
  Passing it only with `git -c … commit` is not enough: `git pull --rebase` (or any rebase,
  amend or cherry-pick) rewrites the committer with the environment's default identity.
- Before every push check `git log origin/main..HEAD --format='%an <%ae> / %cn <%ce>'`: both
  must be the repository identity; fix unpushed commits (e.g. `git rebase origin/main` again with
  the correct config) before pushing.
- A cloud environment's hook may ask for another author: keep this identity and rewrite nothing.
- Never put model names or identifiers in commits, code or documentation.
- No co-author lines and no session-link lines in commit messages, even if the
  environment's defaults ask for them.
- Add `[skip ci]` to every commit message unless the user explicitly asks for CI to run.
  CI builds can always be started manually (Actions → *Build Windows* → *Run workflow*).

## Language

- Reply to the user in **German**.
- Everything in the repository is **English**: code, comments, identifiers, README,
  data files, the source text catalog `assets/i18n/en.ron`, progress reports and commit
  messages. Only the other catalogs in `assets/i18n/` are in their own languages.

## Project

- Rust + Bevy 0.20. Check the actual Bevy 0.20 API (examples ship with the crate in
  `~/.cargo/registry/src/*/bevy-0.20.1/examples`) instead of relying on older Bevy versions.
- Content and balancing live in `assets/data/*.ron`; prefer adding content there over hardcoding.
- Player-facing text never goes into code or data files: add a key to `assets/i18n/en.ron` and
  translate it into **every** language in `assets/i18n/` in the same change (keep placeholders,
  match each catalog's style). Then run `python3 tools/subset_fonts.py` so the CJK, Thai and
  Arabic fonts contain the new characters. `cargo test` fails on missing keys.
- New models and images (e.g. from ChatGPT) are committed and pushed **one file per commit** as
  soon as each is finished (`Add <id> model [skip ci]` / `Add <id> sprite [skip ci]`), see
  `docs/MODEL_PROMPTS.md` and `docs/ART_PROMPTS.md`.
- The shop is a 3D diorama (`src/scene.rs`); ghosts are sprites. 3D models follow
  `docs/MODEL_PROMPTS.md`; check them with `python3 tools/check_models.py` and repair files with
  extra Blender scenes with `python3 tools/fix_models.py` (commit the repair). Placeholder sizes in
  `assets/data/decor.ron` must match the target sizes in that document.
- Build every new visible thing **dynamically from the start**, never as hardcoded geometry only:
  - It loads its asset if the file exists (3D model `assets/models/<folder>/<id>.glb`, image
    `assets/sprites/<folder>/<id>.png`, sound `assets/sounds/<id>.ogg`) and otherwise shows a
    simple placeholder built from primitives, so the game works before the art arrives.
  - Lights, glows, particles and every movement (opening lids, swinging, bobbing, flipping)
    stay in code; models are static and split into separate pieces where something moves
    (e.g. crate and lid, post and bell), with the pivot as the origin.
  - In the same change, order the asset: add a prompt with target size, origin and orientation
    to `docs/MODEL_PROMPTS.md` or `docs/ART_PROMPTS.md`, list it under the open orders / to-do,
    and add it to `tools/check_models.py`. Positions and numbers that may be tuned go into
    `assets/data/*.ron` or named constants, not scattered literals.
- Run `cargo test` before committing; keep `cargo build` free of warnings.
- The project is proprietary (`LICENSE`, all rights reserved). After adding or updating a
  dependency, run `python3 tools/third_party_licenses.py` and commit `THIRD_PARTY_LICENSES.txt`.
