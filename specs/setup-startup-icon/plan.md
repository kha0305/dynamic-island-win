# KẾ HOẠCH KIẾN TRÚC: BỘ ICON, STARTUP TIN CẬY & SETUP (`v0.3.4`)

## 1. Kiến trúc tổng thể
- **Tài nguyên PE thuần Rust (`build.rs` + `assets/app.ico`):**
  - Không dùng thêm crate ngoài; `build.rs` đọc `assets/app.ico`, tạo tệp `.res` chuẩn Win32 gồm `RT_GROUP_ICON`, các `RT_ICON`, `RT_VERSION` (`0.3.4`) và `RT_MANIFEST` (`asInvoker`, `Common-Controls 6.0`, `PerMonitorV2`), rồi liên kết qua `cargo:rustc-link-arg-bins`.
  - Tách cơ chế nhúng payload cho trình cài đặt qua biến môi trường `WDI_SETUP_PAYLOAD` + `rustc-check-cfg=cfg(has_setup_payload)` để `cargo clippy --all-targets` và `cargo test --locked` luôn vượt qua ngay cả khi chưa build `.exe` release.
- **Gia cố Startup (`src/main.rs`, `src/config.rs`, `src/tray.rs`, `src/window/mod.rs`):**
  - Vòng lặp thử lại khởi tạo cửa sổ/renderer ở `src/main.rs` + ghi log lỗi tối giản (`startup.log`).
  - Lắng nghe `TaskbarCreated` + `TIMER_TRAY_RETRY_ID` trong `src/window/mod.rs` để bảo đảm icon khay hệ thống không bao giờ bị mất khi Explorer khởi tạo chậm hoặc khởi động lại.
  - Đối chiếu cả `Run` và `StartupApproved\Run` trong `src/config.rs`.
- **Module cài đặt dùng chung (`src/install.rs`) & Trình cài đặt (`src/bin/setup.rs`):**
  - `src/install.rs` chứa logic dùng chung: đường dẫn chuẩn `%LOCALAPPDATA%\Programs\WinDynamicIsland`, tạo/xóa Shortcut qua COM `IShellLinkW` + `IPersistFile`, đóng tiến trình đang chạy qua `FindWindowW` + `IDM_EXIT` + chờ giải phóng mutex, ghi/xóa khóa `Uninstall` và chạy gỡ cài đặt `--uninstall`.
  - `src/bin/setup.rs` tạo cửa sổ cài đặt Win32 hiện đại tiếng Việt chuẩn (`Segoe UI`, Common Controls v6), tùy chọn cài đặt và khởi chạy app mới ở `Medium IL`.

## 2. Danh sách tệp tạo mới / cập nhật
- Tạo mới: `assets/app.ico`, `build.rs`, `src/install.rs`, `src/bin/setup.rs`, `specs/setup-startup-icon/{spec,plan,tasks}.md`.
- Cập nhật: `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/config.rs`, `src/tray.rs`, `src/window/mod.rs`, `.github/workflows/release.yml`, `README.md`, `NHAT_KY_HOAT_DONG.md`, `SO_TAY_KIEM_TRA_BUG.md`, `TASK_MEMORY.md`.
