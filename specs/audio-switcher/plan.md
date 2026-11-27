# KẾ HOẠCH KIẾN TRÚC: CHUYỂN NHANH THIẾT BỊ ÂM THANH (`AUDIO-SWITCHER`)

- **Đặc tả nguồn:** `specs/audio-switcher/spec.md`
- **Trạng thái:** Đã hoàn thành triển khai & kiểm chứng Phase 7 (`cargo clippy`, `35/35` unit/integration tests, `--release`).

---

## 1. Tổng quan kiến trúc & Luồng dữ liệu

Tái sử dụng hoàn toàn luồng COM MTA nền (`AudioMonitor`) đang chạy trong `src/hud.rs`, tuyệt đối không thêm crate bên ngoài và không gọi COM/MMDevice trên luồng UI chính.

```
[UI / Tray Click: SelectAudioDevice(device_id)]
        |
        v (AudioCommand::SetDefaultDevice(id))
[Luồng nền COM MTA - src/hud.rs]
  1. Khởi tạo/tái dùng COM IPolicyConfig ({870af99c-171d-4f9e-af0d-e63df40c2bc9})
  2. Gọi SetDefaultEndpoint(wide_id, eConsole), (eMultimedia), (eCommunications)
  3. Gắn lại IAudioEndpointVolumeCallback cho thiết bị mặc định mới
  4. Quét lại EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
  5. Đẩy HudWorkerEvent::DevicesChanged + VolumeSync + post_hud_wake(hwnd)
        |
        v (WM_APP_HUD_EVENT)
[WindowController - src/window.rs]
  - Cập nhật cache danh sách thiết bị & tên thiết bị mặc định hiện tại
  - Nếu đang ở AudioDevices: render cập nhật tại chỗ (dấu Mặc định + %)
  - Nếu đổi từ Tray/ngoài khi ở Slim/Compact/idle: bật Volume HUD 2.0s kèm tên thiết bị
```

---

## 2. Chi tiết thay đổi theo từng tệp

### 2.1. `Cargo.toml`
- Thêm 2 feature Windows chính thức vào bảng `[dependencies.windows]`:
  - `"Win32_Devices_FunctionDiscovery"` (cung cấp hằng số chuẩn `PKEY_Device_FriendlyName`).
  - `"Win32_UI_Shell_PropertiesSystem"` (cung cấp `IPropertyStore::GetValue`).
- Không thêm bất kỳ crate thứ ba nào.

### 2.2. `src/hud.rs`
- Định nghĩa cấu trúc dữ liệu thuần túy truyền sang UI:
  - `pub struct AudioOutputDevice { pub id: String, pub name: String, pub is_default: bool }`
- Mở rộng giao tiếp luồng nền:
  - `HudWorkerEvent`: bổ sung `DevicesChanged(Vec<AudioOutputDevice>)`, và mở rộng `VolumeSync { level: f32, muted: bool, device_name: Option<String>, show_hud: bool }`.
  - `AudioCommand`: bổ sung `SetDefaultDevice(String)`.
- Mở rộng `DeviceNotifyCallback` (`IMMNotificationClient_Impl`):
  - Chuyển `OnDeviceStateChanged`, `OnDeviceAdded`, `OnDeviceRemoved`, `OnPropertyValueChanged` sang gửi tín hiệu `AudioThreadMsg::RefreshDeviceList` vào worker channel (debounce gộp trong vòng lặp `try_recv()` giống `EndpointNotify` để tránh quét lặp khi cắm thiết bị USB nhiều endpoint con).
- Hàm `enumerate_output_devices(enumerator: &IMMDeviceEnumerator) -> Vec<AudioOutputDevice>`:
  - Đọc ID thiết bị mặc định hiện tại (`GetDefaultAudioEndpoint(eRender, eMultimedia)`).
  - Duyệt `EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)`, lấy `IMMDevice::GetId()` (wrapper RAII giải phóng `PWSTR` bằng `CoTaskMemFree`), lấy `PKEY_Device_FriendlyName` qua `OpenPropertyStore(STGM_READ)` -> `PropVariantToStringAlloc` (giải phóng `PWSTR` bằng `CoTaskMemFree` + `PropVariantClear(&mut pv)`), sắp xếp ổn định (ưu tiên thiết bị mặc định hoặc giữ thứ tự ổn định dễ bấm).
- Định nghĩa COM `IPolicyConfig` (IID `f8679f50-850a-41cf-9c72-430f290290c8`, CLSID `870af99c-171d-4f9e-af0d-e63df40c2bc9`) trên Windows 10/11 via `#[windows_core::interface]` và hàm `set_default_audio_device(&str)` gọi `SetDefaultEndpoint` cho 3 vai trò `eConsole`, `eMultimedia`, `eCommunications`.
- Khi xử lý `AudioCommand::SetDefaultDevice(id)`:
  - Thực thi `set_default_audio_device(&id)`, nếu thành công chủ động reattach ngay vào endpoint mới và đánh dấu `force_show_hud = true` (để `VolumeSync` gửi kèm tên thiết bị mới), sau đó phát đồng thời `DevicesChanged` và `VolumeSync`.
- Unit tests trong `src/hud.rs`: kiểm tra sắp xếp/cắt gọn nhãn hiển thị thiết bị an toàn khi rỗng/ký tự dài, và kiểm tra `enumerate_output_devices` / khởi tạo `CoCreateInstance` `IPolicyConfig` an toàn trên luồng COM MTA.

### 2.3. `src/state.rs`
- Bổ sung `IslandMode::AudioDevices`:
  - `target_dimensions_for_mode(IslandMode::AudioDevices) -> (424.0, 284.0, 26.0, 1.0)` (nằm gọn trong `MAX_SURFACE_WIDTH_DIP = 464.0`, `MAX_SURFACE_HEIGHT_DIP = 336.0`).
  - Cập nhật `effective_mode` giữ nguyên `IslandMode::AudioDevices` (giống `Expanded` và `Settings`) khi có HUD tạm thời hoặc đang rảnh.
- Mở rộng `HudContent::Volume`: thêm hiển thị `device_name` (để `draw_compact_hud` hiển thị tên thiết bị ngắn gọn khi đổi thiết bị mới trên `2s` HUD).
- Bổ sung `UiAction`:
  - `OpenAudioDevices` (nút chọn thiết bị ở góc dưới-trái của `Expanded`).
  - `SelectAudioDevice(usize)` (chỉ số hàng hiển thị trong `AudioDevices`, map sang `scroll_offset + visible_idx` trong `WindowController`).
- Hitbox & Layout:
  - `fill_expanded_hitboxes`: thêm nút `OpenAudioDevices` ở vùng trống bên trái hàng nút media (`left: pill.left + 16.0 .. center_x - 76.0`, `top: controls_bottom - 34.0 .. controls_bottom - 2.0`), hoàn toàn không giao cắt với `MediaPrev` (`center_x - 68.0 .. center_x - 34.0`) hay `SeekTimeline`.
  - Bổ sung `pub fn fill_audio_devices_hitboxes(pill: PillRectDip, visible_count: usize, out: &mut Vec<UiHitbox>)`: tính nút `BackToMedia` ở góc trên-phải (`Quay lại`) và tối đa 5 thẻ thiết bị (`row_h = 38.0`, `gap = 6.0`).
- Unit tests trong `src/state.rs`: xác minh mọi hitbox trong `Expanded` và `AudioDevices` nằm trọn trong pill và không đè lên nhau.

### 2.4. `src/renderer.rs`
- Mở rộng `RenderFrameInput<'_>`: thêm `audio_devices: &'a [AudioOutputDevice]`, `audio_device_scroll: usize`, `current_device_name: Option<&'a str>`, và `volume_snapshot` (hiển thị % loa hiện tại ngay trên thanh tiêu đề phụ của `AudioDevices`).
- Cập nhật `mode_content_reveal_alpha`: thêm nhánh `IslandMode::AudioDevices` (`((pill.height - 175.0) / 80.0).clamp(0.0, 1.0)`).
- Cập nhật `draw_expanded`: vẽ thẻ `UiAction::OpenAudioDevices` bằng ngôn ngữ `surface_card`/`surface_hover` kèm biểu tượng loa vector + tên ngắn thiết bị mặc định (tự cắt `...` với `SmallLeft`).
- Thêm `draw_audio_devices`:
  - Dùng `virtual_pill_anchored(input.pill, 424.0, 284.0, false)` neo kích thước cố định lúc co giãn để không nhảy chữ.
  - Vẽ tiêu đề `"Thiết bị phát âm thanh"`, dòng phụ `"Mặc định • XX%"` (hoặc trạng thái tắt tiếng) và nút `"Quay lại"`.
  - Vẽ danh sách thiết bị (tối đa 5 mục mỗi trang, hiện chỉ báo cuộn `"(Cuộn để xem thêm)"` nếu `> 5` thiết bị), tô viền `accent` + nhãn `"Đang dùng"` ở thiết bị mặc định.
- Cập nhật `draw_compact_hud` (nhánh `HudContent::Volume`): khi có `device_name` (trong lúc đổi thiết bị mới), hiển thị tên thiết bị ngắn gọn thay cho nhãn `"Âm lượng"` cố định để người dùng biết rõ đang dùng loa/tai nghe nào cùng % âm lượng.
- Bổ sung chuỗi tiêu đề vào `warm_up_text_formats` để tránh khựng frame đầu khi mở `AudioDevices`.

### 2.5. `src/tray.rs`
- Khai báo `IDM_AUDIO_DEVICE_BASE: usize = 2100`, `IDM_AUDIO_DEVICE_MAX: usize = 2131`.
- Truyền `&[AudioOutputDevice]` vào `show_context_menu`, tạo menu con `"Thiết bị phát âm thanh"` bên cạnh `"Màn hình hiển thị"`, đánh dấu tick (`MF_CHECKED`) đúng thiết bị mặc định hiện tại (hoặc hiện mục xám `"Không có thiết bị phát"` nếu rỗng).

### 2.6. `src/window.rs`
- Lưu trạng thái trên `WindowController`:
  - `audio_devices: Vec<AudioOutputDevice>`, `audio_device_scroll: usize`, `audio_device_name: Option<String>`.
  - `volume_hud_show_device_name: bool` (chỉ hiển thị tên thiết bị trên Compact HUD trong lần thông báo đổi thiết bị, hoặc giữ trong `HudContent::Volume`).
- Xử lý `WM_APP_HUD_EVENT`:
  - Nhận `DevicesChanged(list)`: cập nhật `audio_devices`, kẹp `audio_device_scroll`, cập nhật `audio_device_name` và vẽ lại nếu đang ở `Expanded`/`AudioDevices`.
  - Nhận `VolumeSync`: giữ `AudioDevices` không bị Compact HUD nhảy đè; chỉ kích hoạt Compact HUD `2.0s` khi đổi thiết bị từ ngoài `AudioDevices` (như Tray hoặc Windows Settings).
- Xử lý tương tác trên UI & Tray:
  - `UiAction::OpenAudioDevices` -> chuyển `requested_mode = IslandMode::AudioDevices` (tự động `BackToMedia` -> `Expanded`).
  - `UiAction::SelectAudioDevice(visible_idx)` -> lấy `id` tại `audio_device_scroll + visible_idx`, gửi `AudioCommand::SetDefaultDevice(id)` sang `AudioMonitor`, giữ nguyên chế độ `AudioDevices`.
  - Trong `handle_mouse_wheel`: nếu `effective_mode == IslandMode::AudioDevices` và `audio_devices.len() > 5` (khi không giữ `Ctrl`/`Shift`), cuộn bánh xe trượt danh sách thiết bị thay vì chỉnh loa; các chế độ khác giữ nguyên hành vi cuộn chỉnh âm lượng.
  - Trong `WM_COMMAND`: ánh xạ `IDM_AUDIO_DEVICE_BASE..=IDM_AUDIO_DEVICE_MAX` để đổi thiết bị ngay từ System Tray và bật `2.0s` Compact HUD thông báo tên thiết bị + % âm lượng.
- Kiểm tra `hit_test_action` và `update_window_region`: đảm bảo `IslandMode::AudioDevices` hoạt động chuẩn xác trong hệ thống hover / `BackToMedia` / click vùng trống (chỉ đóng `AudioDevices` khi bấm vùng tiêu đề trên bên trái nếu muốn hoặc chỉ qua nút `Quay lại` để tránh bấm trượt).

---

## 3. Kế hoạch kiểm chứng (Phase 7)
1. `cargo clippy --all-targets -- -D warnings` (0 cảnh báo).
2. `cargo test --locked -- --nocapture` (bao gồm unit tests mới cho hitbox `Expanded`/`AudioDevices`, sắp xếp danh sách thiết bị và tích hợp luồng COM MTA `AudioMonitor`).
3. `cargo build --release --locked` và đo kiểm thực tế trên `target/release/win-dynamic-island.exe`:
   - Kiểm tra liệt kê thiết bị thực tế trên máy hiện tại, thử chuyển/khôi phục thiết bị mặc định (hoặc kiểm chứng không lỗi COM `IPolicyConfig`), kiểm tra `EMOJI_HITS=0`, `VCRUNTIME_DEP=False`, `CPU_2S_MS=0`, `NV_MODULES=0`, `LEFTOVER_PROCS=0`.
4. Đối chiếu `SO_TAY_KIEM_TRA_BUG.md` và cập nhật `NHAT_KY_HOAT_DONG.md`, `TASK_MEMORY.md`.
