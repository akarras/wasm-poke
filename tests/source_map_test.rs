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
    
    // Find small_vec_test
    let func = info.functions.iter()
        .find(|f| f.best_name().contains("small_vec_test"))
        .expect("small_vec_test not found");
        
    let body_range = func.body_range.as_ref().expect("no body range");
    let body_len = body_range.end - body_range.start;
    
    let mut mapped_count = 0;
    let mut last_line = 0;
    let mut jumps_to_start = 0;
    
    // Sample instructions (every few bytes)
    for offset in (0..body_len).step_by(5) {
        if let Some(loc) = map_instr_to_source_fast(&info, &bytes, func.index, offset) {
            mapped_count += 1;
            println!("Offset {}: {} : {}", offset, loc.file, loc.line);
            
            // Check for "rough" mapping bug: jumping back to start line (e.g. line 6 or 7)
            // when we are deep in the function.
            // Only check if the file is lib.rs (to avoid inlined code from other crates/files)
            if loc.file.ends_with("lib.rs") {
                // Ignore line 0 (missing info)
                if offset > 50 && loc.line > 0 && loc.line <= 6 {
                    jumps_to_start += 1;
                }
            }
            
            last_line = loc.line;
        }
    }
    
    println!("Mapped {}/{} sample points", mapped_count, body_len / 5);
    println!("Jumps to start: {}", jumps_to_start);
    
    // We expect some mappings
    assert!(mapped_count > 0);
    
    // If the bug is present, we expect jumps_to_start to be high.
    // We want to assert that it is LOW (ideally 0) after fix.
    assert!(jumps_to_start == 0, "Found {} incorrect jumps to function start", jumps_to_start);
}

#[test]
fn test_cross_file_mapping() {
    let (info, bytes) = load_wasm();
    
    // Find vec_test which calls other::do_something (inlined)
    let func = info.functions.iter()
        .find(|f| f.best_name().contains("vec_test"))
        .expect("vec_test not found");
        
    let body_range = func.body_range.as_ref().expect("no body range");
    let body_len = body_range.end - body_range.start;
    
    let mut found_other_rs = false;
    
    for offset in 0..body_len {
        if let Some(loc) = map_instr_to_source_fast(&info, &bytes, func.index, offset) {
            if loc.file.ends_with("other.rs") {
                found_other_rs = true;
                break;
            }
        }
    }
    
    assert!(found_other_rs, "Should find mapping to other.rs due to inlining");
}
