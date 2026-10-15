# Releasing

rustual uses [Semantic Versioning](https://semver.org/). The Rust crates (`rustual-render`,
`rustual-css`, `rustual`) and the Python package (`rustual-py`) share one version, set in the
workspace `Cargo.toml`; `scripts/release.py` keeps every other place in sync.

## Branches

| Branch | Version | Purpose |
|---|---|---|
| `dev` | `X.Y.Z.dev0` (Cargo: `X.Y.Z-dev.0`) | day-to-day work; `X.Y.Z` is the *next* release |
| `main` | `X.Y.Z` | releases only: every commit is a tagged release `vX.Y.Z` |

CI checks this on every push (`python scripts/release.py check`).

## Day to day

Work on `dev` (directly or through pull requests into `dev`) and add a line to the
`## Unreleased` section of [`CHANGELOG.md`](CHANGELOG.md) for anything users will notice.

After a release, `dev` plans the next **patch** release. When the changes call for more, re-plan:

```sh
python scripts/release.py plan minor   # new features, backwards compatible  → X.(Y+1).0.dev0
python scripts/release.py plan major   # breaking changes                    → (X+1).0.0.dev0
python scripts/release.py plan patch   # back to a bug-fix release           → X.Y.(Z+1).dev0
python scripts/release.py status       # what's planned
```

(Before 1.0, breaking changes bump the minor version, as is usual for 0.x crates.)

## Making a release

On an up-to-date `dev` whose CI is green:

```sh
python scripts/release.py release --dry-run   # shows what will happen
python scripts/release.py release
```

This:

1. sets the version to `X.Y.Z` and dates the `Unreleased` changelog section (`## X.Y.Z - date`),
2. merges `dev` into `main` and tags `vX.Y.Z`,
3. fast-forwards `dev` and starts `X.Y.(Z+1).dev0`,
4. pushes `main`, `dev` and the tag.

The tag starts [`.github/workflows/release.yml`](.github/workflows/release.yml), which builds the
sdist and abi3 wheels (manylinux_2_28, musllinux_1_2, Windows x86_64, macOS universal2),
smoke-tests every wheel on its platform, then publishes:

- a GitHub release with the wheels, sdist and the changelog section as notes,
- `rustual-py` on [PyPI](https://pypi.org/project/rustual-py/),
- `rustual-render`, `rustual-css` and `rustual` on [crates.io](https://crates.io/crates/rustual).

Publishing uses trusted publishing (OIDC), so no tokens are stored in GitHub, and versions that are
already published are skipped, so a failed job can simply be re-run.

## One-time setup

- **PyPI**: trusted publisher for project `rustual-py`: owner `Awareness10`, repository `rustual`,
  workflow `release.yml`, environment `pypi`.
- **crates.io**: for each crate (`rustual-render`, `rustual-css`, `rustual`), Settings → Trusted
  Publishing → GitHub: owner `Awareness10`, repository `rustual`, workflow `release.yml`,
  environment `crates-io`.
