# DANH SÁCH ĐẦU VIỆC: `setup-startup-icon` (`v0.3.4`)

- [x] T1.1: Viết `spec.md`, `plan.md`, `tasks.md`
- [x] T2.1: Sinh bộ icon đa kích thước `assets/app.ico` (16, 20, 24, 32, 48, 256) sắc nét chuẩn Liquid Glass
- [x] T2.2: Viết `build.rs` tạo `.res` thuần Rust (`RT_GROUP_ICON`, `RT_VERSION`, `RT_MANIFEST`) và cấu hình `Cargo.toml`
- [x] T3.1: Cập nhật `src/tray.rs` & `src/window/mod.rs` gắn icon chính thức, lắng nghe `TaskbarCreated` và thử lại tạo TrayIcon
- [x] T3.2: Cập nhật `src/main.rs` (retry khởi tạo + `startup.log`) và `src/config.rs` (kiểm tra path + `StartupApproved\Run` + unit test)
- [x] T4.1: Tạo `src/install.rs` (dùng chung đường dẫn chuẩn, COM Shortcut, đóng tiến trình an toàn, Registry `Uninstall` và `--uninstall`)
- [x] T4.2: Tạo `src/bin/setup.rs` (giao diện Setup tiếng Việt chuẩn Windows 11 + `--silent`) và nối `--uninstall` vào `src/main.rs`
- [x] T5.1: Cập nhật `.github/workflows/release.yml`, chạy `clippy`, `test`, `build --release`, thử nghiệm Setup + Uninstall + Startup thực tế, đồng bộ `dist/` và cập nhật tài liệu
