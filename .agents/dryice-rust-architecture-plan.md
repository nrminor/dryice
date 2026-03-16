# `dryice` Rust architecture plan

## Purpose of this document

This document turns the `dryice` format plan into a first-pass Rust project plan.

It focuses on repository and crate structure, public API boundaries, and the development tooling conventions that should make the project pleasant for both humans and agents to work on. It intentionally stays one level above concrete implementation details of the file format.

## Core architectural position

`dryice` should begin as a single Rust library crate inside a workspace root.

That core crate should define:

- the `dryice` data model
- the on-disk format vocabulary
- block-oriented reader and writer machinery
- codec selection and internal dispatch
- optional accelerator arrays such as sort keys

It should not define itself around any one external FASTQ or sequencing parser ecosystem.

That is the most important architectural constraint.

## Why the project should start from scratch

The current repository layout reflects an older idea:

- a `dryice` binary stub
- a `libdryice` crate containing the actual code
- a `dryice-macros` crate that does not yet appear justified

That shape does not match the project we now want to build.

The stronger plan is:

- keep the repository
- keep the name
- keep the planning documents
- treat the old Rust code as disposable scaffolding
- rebuild the crate structure around the current design center

This is cleaner than trying to preserve prototype decisions that were made under a different abstraction.

## Workspace shape

The workspace should exist even if there is only one real crate at first.

That gives us a stable root for:

- future bindings crates
- future adapter crates
- shared lint and profile settings
- one cohesive `just`-driven workflow

### Near-term target

```text
dryice/
+- Cargo.toml
+- Cargo.lock
+- justfile
+- .gitignore
+- README.md
+- rustfmt.toml
+- .agents/
+- dryice/
   +- Cargo.toml
   +- src/
      +- lib.rs
      +- ...
```

### Likely later target

```text
dryice/
+- Cargo.toml
+- Cargo.lock
+- justfile
+- .gitignore
+- README.md
+- rustfmt.toml
+- .agents/
+- dryice/
|  +- Cargo.toml
|  +- src/
+- dryice-python/
|  +- Cargo.toml
|  +- src/
+- dryice-node/
|  +- Cargo.toml
|  +- src/
+- maybe adapter crates later if they earn their keep
```

### What should not exist early

- a macros crate without a concrete macro use case
- a CLI crate before there is a real command-line product
- format-parser-specific crates before the core API boundary is proven

## Crate responsibility

The `dryice` crate should be the canonical home of the format and runtime model.

Its public API should be built from types the project owns.

That means the public surface should prefer:

- `dryice` record and block types
- `dryice` codec enums and config structs
- `dryice` reader and writer types
- `std` traits and primitives where helpful

And it should avoid exposing in the public API:

- `noodles` record types
- `bio-seq` types
- parser-specific traits from outside the crate
- async runtime-specific types unless absolutely necessary
- deep generic APIs whose ergonomics mostly serve internal flexibility rather than user clarity

## Why parser independence matters

`dryice` needs to be usable by Rust users coming from different parts of the bioinformatics ecosystem.

Some users may prefer:

- `noodles`
- `rust-bio`
- ad hoc FASTQ parsing
- internal pipeline-specific record models

If the core `dryice` crate bakes in one of those ecosystems, it immediately becomes less portable and more politically costly to adopt.

The right stance is:

```text
external parser ecosystem
        |
        v
adapter / conversion layer
        |
        v
owned dryice record model
        |
        v
dryice reader/writer/format core
```

That separation keeps the core crate stable and makes future bindings much easier.

## Binding-friendly API design

Even though Python and Node wrappers are not immediate implementation targets, they should influence the Rust API now.

If the core crate is hard to wrap, it is probably also too complicated for ordinary Rust users.

### Good signs

- public structs and enums are owned by `dryice`
- user-facing config is carried in explicit config structs
- errors are structured and translate cleanly
- APIs can be driven with bytes, slices, and buffers under our control
- the crate can operate without requiring users to adopt a specific runtime or parser library

### Warning signs

- lifetimes dominate the top-level public API
- external generic types leak into core signatures
- users must implement a complicated trait stack just to write a block
- a parser crate's record type becomes the de facto canonical input type

The wrappers should be able to sit on top of a clear Rust boundary, not reverse-engineer one.

## Module tree for the core crate

The `dryice` crate should be organized around native container concepts, not around external sequencing formats.

### Proposed first-pass module tree

```text
src/
+- lib.rs
+- error.rs
+- record.rs
+- format/
|  +- mod.rs
|  +- header.rs
|  +- section.rs
|  +- version.rs
+- block/
|  +- mod.rs
|  +- header.rs
|  +- index.rs
|  +- view.rs
|  +- builder.rs
+- codec/
|  +- mod.rs
|  +- sequence.rs
|  +- quality.rs
|  +- name.rs
+- io/
|  +- mod.rs
|  +- reader.rs
|  +- writer.rs
|  +- options.rs
+- accelerator/
|  +- mod.rs
|  +- key.rs
|  +- kind.rs
```

This layout is intentionally modest. It is enough to reflect the format design without exploding into one module per imagined future feature.

## What each module should mean

### `lib.rs`

The crate root should stay thin.

It should do three things well:

- provide crate-level documentation
- declare modules
- re-export a small, deliberate public surface

It should look more like your `labkey-rs` and `sra-taxa-core` roots than like a prototype dumping ground.

### `record.rs`

This should define the read-like record vocabulary that the crate owns.

This is where to put concepts like:

- owned record
- borrowed record view
- optional name/sequence/quality presence rules
- small per-record flags under project control

This module is important because it creates a stable internal language that is not borrowed from another crate.

### `format/`

This should define the file-level layout vocabulary.

Examples:

- file header types
- section kinds
- version markers
- layout flags

This is the schema-facing side of the crate, separate from the runtime machinery that reads and writes it.

### `block/`

This should define block-level structure and traversal.

Examples:

- block headers
- fixed-width record index entries
- block views over encoded bytes
- block builders or encoders

This is likely the structural heart of the crate.

### `codec/`

This should define codec kinds and implementations for sequence, quality, and names.

The design should keep the distinction between:

- codec identity and configuration
- encoded section layout
- actual encode/decode implementation

This separation will matter later if exact and lossy codecs evolve independently.

### `io/`

This should define the operational read/write interface.

Examples:

- block readers
- block writers
- format options
- sequential traversal APIs

The important thing is that this module be about `dryice` IO, not generic genomics input parsing.

### `accelerator/`

This should define optional side arrays, especially the ones that matter for sorting and partitioning.

This area should start narrow. The goal is not to invent a universal sidecar system on day one.

## Public API rules

The first version of the crate should follow these rules.

1. Public types should be project-owned.
2. The crate root should re-export only the small set of types users most often need.
3. Core APIs should not require users to buy into one parser ecosystem.
4. Internal implementation crates, if they ever exist, should not leak into the public API.
5. Features should only gate real optional capability, not paper over architectural uncertainty.

## Adapter strategy

The safest path is to keep adapters out of the core crate until there is pressure to add them.

That means the early project should probably not contain modules like:

- `fastq.rs`
- `bam.rs`
- `fasta.rs`

inside the core `dryice` library.

If later adapter work is justified, it will likely belong in one of two places:

- a lightweight `adapters/` area that is clearly peripheral
- separate crates that depend on `dryice`

But that decision should come after the core record model and writer/reader API exist.

## Workspace manifest style

Your other repos show a consistent pattern that fits this project well.

The workspace root should carry:

- resolver version
- shared package metadata where useful
- shared clippy lint policy
- shared profiles

### Recommended root `Cargo.toml` sketch

```toml
[workspace]
resolver = "3"
members = ["dryice"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/nrminor/dryice"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
perf = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
complexity = { level = "warn", priority = -1 }
correctness = { level = "warn", priority = -1 }
unwrap_used = "deny"

[profile.dev]
incremental = true
lto = false

[profile.release]
lto = true
codegen-units = 1
strip = true
```

This is intentionally closer to your recent repos than to the current `dryice/Cargo.toml`.

In particular:

- `resolver = "3"` should replace the current older resolver
- shared clippy policy should be restored
- the release profile should optimize for ordinary Rust release builds, not size-first binary tuning inherited from another project shape

## Core crate manifest style

The core crate manifest should be lean and use workspace inheritance where it helps.

### Recommended `dryice/Cargo.toml` sketch

```toml
[package]
name = "dryice"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "High-throughput transient container for read-like genomic records"

[dependencies]
thiserror = "2"

[lints]
workspace = true
```

That is not the final dependency list. It is a style statement:

- the crate starts small
- dependencies are added only when they have a clear reason to exist
- the manifest should reflect the real architecture, not legacy experiments

## `justfile` strategy

Your recent repos are doing the right thing here.

For agent-heavy workflows, `just` is not just a convenience layer. It is a way to make the intended development path explicit and repeatable.

That matters because agents are much more reliable when:

- there is one blessed path for formatting, linting, testing, and docs
- the fast feedback loops are easy to discover
- the repository itself encodes expected workflow steps

### Strong patterns worth carrying over

- `default` and `choose`
- `check` and `check-all`
- `fmt-check`, `fmt`, `lint`, `test`, `doc-check`
- `prepare-commit` and `prepare-push`
- a small number of setup and utility recipes

### Recommended initial `justfile` sketch

```just
# dryice project justfile
# All repeating commands should be recipes here.
# Agents MUST read and use these recipes.

default:
    @just --list

choose:
    @just --choose

# === Development Workflow ===

check: fmt-check lint test doc-check
    @echo "All checks passed"

check-all: fmt-check lint-all test-all doc-check
    @echo "All checks passed on full codebase"

# === Formatting ===

fmt-check:
    cargo fmt --all -- --check

fmt:
    cargo fmt --all

# === Linting ===

lint:
    cargo clippy --all-targets --all-features -- -D warnings

lint-all:
    cargo clippy --all-targets --all-features -- -D warnings

# === Testing ===

test:
    cargo nextest run --all-features --no-tests=pass

test-all:
    cargo nextest run --all-features --run-ignored all --no-tests=pass

# === Building ===

build:
    cargo build

build-release:
    cargo build --release

check-compile:
    cargo check --all-targets --all-features

# === Documentation ===

doc-check:
    cargo doc --no-deps --document-private-items

doc:
    cargo doc --no-deps --open

# === jj Workflow ===

prepare-commit: check
    @echo ""
    @echo "Ready to commit. Run: jj commit -m 'your message'"
    @jj status

prepare-push: check-all
    @echo ""
    @echo "Ready to push. Run: jj git push"

status:
    jj status

log:
    jj log

# === Utility ===

clean:
    cargo clean

update:
    cargo update

sloc:
    @tokei --types=Rust --compact
```

This should be enough to start. Reference-repo recipes can be added later only if they are genuinely useful.

## Why this tooling matters for agents

Rust already gives strong correctness pressure through:

- types
- ownership
- trait bounds
- borrow checking
- structured compiler diagnostics

Cargo adds:

- reproducible dependency resolution
- workspace-wide checking
- first-class test, doc, and lint integration

The `justfile` then adds the missing social layer:

- which commands matter most
- which checks are expected before changes are considered healthy
- which commands agents should prefer over ad hoc shell improvisation

The design goal is not just convenience. It is to make the intended workflow the path of least resistance.

## `.gitignore` style

Your deny-by-default `.gitignore` pattern is unusual but coherent, and I think it fits this repo if you want the same high-discipline workflow here.

The current `.gitignore` in `dryice` is more permissive, more legacy-shaped, and already out of date with the architecture we want.

### Recommended direction

- use deny-by-default
- explicitly allow tracked files and directories
- keep the root manifest, lockfile, `justfile`, and agent docs visible
- allow only the crates and files that are meant to exist now
- avoid leaving stale allowlists for crates we intend to remove

### Recommended root `.gitignore` sketch

```gitignore
# Deny-by-default: ignore everything, explicitly allow tracked files.
# All file tracking is intentional. No glob patterns allowed.
*

# === Allowed: project configuration ===
!/.gitignore
!/Cargo.toml
!/Cargo.lock
!/justfile
!/README.md
!/LICENSE
!/rustfmt.toml
!/AGENTS.md
!/opencode.json

# === Allowed: agent configuration ===
!/.agents

# === Allowed: core crate ===
!/dryice
!/dryice/Cargo.toml
!/dryice/src

# === Allowed: GitHub workflows ===
!/.github
!/.github/workflows
```

That sketch is intentionally incomplete. The finished file should enumerate the actual tracked files once the restructure is real.

The important point is philosophical: the `.gitignore` should describe the intended repository shape, not a museum of every experiment the repo has ever contained.

## Nix flake stance

I do not think a flake should be part of the immediate restructuring target.

Right now, the strongest value is likely coming from:

- Cargo workspace discipline
- a good `justfile`
- explicit crate boundaries
- clean dependency choices

A flake becomes much more compelling when one or more of these become true:

- there are C or C++ dependencies that need consistent provisioning
- dynamic linking becomes part of normal development
- Python or Node bindings add platform-specific build complexity
- CI and local developer environments start drifting materially

So the right current stance is:

- do not add Nix just to feel sophisticated
- keep the project simple enough that adding a flake later is straightforward if the need becomes real

## Concrete restructuring recommendation

The next repository change should aim for this destination:

```text
workspace root
+- one real crate: dryice
+- shared workspace metadata and lint policy
+- one cohesive justfile-driven workflow
+- deny-by-default gitignore aligned to the real repo shape
+- no binary stub crate
+- no macros crate
+- no legacy libdryice crate
```

And inside the `dryice` crate:

```text
crate organized around container concepts
not around external genomics file formats
not around parser ecosystem loyalties
not around speculative future crates
```

## Decisions that feel solid already

- `dryice` should start as one library crate.
- The workspace root should remain in place to support future crates.
- The public API should expose project-owned types.
- Parser-library coupling should stay out of the core public boundary.
- Python and Node wrapper friendliness should influence API design now.
- A `justfile` should define the standard development workflow from the start.
- The root manifest should follow the same broad style as your recent Rust repos.
- A deny-by-default `.gitignore` is a good fit if you want the same intentional tracking discipline here.
- Nix is optional later, not required now.

## Open questions to settle before implementation

- What exact top-level modules belong in the first cut of `dryice/src/`?
- Should `record` distinguish owned and borrowed forms immediately, or only once real pressure appears?
- How narrow should the first accelerator model be?
- Which dependencies, if any, deserve to be in the core crate from day one beyond error handling?
- Do we want a `prelude` at all, or should that wait until the API stabilizes?
- Do we want reference-repo setup recipes in the first `justfile`, or keep it minimal at the start?

## First-pass summary

The right next move is not to massage the old repository into shape. It is to replace the old shape with one that matches the project we now understand.

That means:

- one core crate named `dryice`
- a workspace root that is ready for future bindings
- a module tree organized around container concepts
- public APIs built from project-owned types
- explicit development recipes and lint policy that guide both humans and agents

If we get that structure right now, the later format work and the eventual Python and Node wrappers should be much easier to build without architectural regret.
