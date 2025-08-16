//! TypeScript 타입 생성 전용 바이너리
//!
//! Phase 4: ts-rs 기반 자동 타입 생성

use matter_certis_v2_lib::new_architecture::ts_gen;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Starting TypeScript type generation...");

    // 타입 생성 실행
    ts_gen::generate_ts_bindings()?;

    println!("✅ All TypeScript types generated successfully!");
    Ok(())
}
