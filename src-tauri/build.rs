fn main() {
    // 构建脚本把唯一构建版本注入 rustc，前端通过 get_build_version 命令展示。
    // 未注入时回落到 Cargo 版本，保证普通构建/IDE 分析也能编译。
    println!("cargo:rerun-if-env-changed=VSNOTE_BUILD_VERSION");
    let version = std::env::var("VSNOTE_BUILD_VERSION")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION 未设置")
        });
    println!("cargo:rustc-env=VSNOTE_BUILD_VERSION={version}");

    tauri_build::build()
}
