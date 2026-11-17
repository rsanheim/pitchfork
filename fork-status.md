# Fork status

_Checked:_ 2026-11-17
_Upstream:_ jdx/pitchfork `main` at `a40d2d8` (v2.35.0)
_Fork:_ rsanheim/pitchfork

This file records how far this fork has drifted from upstream and how the fork's
worktree isolation compares to what upstream has since shipped.

_Evidence:_ the upstream behavior described here comes from reading upstream source,
docs, and discussions at `a40d2d8`. None of it was run. The fork behavior is as
documented in `docs/concepts/namespaces.md` and covered by
`tests/test_e2e_worktree_isolation.rs` on this branch. The merge conflicts were
measured with `git merge-tree`.

## Drift

| Branch | Version | Ahead | Behind | Merge base with upstream |
|---|---|---|---|---|
| `main` | 2.13.1 | 0 | 595 | `97e011c` (2026-06-09) |
| `worktree-isolation` | 2.15.0 | 6 | 521 | `ccc1e01` (2026-07-03) |
| `chore/fork-binary-releases` | 2.15.0 | 11 | 521 | `ccc1e01` (2026-07-03) |

* The fork revision in day-to-day use is `4578733` (v2.13.1), which predates the July merge of upstream into `worktree-isolation`.
* Upstream has released 20 minor versions since `ccc1e01`: 412 files changed, about 105,000 lines added and 25,000 removed.
* The feature was never proposed upstream. No discussion or pull request was opened.

A trial merge of upstream `main` into `worktree-isolation` conflicts in 12 files:

* Hand-written: `src/pitchfork_toml.rs`, `src/ipc/batch.rs`, `src/cli/list.rs`, `src/cli/mod.rs`, `src/template.rs`, `tests/test_pitchfork_toml.rs`, `docs/reference/configuration.md`
* Generated (resolved by `mise run render`): `docs/cli/commands.json`, `docs/cli/index.md`, `docs/cli/list.md`, `docs/public/schema.json`, `pitchfork.usage.kdl`

Upstream changes since `ccc1e01` in the files the feature touches most:

| File | Lines added | Lines removed |
|---|---|---|
| `src/pitchfork_toml.rs` | 2,355 | 311 |
| `src/ipc/batch.rs` | 1,432 | 174 |
| `src/template.rs` | 846 | 60 |

## What the fork adds

Commit `52da23e` adds `namespace_per_worktree`, an opt-in config key:

```toml
namespace = "myproj"
namespace_per_worktree = true
```

* The namespace becomes `<namespace>-<hash>`, where the hash is a stable 8-character FNV-1a digest of the checkout's canonical path.
* An explicit `namespace` is required as the base.
* `start -l`, `stop -l`, and `restart -l` narrow to the current checkout's namespace.
* `start` registers the checkout's namespace in the global `[namespaces]` registry so supervisor-side features can find its config.
* `{{ worktree_hash }}` is available in templates.
* `pitchfork namespace` prints the current checkout's namespace, and `pitchfork list --dirs` shows each daemon's directory.

## What upstream shipped

Upstream uses the directory name as the namespace and treats each worktree as a
separate directory. There is no path hash and no opt-in key.

* https://github.com/jdx/pitchfork/pull/737 (2026-08-16): the supervisor auto-discovers git worktrees and jj workspaces, so cron, boot start, and file watching see their daemons. Controlled by the `general.worktree` setting, on by default.
* https://github.com/jdx/pitchfork/pull/746 (2026-08-18): hooks resolve from the daemon's directory, `pitchfork run` one-offs use the invocation directory's namespace, `ready_cmd` receives the resolved `$PORT`, `pitchfork clean` takes namespace filters and `--prune`, and a `namespace` set in any of a directory's four config files applies to all of them. Addresses https://github.com/jdx/pitchfork/discussions/741.
* https://github.com/jdx/pitchfork/pull/755 (2026-08-21): gates discovery behind `general.worktree`.
* https://github.com/jdx/pitchfork/pull/880 (2026-09-18): with the proxy enabled, daemons get hostnames of the form `<daemon>.<project>.<tld>` in the primary checkout and `<daemon>.<worktree>.<project>.<tld>` in a linked worktree. `worktree_label` overrides the worktree part.

Upstream's documented position, from `docs/concepts/namespaces.md`:

> Worktrees with the same final directory name need explicit namespace overrides to avoid collisions.

> If a config sets an explicit top-level `namespace`, it overrides the directory-derived namespace, so give each worktree a distinct explicit namespace to keep same-named daemons from colliding.

## Comparison

| | Fork | Upstream |
|---|---|---|
| Checkout identity | Hash of canonical path, appended to an explicit base namespace | Final directory name, or an explicit `namespace` |
| Opt-in | `namespace_per_worktree = true` | None; on by default |
| Explicit `namespace` in committed config | Required; checkouts stay isolated | All checkouts share that namespace |
| Per-checkout setup | None | None when directory names are unique; otherwise a distinct `namespace` per checkout |
| Namespace readability | `myproj-e4c3d83b` | `feature-x` |
| Project name in the namespace | Yes | No, unless set explicitly |
| Moving or renaming a checkout | Changes the namespace | Changes the namespace only if the final directory name changes |
| `-l` scope | Current checkout's namespace | Every non-global daemon in the config chain from the filesystem root to the current directory |
| Worktree vs primary checkout distinction | None; every checkout, primary included, gets a hash | Proxy hostnames only; daemon identity does not record it |
| Template variable for the checkout | `{{ worktree_hash }}` | None; `{{ namespace }}` |
| CLI helpers | `pitchfork namespace`, `list --dirs` | `list --namespace`, `tui --project`, `clean <namespace> --prune` |
| Supervisor finds worktree configs | Namespace registered on `start` | Auto-discovery of git worktrees and jj workspaces |
| Cleanup of deleted checkouts | Not garbage-collected | `clean --prune` |
| Proxy hostnames per worktree | No | Yes |

## Behavior by worktree layout

`myproj` is the primary checkout in each case.

| Layout | Fork | Upstream |
|---|---|---|
| Sibling with a unique name: `~/wt/feature-x` | Isolated as `myproj-<hash>` | Isolated as `feature-x`, if the committed config has no explicit `namespace` |
| Same final directory name: `~/wt/feature-x/myproj` | Isolated as `myproj-<hash>` | Shares namespace `myproj` with the primary checkout |
| Nested in the primary checkout: `myproj/.claude/worktrees/agent-1234` | Isolated; `-l` stays in the worktree | Gets namespace `agent-1234`, but the primary checkout's config is also loaded, so `-l` covers both checkouts' daemons |
| Nested, with an explicit `namespace` in the committed config | Isolated | Expected to fail with a namespace collision error, since two directories in one config chain resolve to the same namespace |

Under upstream, the same-name and nested layouts need a distinct explicit namespace
per checkout. The two mechanisms upstream provides are a `namespace` line in an
untracked `pitchfork.local.toml`, or a registration made with
`pitchfork config add --dir <checkout> --namespace <name>`. Either one is a step
that has to run for each new worktree.
