# Fork binary releases — on hold

Status: **ON HOLD** as of 2026-09-25, at Rob's request.

Work lives on `chore/fork-binary-releases`. Nothing has been merged into `main`,
no PR has been opened, and no GitHub release has been published. Do not resume
builds, publication, or consumer migration until requested.

## Intended approach

- Crow/minibox runs normal CI and builds Linux amd64 natively.
- Linux arm64 is cross-compiled with cargo-zigbuild and Zig, without QEMU.
- A separate native Apple Silicon command builds macOS arm64.
- GitHub Releases distributes the archives; consumers use mise's `github:` backend.
- Zot is not involved in binary distribution.

The implementation preserves upstream's `serious` profile and Rust-target archive
names. Application code and `namespace_per_worktree` behavior are unchanged.

## What exists

- `.crow/ci.yaml`, `script/ci`, and `script/release` are committed and pushed.
- The repo is registered in Crow with `.crow/` as its configuration path.
- Build caches persist under `/cache/pitchfork/` on minibox.
- Release tool versions and dependency locks are committed. Existing mise gates
  use pnpm for JavaScript installation and scripts.
- Tags `v2.15.0-rsanheim.1`, `.2`, and `.3` were pushed. The first two were
  validation attempts; tags were not moved.
- The final build tag, `v2.15.0-rsanheim.3`, points to
  `5cc8d59980933cbb16a7e57a6126191809d12207`.

### Artifacts

The **unpublished GitHub draft** for `v2.15.0-rsanheim.3` contains only:

- `pitchfork-aarch64-apple-darwin.tar.gz` (8,156,090 bytes)
- Its `.tar.gz.sha256` checksum

Crow pipeline [#4](https://minibox.shark-tet.ts.net/repos/rsanheim/pitchfork/pipeline/4)
completed successfully. These files remain in
`/cache/pitchfork/releases/v2.15.0-rsanheim.3/` on minibox:

- `pitchfork-x86_64-unknown-linux-gnu.tar.gz` (9,170,716 bytes)
- `pitchfork-aarch64-unknown-linux-gnu.tar.gz` (8,459,518 bytes)
- A checksum and `.commit` source marker for each archive

The Linux assets have **not** been uploaded to GitHub. Native Mac archives also
remain locally under `target/releases/` for all three tags. The superseded `.2`
draft release was deleted; its tag and local artifact were retained.

## Validation so far

- Normal Crow CI passed: 545 Rust tests (8 skipped), all 32 Bats tests, the browser
  smoke test, lint, docs, and render/no-diff checks. Pipeline #2 took 4m 38s.
- Pipeline #4 passed the gates and built both Linux release artifacts in 12m 23s
  overall. The arm64 binary passed the AArch64 file-format check.
- Native amd64 and Mac binaries reported `pitchfork 2.15.0` and exposed the
  `namespace_per_worktree` schema entry.
- The packaged Mac binary passed a two-checkout worktree-isolation smoke test,
  including `stop -l` preserving the other checkout's daemon.
- Local `mise run ci-dev` was attempted; Rust tests passed, but Bats failed because
  Lume already occupies its fixed test port 7777. Lume was left running.
- The required pre-commit rerun for this status document failed in the parallel
  browser smoke test with `ERR_CONNECTION_REFUSED`, cancelling the remaining
  tests. The isolated, serial Crow gates above passed.

## Remaining if resumed

1. Run the Linux arm64 artifact on a native arm64 environment and verify version
   and worktree behavior. An existing arm64 OrbStack Ubuntu VM was identified.
2. Verify worktree behavior with the Linux amd64 release artifact.
3. Upload the Linux archives and checksums to the matching GitHub draft.
4. Publish explicitly, then prove mise automatically selects and installs the
   correct archive on all three platforms.
5. Only after that, update MeowPlaying's Cargo Git dependency. The intended entry
   is `"github:rsanheim/pitchfork" = "2.15.0-rsanheim.3"`; it is not yet validated
   or usable as a public release installation.

Crow's optional upload step expects a repo-scoped `pitchfork_release_token`
secret (GitHub Contents: write, restricted to manual runs and the upload image).
No such secret was configured. The Mac draft upload used local authenticated
`gh`; no local credential was copied into Crow.

MeowPlaying, Zot, global Crow/agent infrastructure, and upstream GitHub Actions
workflows are unchanged. No builds remain running. GitHub Actions settings were
not changed; reconsider disabling duplicate fork Actions only if this work resumes.

See [fork-releases.md](docs/fork-releases.md) for the build/upload commands.
