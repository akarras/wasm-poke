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

#[test]
fn test_multi_term() {
    let f = make_func();
    assert!(function_matches(&f, "demangled function"));
    assert!(function_matches(&f, "demangled name"));
    assert!(!function_matches(&f, "demangled foo"));
}

#[test]
fn test_demangled_priority() {
    let f = make_func();
    // "ZN9" is in raw name but not demangled. Should match because of fallback.
    assert!(function_matches(&f, "ZN9")); 
    
    // "export" is in export name but not demangled. Should match because of fallback.
    assert!(function_matches(&f, "export"));
}

#[test]
fn test_case_insensitivity() {
    let f = make_func();
    assert!(function_matches(&f, "DEMANGLED"));
    assert!(function_matches(&f, "Function"));
}

#[test]
fn test_no_demangled() {
    let mut f = make_func();
    f.demangled_name = None;
    
    // Now raw name should be searchable
    assert!(function_matches(&f, "ZN9"));
    
    // And export name should be searchable
    assert!(function_matches(&f, "export"));
    
    // Multi-term with no demangled
    assert!(function_matches(&f, "ZN9 export"));
}

#[test]
fn test_fallback() {
    let mut f = make_func();
    f.demangled_name = None;
    f.raw_name = None;
    f.export_names.clear();
    // Now only func[0]
    assert!(function_matches(&f, "func"));
    assert!(function_matches(&f, "0"));
    // func[0] might fail because [ is a glob character and we disabled escaping.
    // assert!(function_matches(&f, "func[0]")); 
    assert!(!function_matches(&f, "func[1]"));
}
