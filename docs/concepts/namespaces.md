# Namespaces

How Pitchfork handles daemons with the same name across different projects.

## The Problem

When working with multiple projects, you might have daemons with the same name in different directories:

```
~/projects/
├── frontend/
│   └── pitchfork.toml    # defines "api" daemon
└── backend/
    └── pitchfork.toml    # also defines "api" daemon
```

Without namespacing, these would conflict. Pitchfork solves this by automatically qualifying daemon IDs with a namespace.

## Daemon ID Format

Pitchfork uses two forms of daemon IDs:

| Format | Example | Description |
|--------|---------|-------------|
| Short ID | `api` | Just the daemon name |
| Qualified ID | `frontend/api` | Namespace + daemon name |

Namespace derivation rules:

- `~/.config/pitchfork/config.toml` and `/etc/pitchfork/config.toml` use namespace `global`
- Project configs use top-level `namespace = "..."` when provided
- Otherwise project configs use the parent directory name of the config file
- If the derived directory namespace is invalid (e.g. contains `--`, spaces, or non-ASCII), loading fails with a clear error and you should set `namespace`

Example override:

```toml
namespace = "my-project"

[daemons.api]
run = "npm run dev"
```

## Using Short IDs

When you're in a project directory, you can use short IDs:

```bash
cd ~/projects/frontend
pitchfork start api        # Starts frontend/api
pitchfork status api       # Shows status of frontend/api
pitchfork logs api         # Shows logs for frontend/api
```

Pitchfork resolves short IDs in this order:

1. Prefer the current directory namespace
2. If not found locally, use a unique match from merged config
3. If `global/<id>` exists in merged config, use it
4. Otherwise return a not-found error
5. If multiple matches exist, return an ambiguity error and require `namespace/name`

## Using Qualified IDs

From any directory, you can use fully qualified IDs:

```bash
# From anywhere
pitchfork start frontend/api
pitchfork status backend/api
pitchfork logs frontend/api
```

This is useful when:
- Operating from outside the project directory
- Managing daemons from multiple projects at once
- Avoiding ambiguity when the same short name exists in multiple projects

Qualified IDs are parsed directly and work even when there is no local `pitchfork.toml`.

## Per-Worktree Namespaces

By default, the namespace identifies the *project*, so two checkouts of the same project (e.g. git worktrees) resolve to the same namespace and collide:

```
~/src/myproj                          # namespace "myproj"
~/.worktrees/c015/myproj              # also "myproj" — collides!
```

With a shared namespace the second checkout sees the first checkout's daemons as its own: `pitchfork start` reports them as already running, and `pitchfork stop -l` stops the other checkout's stack.

Set `namespace_per_worktree = true` to give every checkout of the project its own namespace, regardless of where it lives. It requires an explicit `namespace` (the stable cross-checkout base — directory names can't provide one, since a worktree's directory is often named after a branch):

```toml
namespace = "myproj"
namespace_per_worktree = true

[daemons.api]
run = "npm run dev"
```

The namespace becomes `<namespace>-<hash>`, where `<hash>` is a stable 8-character suffix derived from the checkout's canonical path:

```
$ cd ~/src/myproj && pitchfork start api
$ cd ~/.worktrees/c015/myproj && pitchfork start api
$ pitchfork list --dirs
myproj-ee0d194f/api  12345  running  ~/src/myproj
myproj-e4c3d83b/api  12346  running  ~/.worktrees/c015/myproj
```

Behavior with `namespace_per_worktree` enabled:

- Each checkout gets an isolated set of daemons and logs under one shared supervisor
- The suffix is stable: the same checkout path always produces the same namespace
- `start -l` / `stop -l` / `restart -l` narrow to exactly the current checkout's namespace, so a worktree nested *inside* another checkout (e.g. `myproj/.worktrees/feature`) never touches its parent's daemons
- Short IDs still work: from inside a checkout, `pitchfork start api` resolves to that checkout's daemon
- `pitchfork namespace` prints the current checkout's namespace; `pitchfork list --dirs` shows which directory each daemon belongs to
- Both keys may live in any of the project's config files (`pitchfork.toml`, `pitchfork.local.toml`, or the `.config/` variants) and apply to all of them; conflicting values are an error
- Putting the flag in an untracked `pitchfork.local.toml` opts in a single checkout without committing anything (the committed `namespace` is picked up from `pitchfork.toml`)
- On `start`, the checkout's namespace is registered in the global `[namespaces]` registry so the supervisor can resolve its config for hooks, file-watching, and autostop
- In templates, `{{ id }}` and `{{ namespace }}` render the per-checkout values, and `{{ worktree_hash }}` exposes the bare suffix for isolating external resources (database names, socket paths) per checkout — see [Configuration Templates](/guides/configuration-templates)

Caveats:

- Ports are not allocated by namespace: to run the same daemon in several checkouts simultaneously, use `port = { expect = [...], bump = N }` or port `0` so each instance can find a free port
- The hash is derived from the checkout's path: if you move or rename a checkout while its daemons run, they remain under the old namespace. Stop them by qualified ID from `pitchfork list`, then `pitchfork clean`
- If the parent checkout opted in only via an untracked `pitchfork.local.toml`, a nested worktree does not inherit the flag (the file isn't part of the checkout) — commit the flag, or opt in each checkout, to get the nested `stop -l` protection
- Registered `[namespaces]` entries and log directories for deleted checkouts are not garbage-collected
- Cross-namespace references in committed config (`depends = ["myproj/db"]`, qualified template keys) cannot portably target a per-worktree namespace, since the hash differs per checkout

## Display Behavior

Pitchfork intelligently shows or hides namespaces in output:

**When there's no conflict** (only one daemon named `api`):
```
$ pitchfork list
api  12345  running
```

**When there's a conflict** (multiple daemons named `api`):
```
$ pitchfork list
frontend/api  12345  running
backend/api   12346  running
```

## Naming Rules

Daemon IDs have the following restrictions:

| Rule | Valid | Invalid |
|------|-------|---------| 
| No double dashes | `my-app` | `my--app` |
| No slashes in short ID | `api` | `api/v2` |
| Single slash for qualified ID | `project/api` | `a/b/c` |
| No spaces | `my_app` | `my app` |
| No parent references | `myapp` | `../etc` |
| No leading/trailing dashes | `my-app` | `-app` or `app-` |
| ASCII alphanumeric, `_`, `-`, `.` only | `myapp123` | `myäpp` or `app@v1` |

The `--` sequence is reserved for internal path encoding (converting `namespace/daemon` to `namespace--daemon` for filesystem storage).

Because of this, project directory names containing `--` (or other invalid namespace characters) require an explicit top-level `namespace` override.

## Path Encoding

Internally, Pitchfork converts qualified IDs to filesystem-safe paths:

| Daemon ID | Log Directory | Log File |
|-----------|---------------|----------|
| `frontend/api` | `logs/frontend--api/` | `frontend--api.log` |
| `my-project/web-server` | `logs/my-project--web-server/` | `my-project--web-server.log` |
| `global/postgres` | `logs/global--postgres/` | `global--postgres.log` |

This encoding is transparent to users—you always use `/` in commands, and Pitchfork handles the conversion automatically.

## Examples

### Managing Multiple Projects

```bash
# Start services in both projects
cd ~/projects/frontend && pitchfork start api
cd ~/projects/backend && pitchfork start api

# Check status of all daemons
pitchfork list
# Output:
# frontend/api  12345  running
# backend/api   12346  running

# View logs for a specific project's daemon
pitchfork logs frontend/api

# Stop a specific daemon from anywhere
pitchfork stop backend/api
```

### Working Within a Project

```bash
cd ~/projects/frontend

# Short IDs work here
pitchfork start api
pitchfork logs api
pitchfork stop api
```

### Global Configuration

Daemons defined in `~/.config/pitchfork/config.toml` use the `global` namespace:

```bash
pitchfork start global/postgres
pitchfork logs global/redis
```
