# KẾ HOẠCH TRIỂN KHAI: SỬA NHẬN DIỆN ANTI & HUD THIẾT BỊ BLUETOOTH / NGOẠI VI

- **Mã tính năng:** `device-hud-and-anti-fix`
- **Tham chiếu đặc tả:** [spec.md](spec.md)
- **Trạng thái:** CHỜ NGƯỜI DÙNG DUYỆT (Phase 4)

---

## 1. Kiến trúc tổng thể (theo thang Ponytail, ưu tiên Win32 gốc)
Không thêm crate bên ngoài; tái sử dụng crate `windows = "0.62.2"` và hạ tầng `AudioMonitor` / `HudContent` / `Settings` hiện có:

1. **Nhận diện nguồn Anti độc lập đường dẫn ([src/anti.rs](../../src/anti.rs)):**
   - Tách hàm thuần `classify_anti_source_inputs(env, ancestor_exe_names, running_exe_names, transcript_path, artifact_dir)` để unit test đầy đủ mọi kịch bản trên mọi đường dẫn.
   - Thu thập tên tệp `.exe` của chuỗi tiến trình cha bằng `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)` + `OpenProcess`/`QueryFullProcessImageNameW` (chỉ trích `file_name`, không so sánh đường dẫn ổ đĩa/thư mục).
   - Khi cây cha không xác định được và đường dẫn rơi vào `.gemini/antigravity/`, đối chiếu nhanh danh sách tiến trình đang chạy trên máy (`Antigravity IDE.exe` vs `Antigravity.exe`).

2. **Luồng theo dõi thiết bị ngoại vi & Bluetooth ([src/hud/device.rs](../../src/hud/device.rs)):**
   - Khởi tạo từ bên trong worker hoặc song hành cùng `AudioMonitor` trong [src/hud/mod.rs](../../src/hud/mod.rs), gửi sự kiện `HudWorkerEvent::DeviceChanged(DeviceHudEvent)` về `HWND` qua `WM_APP_HUD_EVENT`.
   - Đăng ký `CM_Register_Notification` (`CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE` với cờ `CM_NOTIFY_FILTER_FLAGS_ALL_INTERFACE_CLASSES`, và theo dõi thay đổi trạng thái kết nối Bluetooth) trên luồng riêng; dọn sạch `CM_Unregister_Notification` qua `Drop`.
   - Dùng `CM_Locate_DevNodeW` + `CM_Get_DevNode_PropertyW` đọc `DEVPKEY_Device_ContainerId`, `DEVPKEY_Device_FriendlyName`, `DEVPKEY_Device_DeviceDesc`, `DEVPKEY_Device_EnumeratorName`, `DEVPKEY_DeviceContainer_Category`, `DEVPKEY_Device_IsConnected` và `DEVPKEY_Device_BatteryLevel`.
   - Khử trùng theo `ContainerId` (cửa sổ ~900ms), giữ cache trạng thái kết nối hiện tại của từng `ContainerId` để nhận đúng lúc vừa cắm/kết nối hay rút/ngắt kết nối, và thực hiện 1 lần đọc bù `% pin` sau ~800ms cho Bluetooth mới kết nối nếu tại thời điểm `t = 0` Windows chưa kịp cập nhật `DEVPKEY_Device_BatteryLevel`.
   - Kết hợp `IMMNotificationClient` trong [src/hud/mod.rs](../../src/hud/mod.rs) để bắt cắm/rút tai nghe jack 3.5mm (và bỏ qua nếu cùng `ContainerId` đã vừa được thông báo từ PnP).

3. **Trạng thái, Cấu hình, Giao diện & Hitbox:**
   - [Cargo.toml](../../Cargo.toml): Bổ sung 3 feature nhẹ thuộc nhóm `Win32_Devices` sẵn có: `"Win32_Devices_Bluetooth"`, `"Win32_Devices_DeviceAndDriverInstallation"`, `"Win32_Devices_Properties"`.
   - [src/state/mod.rs](../../src/state/mod.rs), [src/state/animator.rs](../../src/state/animator.rs), [src/state/hitboxes.rs](../../src/state/hitboxes.rs):
     - Thêm `DeviceHudCategory` (`Bluetooth`, `Headphones`, `Usb`, `Peripheral`), `HudContent::Device { category, connected, battery_percent }` (tên thiết bị lưu ở `WindowController` / `RenderFrameInput` để giữ `HudContent` `Copy`), và `UiAction::ToggleHudDevice`.
     - Thêm `hud_device_enabled: bool` vào `AppConfig` (mặc định `true`).
     - Nâng `Settings` lên `424.0 x 436.0 DIP` (`MAX_SURFACE_HEIGHT_DIP = 460.0`), đặt `HUD Thiết bị` cạnh `HUD Pin / Sạc` (Hàng 4) và `Thiết bị phát` xuống Hàng 5 full-width cho cân đối.
   - [src/config.rs](../../src/config.rs) & [src/tray.rs](../../src/tray.rs): Lưu/đọc `hud_device_enabled` trong `config.ini` và thêm `IDM_TOGGLE_HUD_DEVICE` vào menu con `Tùy chỉnh`.
   - [src/renderer/hud_views.rs](../../src/renderer/hud_views.rs), [src/renderer/settings_views.rs](../../src/renderer/settings_views.rs), [src/renderer/mod.rs](../../src/renderer/mod.rs):
     - Vẽ icon vector Direct2D cho Bluetooth, Tai nghe, USB, Thiết bị ngoại vi và huy hiệu `% pin` nhỏ gọn bên phải cho thiết bị Bluetooth có báo pin.
     - Vẽ ô công tắc `"HUD Thiết bị"` trong `Settings`.
   - [src/window/workers.rs](../../src/window/workers.rs), [src/window/input.rs](../../src/window/input.rs), [src/window/mod.rs](../../src/window/mod.rs):
     - Nhận `HudWorkerEvent::DeviceChanged`, hiển thị `HudContent::Device` trong `2.6 giây` (tự động cập nhật tại chỗ nếu `% pin` về sau ~800ms), xử lý `UiAction::ToggleHudDevice` và `IDM_TOGGLE_HUD_DEVICE`.

4. **Icon thiết bị âm thanh (`Loa` / `Tai nghe` / `Bluetooth`):**
   - Bổ sung `AudioOutputKind` (`Speaker`, `Headphones`, `Bluetooth`) vào `AudioOutputDevice` và trạng thái thiết bị hiện hành trong [src/hud/policy_config.rs](../../src/hud/policy_config.rs), [src/hud/mod.rs](../../src/hud/mod.rs), [src/state/mod.rs](../../src/state/mod.rs).
   - Đọc `PKEY_AudioEndpoint_FormFactor` (`Headphones` / `Headset` / `Speakers`) và `DEVPKEY_Device_EnumeratorName` (`BTH*`) trực tiếp từ `IPropertyStore` của `IMMDevice`.
   - Nới `IslandMode::Compact` lên `(338.0, 40.0, 20.0, 1.0)` trong [src/state/animator.rs](../../src/state/animator.rs), bổ sung hitbox `UiAction::OpenAudioDevices` ở vùng icon bên trái sóng nhạc của `Compact` trong [src/state/hitboxes.rs](../../src/state/hitboxes.rs), và giữ logic click vùng còn lại của `Compact` vẫn mở `Expanded` trong [src/window/input.rs](../../src/window/input.rs).
   - Tạo helper vẽ chung `draw_audio_output_icon` trong [src/renderer/hud_views.rs](../../src/renderer/hud_views.rs) để dùng thống nhất tại `Compact`, `Volume HUD`, `Expanded` và từng dòng `AudioDevices`.

5. **Nâng cấp hiển thị & tương tác Anti (`AntiExpanded` + tóm tắt + màu nguồn + nhấn mạnh lỗi/duyệt):**
   - Trong [src/anti.rs](../../src/anti.rs): ưu tiên đọc `toolSummary`, thêm `summarize_command_hint`, bổ sung `recent_steps` (tối đa 3 bước) vào `AntiProgressSnapshot`.
   - Trong [src/watcher.rs](../../src/watcher.rs): bổ sung `focus_anti_window(source)` tìm cửa sổ đang mở khớp `AntiSource` và đưa lên foreground.
   - Trong [src/state/](../../src/state/mod.rs), [src/renderer/anti_view.rs](../../src/renderer/anti_view.rs), [src/window/](../../src/window/modes.rs): thêm `IslandMode::AntiExpanded` (`424 x 154 DIP`), `UiAction::FocusAntiWindow` / `UiAction::DismissAnti`, màu nhấn phân biệt nguồn, thanh ánh sáng đáy, viền nhấn vàng/đỏ và giữ lỗi `10 giây`.

---

## 2. Danh sách tệp dự kiến tạo / chỉnh sửa
1. [Cargo.toml](../../Cargo.toml) & [Cargo.lock](../../Cargo.lock) — Bổ sung feature Win32 PnP/Properties/Bluetooth.
2. [src/anti.rs](../../src/anti.rs) — Sửa logic nhận diện nguồn Anti 3 tầng độc lập đường dẫn + unit test.
3. [src/hud/device.rs](../../src/hud/device.rs) *(tạo mới)* — Theo dõi PnP Bluetooth (`% pin`), USB, ngoại vi, gộp `ContainerId`, khử nhiễu + unit test.
4. [src/hud/policy_config.rs](../../src/hud/policy_config.rs) & [src/hud/mod.rs](../../src/hud/mod.rs) — Phân loại `AudioOutputKind` (`Speaker`/`Headphones`/`Bluetooth`), tích hợp `device.rs` và sự kiện tai nghe `IMMNotificationClient`.
5. [src/state/mod.rs](../../src/state/mod.rs), [src/state/animator.rs](../../src/state/animator.rs), [src/state/hitboxes.rs](../../src/state/hitboxes.rs), [src/state/tests.rs](../../src/state/tests.rs) — Bổ sung `HudContent::Device`, `AudioOutputKind`, `hud_device_enabled`, `UiAction::ToggleHudDevice`, hitbox icon thiết bị trên `Compact` (`338x40`) và chiều cao `Settings` (`424x436`).
6. [src/config.rs](../../src/config.rs) & [src/tray.rs](../../src/tray.rs) — Lưu cấu hình và mục menu Tray `HUD Thiết bị`.
7. [src/renderer/mod.rs](../../src/renderer/mod.rs), [src/renderer/media_views.rs](../../src/renderer/media_views.rs), [src/renderer/expanded_view.rs](../../src/renderer/expanded_view.rs), [src/renderer/hud_views.rs](../../src/renderer/hud_views.rs), [src/renderer/settings_views.rs](../../src/renderer/settings_views.rs), [src/renderer/tests.rs](../../src/renderer/tests.rs) — Vẽ icon thiết bị âm thanh thống nhất, vẽ HUD thiết bị + `% pin`, vẽ tile `HUD Thiết bị` + test Direct2D offscreen.
8. [src/window/mod.rs](../../src/window/mod.rs), [src/window/workers.rs](../../src/window/workers.rs), [src/window/input.rs](../../src/window/input.rs), [src/window/tests.rs](../../src/window/tests.rs) — Điều phối sự kiện HUD thiết bị, bấm icon trên `Compact` mở nhanh `AudioDevices`, và test tích hợp.
9. [SO_TAY_KIEM_TRA_BUG.md](../../SO_TAY_KIEM_TRA_BUG.md) & [NHAT_KY_HOAT_DONG.md](../../NHAT_KY_HOAT_DONG.md) — Cập nhật bài học và kết quả nghiệm thu.

---

## 3. Điểm dừng chờ duyệt
Kế hoạch hoàn tất. **Chờ người dùng xác nhận duyệt kế hoạch** trước khi tiến hành Phase 5 (Tasks) và Phase 6 (Implement).
