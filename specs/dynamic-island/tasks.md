# DANH SÁCH CÔNG VIỆC: WIN DYNAMIC ISLAND MVP v0.1 (PHASE 5 - TASKS)

> Đối chiếu kiểm tra chéo (Analyze): Toàn bộ 5 tiêu chí nghiệm thu trong `specs/dynamic-island/spec.md` (mục 5) đều đã được ánh xạ đầy đủ vào các task bên dưới, không có task thừa ngoài phạm vi v0.1.

---

## Bước 1 — Khung dự án, Cửa sổ Overlay & System Tray

- [x] **T1.1:** Khởi tạo `Cargo.toml`, `app.manifest` (Per-Monitor DPI Awareness v2) và `build.rs` nhúng tài nguyên Windows (đã dùng API `SetProcessDpiAwarenessContext` để không phụ thuộc `rc.exe`). *(Phụ thuộc: Không)*
- [x] **T1.2:** Xây dựng module `src/tray.rs` tạo icon System Tray bằng `Shell_NotifyIconW` và menu ngữ cảnh tiếng Việt (Hiện/Ẩn, Tạm giữ Slim/Tự ẩn khi rảnh, Thoát; các mục Chọn màn hình và Tự khởi động nối tiếp ở Bước 4). *(Phụ thuộc: T1.1)*
- [x] **T1.3:** Xây dựng module `src/window.rs` & khởi đầu `src/renderer.rs` dựng cửa sổ `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE` căn giữa cạnh trên màn hình chính theo DPI, khởi tạo DirectComposition + DXGI iGPU (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`) + Direct2D vẽ hình viên thuốc bo tròn ban đầu và áp dụng `SetWindowRgn` xuyên chuột vùng trống. *(Phụ thuộc: T1.1)*
- [x] **T1.4:** Ghép vòng lặp thông điệp chính trong `src/main.rs` + unit test cơ bản + chạy thử `.exe` xác nhận ẩn Taskbar/`Alt+Tab`, xuyên chuột và thoát sạch. *(Phụ thuộc: T1.2, T1.3)*

---

## Bước 2 — Vật lý lò xo mượt VSync & Lớp vẽ Direct2D/DirectWrite

- [x] **T2.1:** Cài đặt bộ giải lò xo tắt dần sub-step cố định (`SpringScalar`) trong `src/state.rs`, bảo toàn vận tốc khi chuyển trạng thái đột ngột giữa `Hidden`, `Slim`, `Compact`, `Expanded`, `Settings` và nhận biết ngưỡng nghỉ tuyệt đối. *(Phụ thuộc: T1.4)*
- [x] **T2.2:** Nâng cấp `src/renderer.rs` vẽ nền kính tối mờ, viền sáng, clip nội dung mượt khi co giãn và hỗ trợ vẽ chữ Unicode tiếng Việt/CJK sắc nét qua `DirectWrite`. *(Phụ thuộc: T2.1)*
- [x] **T2.3:** Tích hợp luồng VSync (`IDXGIOutput::WaitForVBlank`) đánh thức bằng `Condvar` + `AtomicBool` chỉ khi đang chuyển động, tự động ngắt hoàn toàn về 0% khi ổn định; bắt sự kiện hover/rời chuột/click/chuột phải trong `src/window.rs`. *(Phụ thuộc: T2.1, T2.2)*
- [x] **T2.4:** Kiểm chứng mượt chuyển động, đo kiểm thực tế mức CPU lúc đứng yên và xác nhận GPU rời NVIDIA RTX 3050 không bị đánh thức. *(Phụ thuộc: T2.3)*

---

## Bước 3 — Tích hợp nhạc Windows GSMTC & Menu trực quan trên Island

- [x] **T3.1:** Xây dựng `src/media.rs` khởi tạo COM/WinRT trên luồng nền, lắng nghe `GlobalSystemMediaTransportControlsSessionManager` thay đổi phiên và metadata bài hát (xử lý an toàn không `unwrap`/`expect`). *(Phụ thuộc: T2.4)*
- [x] **T3.2:** Giải mã stream ảnh bìa bằng WIC và thu nhỏ an toàn xuống tối đa 128x128 px ngay trên luồng nền, gửi pixel BGRA sang UI thread qua `mpsc` + `PostMessageW`. *(Phụ thuộc: T3.1)*
- [x] **T3.3:** Vẽ giao diện `Compact` (ảnh bìa mini + tên bài hát + sóng nhạc) và `Expanded` (ảnh bìa lớn bo góc, tiêu đề, nghệ sĩ, nút Play/Pause/Previous/Next, nút Cài đặt) trong `src/renderer.rs` + hiệu ứng hover và hit-test nút điều khiển. *(Phụ thuộc: T3.2)*
- [x] **T3.4:** Xây dựng giao diện **Menu/Cài đặt trực quan** (`IslandTab::Settings`) ngay trên Dynamic Island với các công tắc gạt (pill switch) cho Tự ẩn khi rảnh, Tự ẩn khi Fullscreen, Sóng nhạc, Tự khởi động, Chọn màn hình, Thu ẩn và Thoát. *(Phụ thuộc: T3.3)*

---

## Bước 4 — Tự ẩn Fullscreen, Đa màn hình, Tray Menu đầy đủ & Nghiệm thu không bug

- [x] **T4.1:** Xây dựng `src/watcher.rs` theo dõi cửa sổ foreground toàn màn hình (loại trừ Desktop/Shell/tàng hình, kết hợp `SHQueryUserNotificationState`) và thay đổi màn hình, chỉ gửi sự kiện khi trạng thái thay đổi. *(Phụ thuộc: T3.4)*
- [x] **T4.2:** Hoàn thiện `src/tray.rs` với menu phân nhóm đầy đủ, trạng thái tick, submenu danh sách màn hình thật (`EnumDisplayMonitors`), lưu/nạp cấu hình (`src/config.rs`) và bật/tắt Tự khởi động (`HKCU`). *(Phụ thuộc: T4.1)*
- [x] **T4.3:** Viết unit test dày đặc cho lò xo, hit-test menu/nút, cấu hình, đa màn hình/DPI và Fullscreen logic; chạy `cargo clippy`, `cargo test`, `cargo build --release`. *(Phụ thuộc: T4.2)*
- [x] **T4.4:** Stress test thực tế trên bản release `.exe` (hover/click liên tục, đo CPU/RAM khi idle, kiểm tra rò rỉ tài nguyên, kiểm tra iGPU, đối chiếu `SO_TAY_KIEM_TRA_BUG.md`), sửa sạch mọi lỗi phát sinh trước khi cập nhật `NHAT_KY_HOAT_DONG.md` và bàn giao. *(Phụ thuộc: T4.3)*
