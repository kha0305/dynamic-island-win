# DANH SÁCH ĐẦU VIỆC TRIỂN KHAI (`AUDIO-SWITCHER` - PHASE 5)

- **Đặc tả & Kế hoạch:** [spec.md](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/specs/audio-switcher/spec.md), [plan.md](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/specs/audio-switcher/plan.md).
- **Trạng thái:** Hoàn tất toàn bộ T1–T6 (Phase 7 đã nghiệm thu).

## Các đầu việc cuốn chiếu
- [x] **T1 (`Cargo.toml`):** Bổ sung `"Win32_Devices_FunctionDiscovery"` và `"Win32_UI_Shell_PropertiesSystem"` vào crate `windows`.
- [x] **T2 (`src/hud.rs`):** Liệt kê thiết bị phát active (`IMMDeviceEnumerator`, `PKEY_Device_FriendlyName`), lắng nghe thay đổi thiết bị qua `IMMNotificationClient`, triển khai COM `IPolicyConfig::SetDefaultEndpoint` (cả 3 vai trò `eConsole`/`eMultimedia`/`eCommunications`) và unit tests.
- [x] **T3 (`src/state.rs`):** Thêm `IslandMode::AudioDevices`, `UiAction::OpenAudioDevices`, `UiAction::SelectAudioDevice(usize)`, mở rộng `HudContent::Volume`, cập nhật `fill_expanded_hitboxes`, viết `fill_audio_devices_hitboxes` và unit tests không trùng hitbox.
- [x] **T4 (`src/renderer.rs`):** Thêm nút thiết bị góc dưới-trái `Expanded`, màn hình danh sách `draw_audio_devices` (tối đa 5 dòng/trang + chỉ báo cuộn), hiển thị tên thiết bị trong Compact HUD khi vừa đổi thiết bị, và `warm_up_text_formats`.
- [x] **T5 (`src/tray.rs` & `src/window.rs`):** Thêm menu con `"Thiết bị phát âm thanh"` trong Tray, xử lý click/cuộn chuột/thay đổi thiết bị và đồng bộ Compact HUD `2.0s` hoặc cập nhật tại chỗ trong `AudioDevices`.
- [x] **T6 (Phase 7):** Chạy `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, `cargo build --release --locked`, đo kiểm bản `.exe` thực tế, kiểm tra `IPolicyConfig` COM thật, cập nhật tài liệu `NHAT_KY_HOAT_DONG.md`, `TASK_MEMORY.md`.
