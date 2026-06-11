# Worktree Isolation Review — upstream or fork?

*Branch:* `worktree-isolation`
*Date:* 2026-06-10 (review), 2026-06-11 (design addendum, implementation)
*Scope:* Originally review-only; the "Proposed plan" pre-flight checklist is now the implementation work order for this branch. Battle-test the result in a real project before opening the upstream discussion.

## TL;DR — recommended direction

Pursue path 2 (upstream), via a GitHub Discussion first. The evidence:

* There is real, documented upstream demand for this problem class (discussion #199 "Daemon ID conflict"), and worktrees are an actively invested area (PR #448 shipped worktree-aware proxy routing in v2.12.0).
* The strongest pitch angle: the existing `proxy.worktree` feature routes `<branch>.<slug>.<tld>` URLs by resolving each worktree path to a namespace — but same-leaf worktrees all collapse to one namespace today, so the routing key is broken for exactly the case it exists for. `worktree_isolation` is the missing half that makes an already-shipped feature actually work.
* The code is well integrated (details below) — implemented at the single namespace-resolution chokepoint, so every existing consumer (proxy, web, TUI, CLI) inherits it for free. It is near submission quality after a few mechanical fixes.
* The fork-maintenance cost of path 1 is high: pitchfork ships ~9 releases per 2 months, and this change lives inside `src/pitchfork_toml.rs` namespace resolution, which upstream churns actively. Rebasing a fork against that file indefinitely is the worst-case fork shape.

The main risk: maintainers may prefer a branch-name suffix (matching the proxy's existing worktree vocabulary) over a path-hash suffix. That is exactly what the discussion-first step de-risks. If upstream declines, fall back to path 1 — the current single-chokepoint design is also the most rebase-friendly shape a fork could have, so nothing is wasted.

## Answers to your config-scoping questions

*"Does pitchfork not have a resolved config per project?"*

Correct — it does not, and this is the crux. There is no "project" object anywhere in the codebase. `all_merged()` is one flat bag of daemons from every config file in the root→cwd chain (plus globals), where each file's daemons are stamped with that file's own namespace at parse time (`parse_str`, src/pitchfork_toml.rs:1099-1140), before any merging. The merge step (src/pitchfork_toml.rs:1478-1496) doesn't even carry `namespace` or `worktree_isolation` forward — a merged config is explicitly multi-namespace. Daemons from a parent directory's `pitchfork.toml` land in the same merged bag under the parent's namespace; that's the intended monorepo behavior.

*"Why does `worktree_isolation_for_dir` need to find the 'nearest' config file?"*

Because "which project am I standing in?" can only be answered by finding the nearest config — and pitchfork already worked this way on `main`: `namespace_for_dir` (src/pitchfork_toml.rs:766-773) does the identical `rfind`-the-deepest-config dance and is used by `resolve_id`, the proxy, etc. Our helper is a parallel of an established pattern, not an invention.

Why it's needed at all: the cwd chain cannot distinguish "parent project of my monorepo" (want included in `-l`) from "parent checkout that happens to contain me" (want excluded). Both look identical — an ancestor directory with a config. The flag, read from the nearest config, declares "this directory is a hard checkout boundary." Note the hashes alone do not fix this: with isolation on, parent and nested checkouts get *different* hashed namespaces, so the old `!= "global"` filter still sweeps the parent's daemons into `stop -l`. The hash fixes identity; the narrowing fixes scope. Both are needed.

*"Our changes to `get_local_configured_daemons` feel off to me in particular"*

Your instinct is half right. The narrowing is necessary (above), but narrowing to exactly *one* namespace string is a genuine correctness hole:

* A project dir can legally hold sibling configs with different explicit namespaces — only the `pitchfork.toml`↔`pitchfork.local.toml` pair is equality-checked (`sibling_base_config`, src/pitchfork_toml.rs:269-274); nothing cross-checks the `.config/` pair against the root pair.
* Repro sketch: `.config/pitchfork.toml` with `namespace = "a"` defining `db`, plus `pitchfork.toml` with `namespace = "b"` and `worktree_isolation = true` defining `api`. The flag applies to all siblings, so they resolve to `a-<hash>` and `b-<hash>`. `namespace_for_dir` `rfind`s only one of them (`b-<hash>`), so `stop -l` / `start -l` silently miss `a-<hash>/db` — daemons that `main`'s `!= "global"` filter included.
* The fix: filter by the *set* of namespaces of all existing config files in the nearest project dir (`sibling_config_paths(nearest)` → `namespace_from_path` each → membership filter). Usually a one-element set, identical behavior to today, and correct by construction. While there, collapse the double traversal (`worktree_isolation_for_dir` + `namespace_for_dir` are two full upward filesystem walks; one helper returning `(nearest_path, namespaces, isolation)` would do).

Why sibling files are scanned from disk rather than using a merged view: namespaces are resolved per file during parse, *before* merging — the merged view doesn't exist yet at that point, and a merged-chain flag would make a file's namespace depend on the invoking cwd, breaking the invariant that `namespace_from_path(file)` is a pure function of the file. The supervisor relies on that invariant when it re-derives daemon IDs from registered namespace dirs in its own working directory (`all_merged_all_namespaces`, src/pitchfork_toml.rs:992-1018). The sibling scan is the only way for a flag in an untracked `pitchfork.local.toml` to re-namespace daemons defined in the committed `pitchfork.toml`. So the shape is forced by the architecture; it's correct.

## Upstream contribution landscape

* No CONTRIBUTING.md, no PR/issue templates. The de facto contributor guide is the repo's CLAUDE.md/AGENTS.md (`mise run ci-dev` before committing, conventional commits).
* Issues are disabled — all feature requests go through GitHub Discussions. jdx's stated policy (from mise's contributing guide): create a discussion before a non-obvious PR, because "PRs are often either rejected or need to change significantly after submission."
* External feature PRs do get merged, fast: #437 (outsider adding a config option, the closest analog) went from open to merged in 3 days with no design pushback. gaojunran is a de facto co-maintainer who implements most features, often picking up ideas from discussions — if you want this to land as *your* PR, say so explicitly in the discussion and have the PR ready.
* Existing demand: discussion #199 is exactly this problem class ("the fact that the pitchfork processes aren't automatically namespaced to directories... is very unintuitive" — johnpyp). It was resolved by the current namespace system (#213), which is precisely what still collides for same-leaf checkouts. The residual gap is real and unaddressed; nobody has asked for per-checkout isolation explicitly yet, so demand is inferred, not proven.
* jdx weighs complexity heavily (his only adjacent comment in #199: "I am ok not permitting this if it's too much work"). The opt-in, no-new-CLI-surface, no-settings-change design fits that bias — worth emphasizing.
* CI gates that will apply: conventional-commit PR title check, `mise run render` must produce no diff, fmt/clippy/tests, plus a Greptile AI review bot.
* Red flag: an aggressive PR auto-closer (failing checks → closed after 7 days; any PR → closed after 30 days). Don't open the PR until you can iterate promptly.

## Code quality assessment

*Verdict: native, not bolt-on — but not yet submission-ready due to two mechanical gaps.*

What's good:

* Implemented at the right chokepoint (`namespace_from_file` / `namespace_from_path_with_override`), so the proxy's worktree routing, the web API, the TUI, and all ID resolution inherit isolation with zero changes. The one consumer needing special handling (`get_local_configured_daemons`) got it.
* The new parse/read functions are near line-for-line mirrors of the existing namespace-override machinery; the error variant matches sibling `ConfigParseError` variants exactly (miette code/url/help).
* The inline FNV-1a is justified (no hashing code exists in src/ today; std's `DefaultHasher` is unstable across releases; daemon identity depends on hash stability) and the golden-hash test converts the "stable forever" comment into an enforced contract.
* Tests match house style precisely (banner separators, TempDir, doc-comment-per-test) and cover the right scenarios, including both e2e cases that matter (same-leaf checkouts; nested worktree `stop -l`).

What a maintainer would ask for:

* Blocking: `docs/public/schema.json` was not regenerated. The schemars field is correct, but `mise run render` was never run — upstream CI asserts render produces no diff, so the PR fails CI as-is. Editor autocomplete also flags the key as unknown until fixed.
* Blocking: the commit message ("Add opt-in worktree_isolation...") is not conventional-commit format; needs `feat(config): ...` at minimum for the PR title.
* Consolidate the duplicated parse-whole-TOML-for-one-key machinery (namespace override + worktree_isolation) into one top-level-overrides parser, parsed once per file. The sibling scan currently re-reads and re-parses up to 4 files per config parsed — roughly O(k²) parses per project dir per command. Files are tiny so it's invisible in wall-clock terms, but the codebase's own comments preach computing namespaces once.
* Fold the `is_global_config` guard into `effective_worktree_isolation` (the same triad is duplicated verbatim at three call sites: src/pitchfork_toml.rs:514-521, 1126-1130, 1337-1341), and reconsider the `self_value: Option<Option<bool>>` parameter (nested Options with subtle semantics).
* Add a unit test for the global-config rejection path — the one error message asserted nowhere.

## Feature interactions (overlap analysis)

Complement, not overlap. Supervisor-side features (autostop, cron, retry, file-watch, clean) are all keyed on per-daemon state (dir/cmd/pid), not namespace shape — hashed namespaces are inert there. The honest caveats to carry into any upstream pitch:

* *Ports:* the docs oversell "isolated ports." Namespaces don't allocate ports — bind-checking plus opt-in `port.bump` does. What isolation actually removes is the `DaemonAlreadyRunning` short-circuit; a second checkout then hits `PortConflict` unless the config uses `port = { expect = [...], bump = N }` or port 0. The docs claim should be softened.
* *Hash leakage:* the suffix is machine- and path-specific. It appears in every `pitchfork list` line, must be typed to address another checkout's daemon, and makes committed cross-namespace references (`depends = ["myproj/postgres"]`, qualified group members) impossible to write portably for an isolated project. There is no CLI command that prints "my current namespace" (the web API has one).
* *Ad-hoc daemons:* `pitchfork run <name>` for unconfigured names falls back to `global/<name>` — two checkouts running the same ad-hoc daemon still collide. Contradicts the docs' "isolated set of daemons" framing for that case.
* *Orphan accumulation:* nothing detects a deleted/moved checkout. Stopped entries are cleanable (`pitchfork clean`), but log dirs linger until retention prunes, and `proxy add` permanently writes `[namespaces.<hash>]` entries to the global config with no GC. Same gaps as `main`, but per-checkout namespaces multiply the identities a user accrues.
* *Existing-feature workaround check:* an explicit `namespace` in an untracked `pitchfork.local.toml` does NOT achieve this today — namespace inheritance flows only base→local, so a local-only namespace never re-namespaces daemons defined in the tracked file. The flag's sibling-file inheritance is a genuinely new capability, not sugar.

## Correctness findings

| Finding | Severity | Failure mode |
|---|---|---|
| `-l` narrows to a single namespace; sibling configs with divergent explicit namespaces silently drop out of `start -l`/`stop -l` | Medium | Silent |
| Untracked `pitchfork.local.toml` opt-in does not protect a parent from a nested worktree's `stop -l` (nested checkout lacks the flag → falls back to broad `!= "global"` filter), despite docs advertising the local.toml pattern | Medium | Silent |
| Moved/renamed checkout: daemons orphaned under the old hash; short-name `pitchfork stop api` resolves to the *new* namespace and reports "not running" while the orphan keeps running. Manual recovery works (qualified ID from `list`, then `clean`) but is undocumented | Medium | Silent |
| `docs/public/schema.json` not regenerated (`mise run render` not run) | Medium | Loud in CI |
| `canonicalize()` fallback is silent; if it fails once then succeeds (permissions, NFS), the same checkout gets two namespaces. Mitigated: CWD is already symlink-free via getcwd; recommend a `warn!` when fallback fires for an existing dir | Low-Med | Silent |
| Broken TOML in any sibling file now fails namespace resolution for valid files, even when the flag is unused (sibling scan runs unconditionally) | Low | Fail-fast, clear error |
| `start -l` can transitively start cross-namespace deps that `stop -l` will never stop | Low | Silent |
| `-l` semantics become mode-dependent in monorepos (nearest config with the flag excludes ancestor daemons `main` would include) | Low | By design, needs docs |
| `InvalidWorktreeIsolation` help text ("set worktree_isolation = true") is wrong advice for the conflict and global-config error cases | Low | Cosmetic |

Verified fine (explicitly checked, no issue): hash suffix can never fail namespace validation; FNV-1a implementation is correct (independently recomputed against the golden test); `namespace_for_dir` does return the hashed namespace so the `-l` filter matches; the `self_value` sibling-path matching has no normalization bug; all 9 new unit tests and both e2e tests pass; the 27 failing e2e tests in the wider suite fail identically on `main` (environmental, not ours).

## Design addendum (2026-06-11): namespace ambiguity and hash UX

Two design weaknesses identified after the initial review, with the resolution we landed on.

*Problem 1: derived namespaces diverge per checkout.* Namespace derivation uses the leaf dir name, so a project checked out at `~/src/search-lib` and a worktree at `~/worktrees/fix-bug` resolve to `search-lib-<h1>` and `fix-bug-<h2>` — not recognizably the same project. This only happens when `namespace` is left implicit; an explicit committed `namespace = "search-lib"` already gives every checkout the same base. Pitchfork has no strict project boundary on disk, so there is no reliable way to derive a meaningful cross-checkout base from the filesystem.

*Problem 2: hash suffixes are hostile to humans.* `pitchfork start app-ee0d194f/api` is a non-starter as the headline UX. Scope of the pain: short-ID resolution inside a checkout works (`pitchfork start api`), so hashes are mostly read in `pitchfork list` or typed for cross-checkout operations — but that is still enough to sink an upstream UX review.

*Considered and rejected: deriving the project from the VCS root.* A git worktree's own root is still the worktree dir (the `.git` file lives there), so recovering the main project name requires resolving the git common dir — VCS resolution in the namespace hot path, which must be cwd-independent and runs per config file per invocation. Renaming the main checkout would silently re-namespace every worktree; jj needs a parallel path; plain duplicate clones get nothing. Pitchfork's project boundary is the config file, and `namespace` is the identity it declares — codify that rather than importing git's project model.

*Considered and rejected: per-checkout naming knobs.* A `worktree_name` key (settable per checkout, hash as fallback) was floated and dropped — nobody names every ephemeral worktree, so it's API surface whose happy path is "never set it". An env var suffix is disqualified outright: the supervisor re-derives daemon IDs from config files in its own environment (file-watch, hooks, autostop), so per-shell identity input makes CLI-derived and supervisor-derived namespaces diverge — daemons silently lose config-driven features. Identity must come from the filesystem, not the environment.

*Resolved design:*

* Require an explicit `namespace` to enable isolation; fail fast otherwise. Isolation is meaningless without a stable cross-checkout base, and the requirement prevents branch-named namespace gibberish (`fix-bug-<hash>`).
* Refinement (final design): when isolation is on, the explicit `namespace` is resolved with the same sibling-file scan as the flag itself — it may be declared in any of the project dir's four config files and applies to all of them; divergent explicit values across siblings are a hard error. This (a) lets the flag live in untracked `pitchfork.local.toml` while `namespace` stays in the committed file, and (b) makes the `-l` single-namespace filter provably correct, superseding the earlier "filter by namespace set" fix — with sibling agreement enforced, the set always has exactly one element.
* Rename the flag to `namespace_per_worktree = true` — "worktree" is upstream's established vocabulary (`proxy.worktree`, PR #448), and the flat additive key leaves the existing `namespace` machinery untouched. It also works for plain duplicate clones; that's a docs footnote. Float the structured alternative `namespace = { name = "search-lib", worktree = true }` in the upstream discussion: it has in-repo precedent (`port` is already an int-or-table union) and, with `name` required in table form, schema-enforces the explicit-namespace requirement — but it reworks the raw single-key parser, the base/local "values must match" check, sibling comparison, and write round-trips on a stable existing key, so restructuring `namespace` should be jdx's call, not a drive-by contributor's. (Literal syntax symmetry with `proxy.worktree` isn't achievable anyway — that's a runtime setting, the wrong layer for parse-time identity.)
* The path hash stays as the sole suffix. Treat readability as a display concern, not an identity concern: the hash only needs to be recognizable, and the daemon's recorded `dir` (already in state, used by autostop) tells the user which checkout is which.
* Display improvements to carry the UX: a directory column in `pitchfork list` (`--dirs` flag or always-on with `$HOME` → `~`; src/cli/list.rs currently shows only Name/PID/Status/Proxy/Error), and optionally a `pitchfork namespace` command printing the current dir's resolved namespace (the web API has this at src/web/routes/api/namespaces.rs; the CLI has no equivalent). Short-ID resolution inside a checkout already means the hash is rarely typed.

*Configuration templates (docs/guides/configuration-templates.md):*

* `{{ id }}` and `{{ namespace }}` already include the hash by construction — the template context is built from the `DaemonId` (src/template.rs:69-71), which is qualified with the hashed namespace at parse time. Short-name refs like `{{ daemons.redis.port }}` are gated on namespace equality (src/template.rs:99) and keep working within a checkout, since all of its daemons share the hashed namespace.
* Add a `{{ worktree_hash }}` template variable exposing the bare suffix. The use case is isolating external resources per checkout (`DATABASE_NAME = "myapp_{{ worktree_hash }}"` — a valid unquoted identifier, unlike the full namespace) — the original resource-conflict motivation extends beyond pitchfork's own daemons. Follow the `proxy_url` precedent (src/template.rs:149-152): always inserted, value when isolated, null otherwise, so `| default(value=...)` degrades gracefully.
* Caveat for docs: the qualified template form `{{ daemons["ns.name"].port }}` cannot portably reference an isolated namespace from committed config (the key would contain the hash) — same class as the cross-namespace `depends` limitation.
* Tests required (no template test currently touches isolation, and template.rs unit tests construct `DaemonId`s directly so they cannot catch a wiring break): parse an isolated config and assert `{{ id }}`/`{{ namespace }}` render hashed values; short-name daemon refs between two daemons in one isolated checkout; `{{ worktree_hash }}` value/null semantics; an e2e with two checkouts whose `run`/`env` embed `{{ id }}` and a `{{ worktree_hash }}`-derived resource name, asserted via logs; one e2e hook assertion (hooks are re-rendered at fire time by the supervisor) confirming the supervisor-side render also sees the hashed namespace.

## Implementation notes (2026-06-11)

The pre-flight plan below was implemented on this branch. One significant gap was discovered by the new template e2e test, beyond what the review predicted:

*Supervisor-side config resolution gap.* Hooks, file-watching, and autostop resolve daemon configs via `all_merged_all_namespaces()` — the supervisor's own cwd chain plus the global `[namespaces]` registry. A per-worktree checkout outside the supervisor's chain (every checkout except the one whose `start` auto-spawned the supervisor) was invisible: its `on_ready` hooks silently never fired, and file-watch/autostop would likewise not resolve. Fix: `start` now auto-registers the checkout's namespace → project dir in the global `[namespaces]` registry when `namespace_per_worktree` is on (mirroring what `proxy add` already does), deduped so the global config is only written when the entry is missing or stale. Consequence: the registry accumulates one entry per checkout ever started; there is still no GC for entries whose directory is gone (pre-existing gap, now noted in the docs caveats).

Also fixed opportunistically, per the review's "likely maintainer requests": the two single-key TOML parsers were consolidated into one `TopLevelOverrides` parser; the triplicated `is_global_config` guard became one `resolve_config_namespace` chokepoint; the `self_value: Option<Option<bool>>` parameter is gone; `get_local_configured_daemons` does one filesystem traversal instead of two; placeholder (not-yet-started) daemons in `list` now carry their config-resolved dir so `--dirs` works for them.

## Proposed plan

Pre-flight on this branch (before anything goes upstream):

- [x] Require explicit `namespace` when isolation is enabled (sibling-wide scan, divergent values error; fail fast with a clear error when missing)
- [x] Rename the flag to `namespace_per_worktree`; update docs, error variant, and schema accordingly (offer the `namespace = { name, worktree }` table form as an alternative in the upstream discussion)
- [x] Add a `--dirs` flag to `pitchfork list` (Dir column, `$HOME` contracted to `~`) so hash-suffixed namespaces are identifiable at a glance; add a `pitchfork namespace` command printing the current dir's resolved namespace
- [x] Add `{{ worktree_hash }}` template variable (always inserted, null when not isolated, per the `proxy_url` precedent) and the template test suite from the addendum: hashed `{{ id }}`/`{{ namespace }}` rendering, intra-checkout short-name refs, value/null semantics, two-checkout e2e, supervisor-side hook render
- [x] Document the template caveat: qualified `{{ daemons["ns.name"] }}` refs cannot portably target an isolated namespace from committed config
- [x] Run `mise run render` (or `mise run ci-dev`) and commit the regenerated `docs/public/schema.json`
- [x] Auto-register per-worktree namespaces on `start` so supervisor-side features (hooks, file-watch, autostop) resolve the checkout's config (found via the template e2e; see implementation notes)
- [x] Use conventional-commit format for the branch commits / PR title (`feat(config): ...`)
- [x] Resolve the `-l` divergent-sibling-namespace bug via sibling namespace agreement (enforced by the required-namespace rule above, so the single-namespace filter is correct by construction); collapse the double filesystem traversal in `get_local_configured_daemons`
- [x] Add a docs warning: local.toml-only opt-in does not protect against nested worktrees unless each checkout opts in; add a moved-checkout recovery paragraph (stop by qualified ID, then `pitchfork clean`)
- [x] Soften the "isolated ports" docs claim (isolation removes the already-running short-circuit; `port.bump` or port 0 still needed for port reuse)
- [x] Add `warn!` when `canonicalize()` fails for an existing dir
- [x] Add a unit test for the global-config rejection error

Battle-test checklist (before the upstream discussion):

- [ ] Run the fork against a real multi-worktree project for a while: daily `start`/`stop -l`/`list --dirs` flows, agent-created worktrees, hooks and file-watch firing per checkout
- [ ] Watch for: stale `[namespaces]` registry entries piling up, moved-checkout orphan recovery in practice, `{{ worktree_hash }}`-keyed resources (db names) behaving as expected
- [ ] Revisit the port story: confirm `port = { expect = [...], bump = N }` (or port 0) is livable for running the same stack in several checkouts at once

Then, upstream sequence:

- [ ] Open an Ideas discussion on jdx/pitchfork framed as the unsolved remainder of #199: directory-name namespaces collide across same-leaf checkouts/worktrees. Reference #448 (worktree-aware proxy) and note this makes its routing key actually disambiguate.
- [ ] Pre-empt the likely design question in the discussion: why not branch-name suffixes (unstable across branch renames/switches and detached HEAD — daemon identity in the state file can't follow live VCS state the way proxy routing can; charset issues per discussion #297; doesn't cover plain duplicate clones or jj) and why not an env var (CLI/supervisor identity divergence). Pair the hash with the `list` directory column so the readability objection is answered in the same breath
- [ ] State explicitly that a working implementation with unit + e2e tests exists and you intend to open the PR (preserves authorship against the maintainer-implements-it pattern)
- [ ] After sign-off (or ~a week of silence with no objection), open the PR; expect the Greptile bot plus possible consolidation requests (single-key parse machinery, `is_global_config` guard fold); iterate within days — the auto-closer is aggressive
- [ ] Optional sweeteners if asked: consolidate the duplicated parse machinery; per-case help text on the error variant

Fallback to path 1 (personal fork) if upstream declines or demands a redesign you don't want:

* Keep the current shape — the single-chokepoint design in `pitchfork_toml.rs` plus one helper in `batch.rs` is already close to the minimum-conflict-surface a fork can have.
* Still do the pre-flight fixes above (the `-l` set-filter bug and the docs traps affect you regardless).
* Track upstream's namespace work (gaojunran churns this area); the main rebase hazard is `src/pitchfork_toml.rs`.
* Skip the schema regen ceremony only if you never intend to PR; editor autocomplete still benefits from it.
