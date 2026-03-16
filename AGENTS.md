# AGENTS.md

This repository is being developed with heavy AI assistance, but the standard here is not "move fast and sketch vaguely." The standard is deliberate library design in a language with a powerful type system, with particular emphasis on user experience, semver discipline, and avoiding premature architectural commitments that would be hard to unwind later.

Agents working in this repository should treat this file as a philosophical primer for the work.

## What `dryice` is

`dryice` is a high-throughput transient container for read-like genomic records.

It is intended for temporary persistence, especially in workflows like external sorting, spill/reload pipelines, partitioning, and nearby genomics tasks where records need to move to disk and back quickly. It is not intended to be a general archival genomics format or a universal genomics transformation framework.

The core design center is:

- block-oriented on-disk format
- read-like records at the user boundary
- parser-agnostic core Rust API
- future Python and Node wrappers kept in mind from the beginning

## General philosophy

This project should be designed slowly and carefully before substantial implementation work begins.

The process here is intentionally "slow until fast." The goal is to spend time on:

- choosing the right primitives
- choosing the right abstractions
- deciding what belongs in the public API and what should remain private
- using diagrams and Rust sketches to pressure-test ideas before committing to them

Agents should not mistake hesitation for indecision. This is the intended process.

## The role of the agent

Your role is not just to produce plausible Rust code quickly. Your role is to help design a high-quality Rust library.

That means you should:

- help surface tradeoffs clearly
- sketch usage examples, not just internal types
- prefer API-first thinking when evaluating library designs
- be willing to do hard design work when it creates real user benefit
- keep semver consequences in mind before proposing public types or traits
- avoid pushing speculative internal architecture into the public API

You should not:

- rush to implementation before the design feels stable enough
- optimize for the shortest path to "some code exists"
- treat internal type sketches as inherently more important than user call sites
- reach for dynamic dispatch in hot per-record paths

## Rust library design philosophy

Rust's type system is a feature to be used, not feared.

Library authors in Rust should be willing to do substantial internal and type-level design work if it produces a meaningfully better user experience for downstream developers. Many of the best Rust libraries do exactly this.

The standard is not:

- avoid complexity at all costs

The standard is:

- spend complexity where it creates durable user-facing clarity, safety, or power
- avoid complexity that only serves internal elegance or speculative flexibility

In other words:

```text
be conservative about public commitments
but aggressive about doing hard design work
when it creates real user-facing benefit
```

Agents should not shy away from:

- sophisticated builder APIs
- carefully designed trait boundaries
- type-state or strongly typed configuration where it truly improves UX
- internal complexity that lets the public API remain cleaner and safer

But agents should also avoid:

- cleverness for its own sake
- generic extension surfaces that are not yet justified
- exposing internal complexity prematurely

## User experience comes first

When sketching APIs, always show the usage side.

It is not enough to sketch internal structs, traits, or module names. The most important question is often:

- what does it look like to use this library?

Agents should routinely include call-site sketches like:

- how a user constructs a writer
- how a user configures encodings
- how a user reads records from a reader
- how a user with `noodles` or another parser would integrate with the trait boundary

If an API sketch looks elegant internally but awkward at the call site, that is a design smell.

## Public API boundaries

The core crate should be parser-agnostic.

That means the public API should avoid coupling to one ecosystem's record types. In particular, agents should be very cautious about exposing types from external Rust bioinformatics libraries in the public surface.

Current direction:

- write-side boundary centered on a trait like `SeqRecordLike`
- read-side API likely yields a crate-provided row-wise `SeqRecord`
- concrete internal representations remain private unless there is a strong reason to expose them

Do not assume that internal types should become public just because they exist.

## Internal design center

The likely internal ownership center is block-oriented state, not an elaborate internal record hierarchy.

The main internal work happens around:

- block assembly
- block-local encoding state
- record index construction
- block decode state
- extraction of row-wise output records

Agents should avoid inventing unnecessary internal per-record abstractions before the pipeline clearly requires them.

## Performance expectations

This library is intended for high-throughput genomics work.

As a result:

- per-record dynamic dispatch in hot paths is unacceptable
- visitor-style APIs should be treated with caution if they harm ergonomics or throughput
- traits are most attractive on the write/input side where static dispatch works naturally
- iterator-shaped read APIs are preferable when possible

Performance concerns should be real, not theatrical. Use them to shape APIs honestly, not to justify premature low-level complexity everywhere.

## Builders and configuration

User-facing configuration is likely to benefit from builders.

Current direction:

- flat builder surface for users
- internally grouped configuration objects
- sensible defaults where possible
- built-in options first, extension points later

The `bon` crate is a serious candidate for public config builders in this repo. Agents should keep it in mind for option-rich public configuration objects, while avoiding unnecessary proc-macro complexity on tiny internal-only types.

Agents should be alert for places where:

- plain builders are enough
- `bon` improves ergonomics substantially
- typestate builders would genuinely prevent invalid states and improve user experience

Do not assume typestate is always overkill. Do not assume it is always warranted either.

## Extensibility philosophy

The project should leave room for future growth without exposing speculative extension mechanisms too early.

Example current stance:

- public accelerator API should start with concrete built-ins such as sort-key-oriented choices
- internal layout should still leave room for multiple optional accelerator sections
- user-defined/plugin-like accelerator APIs are deferred until there is real evidence for what they should mean

This is a general pattern for the repo:

- concrete built-ins first
- internal room for growth
- extension surfaces only when justified

## Wrappers matter now

Python and Node wrappers are expected later.

That means agents should think now about:

- which public types are easy to wrap
- which APIs are FFI-hostile
- where lifetimes and generics might leak too far into the public surface
- how errors and configuration types would translate into other ecosystems

You do not need to design the wrappers now, but you do need to avoid making them artificially hard later.

## Documentation and design artifacts

This repository uses `.agents/` for planning and design artifacts that are not necessarily intended to live in version control forever.

Agents should treat those documents as part of the design process. Update them when meaningful design decisions are made so that decisions do not live only in chat context.

At the same time, avoid rewriting them noisily after every small conversational turn. Prefer:

- small updates after genuine convergence
- larger consolidations after a design area becomes materially clearer

## Current design artifacts

At the time of writing, the core planning stack includes:

- `.agents/dryice-design-first-pass.md`
- `.agents/dryice-rust-architecture-plan.md`
- `.agents/dryice-rust-design-notebook.md`

Agents should read and respect these before proposing major architectural shifts.

## Workflow expectations

The development workflow should be cohesive and tool-driven.

Agents should prefer:

- `just` recipes over ad hoc command improvisation
- workspace-level linting and checking
- explicit command paths for formatting, testing, and docs
- repository structures that make intended workflows obvious to both humans and agents

Where possible, shape the project so that the tooling and type system make mistakes harder to express in the first place.

## Final attitude

Treat this project like a real Rust library with a real future, not a disposable prototype.

That means:

- think carefully
- sketch concretely
- show usage
- do the hard design work when it helps users
- keep internals flexible and public APIs deliberate

The project should not be rushed past the design stage just because code generation is easy.
