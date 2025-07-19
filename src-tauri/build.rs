fn main() {
    // ts-rs 출력 디렉토리 설정
    println!("cargo:rustc-env=TS_RS_EXPORT_DIR=../src/types/generated");
    
    tauri_build::build();
}
