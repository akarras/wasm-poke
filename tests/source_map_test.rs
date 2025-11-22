use std::path::PathBuf;
use wasm_poke::{parse_wasm, map_instr_to_source, WasmModuleInfo};

#[test]
fn test_source_mapping_repro() {
    std::env::set_var("DEBUG_SOURCE_MAP", "1");
    let wasm_path = PathBuf::from("target/wasm32-unknown-unknown/debug/repro_crate.wasm");
    if !wasm_path.exists() {
        // Skip if fixture not built (e.g. in CI without target)
        return;
    }

    let wasm_bytes = std::fs::read(&wasm_path).expect("failed to read wasm");
    let module = parse_wasm(&wasm_path).expect("failed to parse wasm");

    // Find 'do_something' function
    let func = module.functions.iter()
        .find(|f| f.demangled_name.as_deref().unwrap_or("").contains("do_something"))
        .expect("do_something function not found");

    // Map an instruction in the body (e.g. offset 0 or near start)
    // The function body should map to 'src/other.rs'
    let loc = map_instr_to_source(&wasm_bytes, func.index, 0)
        .expect("failed to map instruction to source");

    println!("Mapped 'do_something' offset 0 to: {}:{}", loc.file, loc.line);

    assert!(loc.file.ends_with("other.rs"), "Expected mapping to other.rs, got {}", loc.file);
    // Line 4 is where do_something is defined
    assert_eq!(loc.line, 4, "Expected line 4, got {}", loc.line);

    // Check test_func
    let test_func = module.functions.iter()
        .find(|f| {
            f.demangled_name.as_deref().unwrap_or("").contains("test_func") ||
            f.export_names.iter().any(|e| e.contains("test_func"))
        })
        .expect("test_func function not found");
    
    let loc_test = map_instr_to_source(&wasm_bytes, test_func.index, 0)
        .expect("failed to map test_func");
    println!("Mapped 'test_func' offset 0 to: {}:{}", loc_test.file, loc_test.line);
    assert!(loc_test.file.ends_with("lib.rs"), "Expected mapping to lib.rs, got {}", loc_test.file);
    // Line 6 is function signature, which is acceptable for offset 0
    assert_eq!(loc_test.line, 6, "Expected line 6, got {}", loc_test.line);
}
