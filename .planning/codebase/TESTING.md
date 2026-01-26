# Testing Patterns

**Analysis Date:** 2026-01-26

## Test Framework

**Runner:**
- Rust built-in test framework (cargo test)
- No external test crate (no pytest, vitest, etc.)
- Standard `#[test]` attribute for test functions

**Config:**
- No explicit test configuration file
- Uses Cargo.toml features flag: `fixture-tests` (defined but not enabled by default)
- Workspace includes test fixture crates in `tests/fixtures/*`

**Run Commands:**
```bash
cargo test                    # Run all tests
cargo test -- --nocapture    # Run with println! output visible
cargo test test_mapping_accuracy    # Run specific test
cargo build --release-debug    # Build test fixtures with debug info
```

## Test File Organization

**Location:**
- Integration tests: `tests/*.rs` (co-located with main codebase, not inside src/)
- Unit tests: inline in modules with `#[cfg(test)] mod tests {}`
- Test fixtures: `tests/fixtures/` (separate crates: repro_crate, simple-wasm)

**Naming:**
- Test files: `search_test.rs`, `source_map_test.rs`
- Test functions: `test_<behavior_under_test>()` (e.g., `test_mapping_accuracy()`, `test_case_insensitivity()`)
- Helper functions: `make_<domain>()` (e.g., `make_func()`)

**Structure:**
```
tests/
├── search_test.rs          # Tests for pattern matching/filtering
├── source_map_test.rs      # Tests for DWARF source mapping
└── fixtures/
    ├── repro_crate/        # Rust crate compiled to WASM for testing
    └── simple-wasm/        # Simple WASM fixture
```

## Test Structure

**Unit Test Pattern (from `tests/search_test.rs`):**
```rust
use wasm_poke::{FunctionInfo, function_matches};

fn make_func() -> FunctionInfo {
    FunctionInfo {
        index: 0,
        code_size: 100,
        body_range: None,
        demangled_name: Some("demangled::function_name".to_string()),
        raw_name: Some("_ZN9demangled13function_nameE".to_string()),
        export_names: vec!["export_name".to_string()],
    }
}

#[test]
fn test_single_term() {
    let f = make_func();
    assert!(function_matches(&f, "demangled"));
    assert!(function_matches(&f, "function"));
}
```

**Key patterns:**
1. Helper function `make_func()` creates test data
2. Each test function is a complete, independent scenario
3. Setup is explicit and minimal in each test
4. Single responsibility: test one behavior per function

**Integration Test Pattern (from `tests/source_map_test.rs`):**
```rust
use std::path::PathBuf;
use wasm_poke::{map_instr_to_source_fast, parse_wasm_from_bytes, WasmModuleInfo};

fn load_wasm() -> (WasmModuleInfo, Vec<u8>) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target/wasm32-unknown-unknown/release-debug/repro_crate.wasm");

    let wasm_bytes = std::fs::read(&path).expect("failed to read wasm fixture");
    let info = parse_wasm_from_bytes(&wasm_bytes).expect("failed to parse wasm");
    (info, wasm_bytes)
}

#[test]
fn test_mapping_accuracy() {
    let (info, bytes) = load_wasm();
    // ... test logic
    assert!(mapped_count > 0);
    assert!(jumps_to_start == 0, "Found {} incorrect jumps to function start", jumps_to_start);
}
```

**Key patterns:**
1. Fixture loading happens in helper function (`load_wasm()`)
2. Tests use actual compiled WASM binaries
3. Iteration/sampling used for bulk validation: `for offset in (0..body_len).step_by(5)`
4. Assertions describe what went wrong in message parameter

## Inline Unit Tests

**Pattern (from `src/parser.rs`):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_invalid_bytes() {
        let bytes = b"\0asm\x01\0\0\0"; // valid header + empty module
        let res = parse_wasm_from_bytes(bytes);
        // It's valid empty module; no code section means zero functions.
        let info = res.expect("empty module should parse");
        assert_eq!(info.defined_functions, 0);
        assert_eq!(info.functions.len(), 0);
        assert_eq!(info.total_code_size, 0);
    }
}
```

**Key patterns:**
1. Inline tests in modules for unit-level validation
2. `#[cfg(test)]` gating prevents inclusion in release builds
3. `use super::*;` imports module under test
4. Minimal setup; focus on single case
5. `assert_eq!()` used for value comparison with equality

## Mocking

**Framework:** None

**Approach:** Instead of mocking:
- Use real fixture data (actual WASM binaries in `tests/fixtures/`)
- Create minimal real objects (e.g., `make_func()` constructs real FunctionInfo)
- No dependency injection; test actual functions directly

**Example (constructing test data instead of mocking):**
```rust
// From search_test.rs - construct real object instead of mock
fn make_func() -> FunctionInfo {
    FunctionInfo {
        index: 0,
        code_size: 100,
        body_range: None,
        demangled_name: Some("demangled::function_name".to_string()),
        raw_name: Some("_ZN9demangled13function_nameE".to_string()),
        export_names: vec!["export_name".to_string()],
    }
}
```

## Fixtures and Test Data

**Test Data Location:**
- Small inline fixtures: `make_func()`, `make_module()` style helpers
- Large fixtures: `tests/fixtures/repro_crate/` (full Rust crate compiled to WASM)
- Binary fixtures: Pre-compiled `.wasm` files in target directory

**Fixture Access Pattern:**
```rust
let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
path.push("target/wasm32-unknown-unknown/release-debug/repro_crate.wasm");
let wasm_bytes = std::fs::read(&path).expect("failed to read wasm fixture");
```

**Fixture Management:**
- Fixtures built separately (repro_crate compiled with wasm32 target)
- Tests expect fixtures at `target/wasm32-unknown-unknown/release-debug/`
- No automatic fixture generation; manual build required

## Coverage

**Requirements:** None enforced

**Current approach:**
- Manual testing and sampling: `for offset in (0..body_len).step_by(5)`
- All public functions have at least one test
- `search_test.rs`: 9 test cases covering pattern matching variations
- `source_map_test.rs`: 2 integration tests covering mapping accuracy
- Parser smoke test in inline module

**Observed gaps:**
- No coverage tool integration (no tarpaulin, llvm-cov)
- No coverage threshold enforcement
- Some code paths tested indirectly through integration tests

## Test Types

**Unit Tests:**
- `test_single_term()`: Single function behavior
- `test_multi_term()`: Multiple input term matching
- `test_case_insensitivity()`: Case-insensitive matching
- `test_fallback()`: Fallback naming when primary not available
- **Location:** `tests/search_test.rs`
- **Focus:** Pattern matching logic in `function_matches()`

**Integration Tests:**
- `test_mapping_accuracy()`: End-to-end DWARF source mapping across function body
- `test_cross_file_mapping()`: Verifies inlined code from other files is detected
- **Location:** `tests/source_map_test.rs`
- **Focus:** Parser + mapper working together on real WASM binary

**Smoke Tests:**
- `parse_invalid_bytes()`: Parser handles empty WASM module correctly
- **Location:** Inline in `src/parser.rs` as `#[cfg(test)] mod tests`
- **Focus:** Parser robustness

## Common Assertion Patterns

**Direct assertions:**
```rust
assert!(function_matches(&f, "demangled"));              // Boolean
assert!(!function_matches(&f, "foo"));                  // Negation
assert_eq!(info.defined_functions, 0);                  // Equality
assert_eq!(info.functions.len(), 0);                    // Collection size
```

**Expectation-based (for tests that must find results):**
```rust
let func = info.functions.iter()
    .find(|f| f.best_name().contains("small_vec_test"))
    .expect("small_vec_test not found");

let wasm_bytes = std::fs::read(&path).expect("failed to read wasm fixture");
```

**Assertion with messages:**
```rust
assert!(jumps_to_start == 0, "Found {} incorrect jumps to function start", jumps_to_start);
assert!(found_other_rs, "Should find mapping to other.rs due to inlining");
```

## Test Organization

**By domain:**
- `search_test.rs`: Tests filtering/pattern matching (function_matches)
- `source_map_test.rs`: Tests address-to-source mapping (map_instr_to_source_fast)
- `src/parser.rs` inline tests: Tests WASM parsing (parse_wasm_from_bytes)

**By test type:**
- Unit: Small isolated function behavior (search_test.rs)
- Integration: Real WASM binary behavior (source_map_test.rs)
- Smoke: Basic parser correctness (parser inline tests)

## Debugging Tests

**Running with output:**
```bash
cargo test -- --nocapture --test-threads=1
```
Prints `println!()` from tests. Example from `test_mapping_accuracy()`:
```rust
println!("Offset {}: {} : {}", offset, loc.file, loc.line);
println!("Mapped {}/{} sample points", mapped_count, body_len / 5);
println!("Jumps to start: {}", jumps_to_start);
```

**Single test execution:**
```bash
cargo test test_mapping_accuracy -- --nocapture
```

## Test Maintenance

**Fixture Compilation:**
- Requires Rust with wasm32 target: `rustup target add wasm32-unknown-unknown`
- Rebuild when fixture source changes: `cd tests/fixtures/repro_crate && cargo build --target wasm32-unknown-unknown --release-debug`
- Fixtures stored in version control (not generated)

**Test Dependencies:**
- Only dependency version pinned: wasmparser = "0.241.2"
- Tests use same dependency versions as main code
- No test-specific dependencies in Cargo.toml

---

*Testing analysis: 2026-01-26*
