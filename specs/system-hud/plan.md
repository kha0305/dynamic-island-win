# KẾ HOẠCH TRIỂN KHAI: HUD ÂM LƯỢNG VÀ PIN / NGUỒN SẠC (`SYSTEM-HUD`)

- **Mã tính năng (Slug):** `system-hud`
- **Giai đoạn:** Hoàn tất Phase 5–7 (Đã kiểm chứng unit, Win32 `WM_MOUSEWHEEL` và `.exe` Release).

---

## 1. Kiến trúc tổng thể (Zero-Polling & An Toàn Luồng UI)
1. **Giám sát âm lượng trên luồng nền riêng (`src/hud.rs`):**
   - Dùng `COM MTA` trên luồng nền chuyên biệt đăng ký `IMMNotificationClient` (theo dõi đổi thiết bị âm thanh mặc định) và `IAudioEndpointVolumeCallback` (nhận sự kiện đổi volume/mute tức thời từ Windows Audio Service).
   - Dùng `APP_VOLUME_EVENT_CONTEXT` GUID để phân biệt thao tác từ chính app, tránh tự dội lặp echo khi kéo thanh hoặc cuộn chuột.
   - Giao tiếp hai chiều với luồng UI qua `mpsc` (`AudioCommand` / `HudWorkerEvent`) và `PostMessageW(hwnd, WM_APP_HUD_EVENT, 0, 0)`.
2. **Giám sát Pin / Sạc chuẩn Win32 (`WM_POWERBROADCAST`):**
   - Đăng ký `RegisterPowerSettingNotification(hwnd, &GUID_ACDC_POWER_SOURCE, DEVICE_NOTIFY_WINDOW_HANDLE)` và giải phóng qua `UnregisterPowerSettingNotification` lúc `shutdown_and_destroy` / `Drop`.
   - Đọc `GetSystemPowerStatus`, chỉ phát HUD khi đổi trạng thái cắm/rút sạc hoặc tụt xuống mốc pin yếu (`<= 15%` và `<= 7%`).
3. **Xử lý cuộn con lăn chuột (`WM_MOUSEWHEEL`) an toàn không bug:**
   - **Không dùng global hook `WH_MOUSE_LL`**, xử lý thuần `WM_MOUSEWHEEL` cục bộ trên cửa sổ Island.
   - **Tích lũy chính xác `WHEEL_DELTA` (`120`):** Hỗ trợ chuột nấc lẫn Precision Touchpad mịn, tự xóa phần dư khi đảo chiều hoặc khi rời chuột (`WM_MOUSELEAVE`).
   - **Quy đổi đúng tọa độ màn hình `lparam` -> `ScreenToClient` -> DIP:** Chỉ nhận cuộn trong phạm vi `animator.current_pill()`, bỏ qua khi giữ `Ctrl` / `Shift` / nút chuột, khi ở `Expanded` / `Settings` hoặc khi tắt công tắc `Cuộn chỉnh loa`.

---

## 2. Danh sách tệp tạo mới và chỉnh sửa
1. `Cargo.toml` & `Cargo.lock`: Bổ sung `Win32_Media_Audio`, `Win32_Media_Audio_Endpoints`, `Win32_System_Power`, `Win32_System_Com_StructuredStorage`, `Win32_System_Variant`.
2. `src/hud.rs`: `AudioMonitor`, `PowerStateTracker`, `WheelVolumeAccumulator`.
3. `src/state.rs`: `HudContent`, `AppConfig::{hud_volume_enabled, hud_wheel_enabled, hud_power_enabled}`, `effective_mode(..., hud_active)`, `fill_volume_hud_hitboxes`, lưới `Settings` 5 hàng `424 x 298 DIP`, `MAX_SURFACE_HEIGHT_DIP = 336.0`.
4. `src/config.rs` & `src/tray.rs`: Lưu/đọc 3 khóa mới trong `config.ini` và thêm 3 mục menu System Tray.
5. `src/renderer.rs`: Vẽ vector icon Loa/Pin trong `draw_compact_hud` và 3 tile mới trong `draw_settings`.
6. `src/window.rs` & `src/main.rs`: Xử lý `WM_APP_HUD_EVENT`, `WM_POWERBROADCAST`, `WM_MOUSEWHEEL`, kéo thanh âm lượng (`SetCapture`/`ReleaseCapture` ngoài borrow `&mut`) và `TIMER_HUD_EXPIRE_ID`.
