# Fork CI and binary releases

Crow runs `.crow/ci.yaml` on native linux/amd64 using a stock Rust/bookworm
image. `script/ci` installs the repo's mise tools, runs the existing `ci` and
`render` tasks, and rejects generated diffs. Tasks run serially to avoid test
supervisors interfering with one another. Cargo downloads, Rust toolchains,
target data, pnpm, mise, and Playwright caches persist under `/cache/pitchfork`.
No custom image, privileged step, host socket, or global Crow change is needed.

## Build a release

Use an immutable tag matching `Cargo.toml` plus a fork revision. For example:

```sh
mise run ci-dev
git push origin chore/fork-binary-releases
git tag v2.15.0-rsanheim.1 <full-commit-sha>
git push origin v2.15.0-rsanheim.1
my-crow pipeline create rsanheim/pitchfork --tag v2.15.0-rsanheim.1
```

A manual **tag** pipeline runs all normal gates and then builds the two Linux
archives. A manual **branch** pipeline only runs the gates. Tag pushes do not
automatically build or publish releases. Crow's checkout, the tag, and artifact
source commits must agree. Release helpers reject modified tracked files.

The Linux amd64 build uses native Cargo. Linux arm64 uses pinned cargo-zigbuild
and Zig with glibc 2.36 as the minimum (Debian bookworm). Neither runs a compiler
under QEMU. Both preserve upstream's `serious` profile (release optimization,
LTO, stripped symbols, one codegen unit) and use `Cargo.lock` and the UI pnpm
lockfile. Rust, Node, pnpm, Zig, and cargo-zigbuild are pinned in `mise.toml`.
The toolchain and source are repeatable; byte-for-byte reproducibility across
OS image updates is not promised.

Artifacts are retained in `/cache/pitchfork/releases/<tag>/`:

- `pitchfork-x86_64-unknown-linux-gnu.tar.gz`
- `pitchfork-aarch64-unknown-linux-gnu.tar.gz`
- `pitchfork-aarch64-apple-darwin.tar.gz` (built separately on Mac)

These reuse [upstream's names and profile](https://github.com/jdx/pitchfork/blob/main/.github/workflows/release.yml).
Each archive contains only `pitchfork`, with a sibling `.tar.gz.sha256` file.
The `.commit` files are local upload guards, not release assets.

## Upload to GitHub

Configure a Crow **repository** secret named `pitchfork_release_token`, limited
to the `manual` event and the upload step's `docker.io/library/debian:bookworm-slim`
image. Use a fine-grained GitHub token scoped to `rsanheim/pitchfork` with
Contents: write. No upstream release-plz or signing secrets are used.

```sh
my-crow pipeline create rsanheim/pitchfork --tag v2.15.0-rsanheim.1 \
  --var PITCHFORK_PUBLISH=true
```

This creates a draft prerelease and uploads Linux archives and checksums.
Uploads never use `--clobber`. To test before provisioning the Crow secret,
copy the artifact directory from minibox's cache volume to a local directory
and use your existing authenticated `gh` session from the exact tagged checkout:

```sh
PITCHFORK_RELEASE_DIR=/path/to/releases script/release upload v2.15.0-rsanheim.1 linux
```

On a native Apple Silicon Mac, check out the **same tag**, then:

```sh
script/release build v2.15.0-rsanheim.1 macos
script/release upload v2.15.0-rsanheim.1 macos
```

After checking the artifacts, publish explicitly:

```sh
gh release edit v2.15.0-rsanheim.1 --repo rsanheim/pitchfork \
  --draft=false --prerelease --latest=false
```

The two Linux binaries can ship before the Mac upload. Do not replace published
assets or move tags; source fixes get a new tag. The binary reports the Cargo
version (`pitchfork 2.15.0`); the tag's fork revision identifies the distribution.

## Consumer

Replace MeowPlaying's Cargo Git entry with:

```toml
"github:rsanheim/pitchfork" = "2.15.0-rsanheim.1"
```

Run `mise install` and `mise exec -- pitchfork --version`. The current
[GitHub backend](https://mise.jdx.dev/dev-tools/backends/github.html#asset-autodetection)
selects these Rust target names automatically. No `exe`, platform override,
Rust compiler, or ubi backend is needed. Refresh any tracked consumer mise lockfile.

Upstream GitHub Actions workflows are unchanged. Once Crow is proven and its
status checks are selected where needed, disable Actions in this fork's repo
settings to avoid duplicate upstream CI and release automation.
