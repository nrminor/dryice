# bitnuc bug: AVX packing panics on empty input

## Summary

`bitnuc::twobit::encode` panics with "attempt to subtract with overflow" when called with an empty input slice on machines that use the AVX SIMD path.

## Location

`bitnuc-0.4.1/src/twobit/packing/avx.rs:147`

## Reproduction

```rust
let mut packed: Vec<u64> = Vec::new();
bitnuc::twobit::encode(b"", &mut packed).unwrap();
```

This passes on machines that use the non-AVX fallback path but panics on machines with AVX support (e.g., GitHub Actions `ubuntu-24.04` runners).

## Stack trace

```text
thread panicked at bitnuc-0.4.1/src/twobit/packing/avx.rs:147:17:
attempt to subtract with overflow
```

## Root cause

The AVX packing code appears to compute the number of SIMD chunks from the input length without guarding against zero-length input. When the length is 0, the subtraction overflows.

## Workaround

Guard against empty input before calling `bitnuc::twobit::encode`:

```rust
if sequence.is_empty() {
    return Ok(vec![]);
}
```

## Affected version

`bitnuc 0.4.1`

## Upstream issue

Not yet filed. This document is a reminder to open an issue at https://github.com/noamteyssier/bitnuc.
