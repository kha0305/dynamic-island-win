# ĐẶC TẢ TÍNH NĂNG: CHUYỂN NHANH THIẾT BỊ ÂM THANH (`AUDIO-SWITCHER`)

- **Mã tính năng (Slug):** `audio-switcher`
- **Ngày lập:** 2026-11-27
- **Trạng thái:** Hoàn tất triển khai Phase 6 & kiểm thử thực tế Phase 7.

---

## 1. Bối cảnh và mục tiêu
- Mở rộng nền tảng WASAPI COM hiện có trong `src/hud.rs` để người dùng xem danh sách các thiết bị phát âm thanh đang hoạt động (`eRender`, `DEVICE_STATE_ACTIVE`) và đổi thiết bị phát mặc định chỉ bằng 1 chạm ngay trên Dynamic Island hoặc qua menu System Tray.
- Đáp ứng toàn bộ quy chuẩn `GEMINI.md`: thuần Rust dùng `windows = "0.62.2"` (không thêm crate ngoài), mọi thao tác COM chạy trên luồng nền MTA (`AudioMonitor`), `0% CPU` khi đứng yên, không đánh thức GPU rời NVIDIA RTX 3050, không emoji và tiếng Việt có dấu chuẩn xác.

---

## 2. Quyết định đã chốt (Phase 3 — Clarify)
1. **Cách mở & bố trí giao diện:**
   - Thêm nút chọn thiết bị âm thanh ngay ở hàng điều khiển phía dưới của chế độ `Expanded` (`416 x 160 DIP`), hiển thị biểu tượng loa/tai nghe kèm tên thiết bị đang dùng.
   - Bấm vào nút này mở chế độ chọn thiết bị trực tiếp trên Island (`AudioDevices`), đồng thời bổ sung menu con `"Thiết bị phát âm thanh"` trong System Tray chuột phải.
2. **Vai trò Windows Audio khi chuyển thiết bị:**
   - Khi chọn thiết bị mới, gọi `IPolicyConfig::SetDefaultEndpoint` đồng bộ cho cả 3 vai trò (`eConsole`, `eMultimedia`, `eCommunications`) để âm thanh media, game, trình duyệt và ứng dụng thoại (Discord, Meet, Teams) chuyển cùng lúc như thao tác "Set as default" của Windows.
3. **Phản hồi giao diện sau khi chuyển thiết bị:**
   - Nếu người dùng đang mở danh sách `AudioDevices` trên Island: giữ nguyên danh sách mở và cập nhật tại chỗ dấu chọn mặc định + % âm lượng hiện tại của thiết bị mới.
   - Nếu chuyển thiết bị từ System Tray hoặc từ ngoài Windows lúc Island đang ở `Slim` / `Compact` / `Hidden` (do `auto_hide_when_idle`): bung HUD `Compact` trong `2.0 giây` hiển thị tên thiết bị mới và mức âm lượng hiện tại.
4. **Quy tắc dự án đi kèm:** Đã ghi nhận quy tắc đồng bộ `CompositionDrawingSurface` + `CreateRoundRectRgn` vào `GEMINI.md`.

---

## 3. Yêu cầu chức năng & biên an toàn
1. **Liệt kê thiết bị phát động (`IMMDeviceEnumerator`):**
   - Dùng `EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)` lấy danh sách thiết bị đang hoạt động, đọc `IMMDevice::GetId` (giải phóng `PWSTR` bằng `CoTaskMemFree`), đọc `PKEY_Device_FriendlyName` qua `IPropertyStore::GetValue` + `PropVariantToStringAlloc` + `PropVariantClear`.
   - Làm mới danh sách tự động khi `IMMNotificationClient` nhận `OnDeviceAdded`, `OnDeviceRemoved`, `OnDeviceStateChanged`, `OnDefaultDeviceChanged`, `OnPropertyValueChanged`.
2. **Trình bày danh sách thiết bị trên `AudioDevices`:**
   - Khung `424 x 284 DIP` (nằm gọn trong `MAX_SURFACE_HEIGHT_DIP = 336.0`), gồm thanh tiêu đề `"Thiết bị phát âm thanh"` + nút `"Quay lại"` (`BackToMedia` -> `Expanded`) + thông số âm lượng hiện tại của thiết bị mặc định + tối đa 5 dòng thiết bị hiển thị cùng lúc (hỗ trợ cuộn chuột nếu có hơn 5 thiết bị).
   - Mỗi dòng hiển thị tên thân thiện (tự cắt `...` nếu dài), nhãn `"Mặc định"` ở thiết bị đang kích hoạt và highlight hover theo ngôn ngữ `Liquid Glass` kính khói.
3. **Độ tin cậy COM & Trạng thái:**
   - Mọi lỗi `SetDefaultEndpoint` hoặc thiết bị bị rút đột ngột được xử lý an toàn trên luồng nền và đồng bộ lại danh sách mới nhất, không `unwrap()` hay sập ứng dụng.
   - `effective_mode` giữ nguyên `AudioDevices` giống `Expanded`/`Settings` khi có sự kiện HUD khác nhảy vào, và không tự ẩn trong lúc chuột đang nằm trên viên thuốc.

---

## 4. Tiêu chí nghiệm thu
- Danh sách trên Island và System Tray khớp đúng danh sách thiết bị phát active của Windows 11, tự cập nhật khi cắm/rút tai nghe hoặc loa.
- Bấm chọn thiết bị mới chuyển thành công mặc định cả `eConsole`, `eMultimedia`, `eCommunications`; thanh/HUD âm lượng tự gắn sang thiết bị mới và hiện tên thiết bị + % mới.
- Mở `AudioDevices` không đè chéo hitbox, chữ tiếng Việt đủ dấu, kính khói tương phản tốt trên nền trắng/đen, không lệch pha hoạt ảnh `CompositionDrawingSurface`.
- `cargo clippy --all-targets -- -D warnings`, toàn bộ unit tests và kiểm tra runtime (`0% CPU` khi đứng yên, 0 module GPU NVIDIA) đạt sạch.
