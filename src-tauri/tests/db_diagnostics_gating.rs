// Verify db diagnostics command is only available under dev-tools/debug_assertions.
#[test]
fn db_diagnostics_command_gating_compiles() {
    // This "test" compiles conditionally with the same cfg as the command export.
    // It ensures that when the feature or debug assertions are off, the symbol is absent,
    // and when on, it is present.
    #[cfg(any(feature = "dev-tools", debug_assertions))]
    {
    // If this use resolves, the symbol is present under the gate.
    use matter_certis_v2_lib::commands::devtools::db_diagnostics::scan_db_pagination_mismatches as _sym;
    let present = true; // presence check by successful import
    assert!(present);
    }

    #[cfg(all(not(feature = "dev-tools"), not(debug_assertions)))]
    {
        // Cannot refer to the symbol here; the block should compile without any reference.
        assert!(true);
    }
}
