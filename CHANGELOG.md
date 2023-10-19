# Changelog

## Unreleased

## 0.3.0

- Remove type parameters from `Session::{text, binary}()` methods, replacing with equivalent `impl Trait` parameters.
- `Session::text()` now receives an `impl Into<ByteString>`, making broadcasting text messages more efficient.

## 0.2.5

- TODO
