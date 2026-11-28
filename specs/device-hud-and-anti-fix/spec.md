# ĐẶC TẢ KỸ THUẬT: SỬA NHẬN DIỆN ANTI & HUD THIẾT BỊ BLUETOOTH / NGOẠI VI

- **Mã tính năng (Slug):** `device-hud-and-anti-fix`
- **Ngày lập:** 2026-11-28
- **Trạng thái:** Đã làm rõ yêu cầu (Phase 3) -> Chờ duyệt kế hoạch (Phase 4)

---

## 1. Bối cảnh và Bài toán
Người dùng cần xử lý 2 hạng mục trong cùng một đợt nâng cấp:
1. **Sửa lỗi nhận diện sai nguồn Anti (`Anti IDE` bị ghi thành `Anti App`):**
   - Hiện tại `src/anti.rs` phân loại `agy CLI`, `Anti IDE`, `Anti App` dựa vào đường dẫn `transcriptPath` / `artifactDirectoryPath`. Khi `Antigravity IDE` di chuyển dữ liệu về thư mục dùng chung `.gemini/antigravity/`, đường dẫn chứa `/antigravity/` bị nhận nhầm thành `Anti App`.
   - Yêu cầu bắt buộc: việc phân loại phải **độc lập hoàn toàn với đường dẫn cài đặt** (không phụ thuộc ổ đĩa hay thư mục `Programs\...`), hoạt động đúng trên mọi máy Windows khác nhau.
2. **HUD Thông báo Kết nối / Ngắt kết nối Bluetooth (kèm `% pin`) và Thiết bị ngoại vi (Tai nghe, USB, Ổ đĩa, HID):**
   - Khi kết nối hoặc ngắt kết nối một thiết bị Bluetooth (tai nghe, loa, chuột, bàn phím, tay cầm...), Dynamic Island bung dạng `Compact` hiển thị tên thiết bị, trạng thái `Đã kết nối` / `Đã ngắt kết nối` và **`% pin`** (nếu thiết bị có báo pin qua Windows).
   - Khi cắm hoặc rút tai nghe (jack 3.5mm / USB Audio), USB, ổ đĩa rời hoặc thiết bị ngoại vi khác, Dynamic Island hiển thị thông báo tên thiết bị gọn gàng (chống nhảy trùng nhiều node con cùng một thiết bị vật lý).

---

3. **Icon thiết bị âm thanh phân biệt Loa / Tai nghe / Bluetooth ở mọi nơi:**
   - Dựa theo `PKEY_AudioEndpoint_FormFactor` và `DEVPKEY_Device_EnumeratorName` của endpoint âm thanh Windows (`IMMDevice`) để phân loại `AudioOutputKind` (`Speaker`, `Headphones`, `Bluetooth`), không đoán mò theo tên thiết bị.
   - Trên thanh `Compact` đang phát nhạc (và trạng thái nghỉ `Compact`): hiển thị icon thiết bị âm thanh nhỏ gọn ngay bên trái sóng nhạc (nới `Compact` lên `338 x 40 DIP` để chữ rộng rãi); bấm trực tiếp vào icon này để mở nhanh danh sách `Thiết bị phát âm thanh`.
   - Đồng bộ icon `Loa` / `Tai nghe` / `Bluetooth` trên `Volume HUD`, nút thiết bị góc dưới-trái của `Expanded` và từng dòng trong danh sách `Thiết bị phát âm thanh`.

## 2. Yêu cầu đã chốt (Phase 3 - Clarify)
1. **Chiều sự kiện HUD Thiết bị:** Hiển thị cả khi **kết nối / cắm vào** và khi **ngắt kết nối / rút ra**.
2. **Chống trùng thiết bị đa thành phần:** Gộp theo thiết bị vật lý (`DEVPKEY_Device_ContainerId`) để 1 lần cắm thiết bị USB / Bluetooth chỉ hiển thị **1 thông báo** với tên thiết bị chính xác (tránh hiện lặp `USB Composite`, `HID`, `Audio`...).
3. **Cài đặt:** Bổ sung **1 công tắc chung** `"HUD Thiết bị"` (`hud_device_enabled`, mặc định `true`) trong bảng `Cài đặt` và menu System Tray (`Tùy chỉnh`).
4. **Vị trí icon thiết bị âm thanh:** Đặt cả trên thanh thu gọn `Compact` (cạnh sóng nhạc, bấm mở nhanh danh sách thiết bị) lẫn đồng bộ phân biệt `Loa` / `Tai nghe` / `Bluetooth` trên `Expanded`, `Volume HUD` và bảng `Thiết bị phát âm thanh`.
5. **Nâng cấp hiển thị Anti trên Dynamic Island:**
   - Thay câu lệnh thô dài bằng tóm tắt ngắn (`toolSummary` hoặc rút gọn lệnh), giữ màu nhấn phân biệt theo nguồn (`agy CLI`, `Anti IDE`, `Anti App`) và thêm thanh ánh sáng chạy nhịp ở đáy viên thuốc.
   - Bấm vào thanh Anti bung thẻ chi tiết (`AntiExpanded`) xem đầy đủ 2 dòng mô tả, tối đa 3 bước gần nhất và các nút **`Mở cửa sổ`**, **`Ẩn nhanh`**, **`Cài đặt`**.
   - Nhấn mạnh viền sáng vàng khi `Đang chờ duyệt`, viền sáng đỏ và giữ lâu (`10 giây`) khi `Bị lỗi`.

---

## 3. Yêu cầu chức năng chi tiết

### A. Nhận diện nguồn Anti độc lập đường dẫn (`src/anti.rs`)
Phân loại theo 3 tầng ưu tiên:
1. **Tầng 1 — Biến môi trường phiên (không phụ thuộc thư mục cài đặt):**
   - `ANTIGRAVITY_LS_VERSION` bắt đầu bằng `cli-`, hoặc `JETSKI_APP_DATA_DIR == "antigravity-cli"`, hoặc tên tệp của `ANTIGRAVITY_AGENTAPI_EXE` là `agy.exe` -> `agy CLI`.
   - `JETSKI_APP_DATA_DIR == "antigravity-ide"` -> `Anti IDE`.
   - Nếu có `VSCODE_PID`, tra tên tệp `.exe` của PID đó để nhận diện `Anti IDE` / `Anti App`.
2. **Tầng 2 — Lần ngược cây tiến trình cha (Win32 ToolHelp, chỉ đối chiếu tên tệp `.exe`):**
   - Đi ngược `th32ParentProcessID` tối đa 10 cấp từ tiến trình `--anti-hook`:
     - Tên tệp `agy.exe` -> `agy CLI`.
     - Tên tệp chứa `antigravity` và `ide` (như `Antigravity IDE.exe`) -> `Anti IDE`.
     - Tên tệp chứa `antigravity` và không chứa `ide` (như `Antigravity.exe`) -> `Anti App`.
3. **Tầng 3 — Fallback khi cây cha bị ngắt hoặc chạy unit test:**
   - Giữ đối chiếu chuỗi `antigravity-cli/` và `antigravity-ide/`.
   - Khi rơi vào thư mục dùng chung `antigravity/`: kiểm tra danh sách tiến trình đang chạy theo **tên tệp `.exe`**: chỉ có `Antigravity IDE.exe` -> `Anti IDE`; chỉ có `Antigravity.exe` -> `Anti App`; không xác định rõ -> `Antigravity`.

### B. HUD Bluetooth (`% pin`) & Thiết bị ngoại vi (`Tai nghe`, `USB`, `Ngoại vi`)
1. **Cơ chế bắt sự kiện nền (`0% CPU` khi chờ):**
   - Dùng Win32 `CM_Register_Notification` (`CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE` và/hoặc `CM_NOTIFY_FILTER_TYPE_DEVICEINSTANCE` trên luồng nền) kết hợp `IMMNotificationClient` sẵn có cho tai nghe jack 3.5mm / âm thanh:
     - **Bluetooth (`BTHENUM\DEV_*`, `BTHLE\DEV_*`, thiết bị cùng `ContainerId`):** Theo dõi `DEVPKEY_Device_IsConnected` (`{83DA6326-97A6-4088-9453-A1923F573B29}, 15`) và đọc `% pin` từ `DEVPKEY_Device_BatteryLevel` (`{104EA319-6EE2-4701-BD47-8DDBF425BBE5}, 2`) trên tất cả devnode cùng `ContainerId` (gồm cả `BTHHFENUM`, `BTHLEDevice`, `HID`). Khi vừa kết nối mà chưa có `% pin` ngay, kiểm tra bổ sung một nhịp trễ ~800ms (không poll liên tục).
     - **Tai nghe / Âm thanh (`IMMNotificationClient`):** Phát hiện khi thiết bị phát mới xuất hiện hoặc đổi trạng thái hoạt động/ngắt (jack 3.5mm, USB Audio) và gộp chung `ContainerId` nếu thiết bị vừa sinh sự kiện PnP USB/Bluetooth để không bắn 2 thông báo chồng nhau.
     - **USB / Lưu trữ / Ngoại vi (`USB\*`, `HID\*`, `USBSTOR\*`, `Volume`):** Bỏ qua node hệ thống ảo/nội bộ (`ROOT\*`, `SWD\PRINTENUM\*`, `USB\ROOT_HUB*`, `PCI\*`), đọc tên thân thiện (`DEVPKEY_Device_FriendlyName` / `DEVPKEY_Device_DeviceDesc`), gộp theo `ContainerId` với cửa sổ khử trùng ~900ms.
2. **Hiển thị trên Dynamic Island (`Compact`, tự thu sau `2.6 giây`):**
   - Biểu tượng vector Direct2D phân biệt theo loại thiết bị: **Bluetooth**, **Tai nghe**, **USB / Ổ đĩa**, **Ngoại vi**.
   - Dòng trên: Tên thiết bị (cắt ellipsis gọn nếu dài).
   - Dòng dưới:
     - Bluetooth kết nối có pin: `"Đã kết nối · Pin 85%"` kèm viên pin mini ở góc phải hiển thị đúng mức `%` và đổi màu cảnh báo khi pin thấp (`<= 20%`).
     - Bluetooth kết nối không báo pin / ngoại vi cắm vào: `"Đã kết nối"`.
     - Ngắt kết nối / rút ra: `"Đã ngắt kết nối"` (hoặc `"Đã rút thiết bị"`).
3. **Bật/tắt & Lưu cấu hình:**
   - Thêm `hud_device_enabled: bool` (mặc định `true`) vào `AppConfig`, lưu/đọc trong `config.ini`, đồng bộ menu `"HUD Thiết bị"` trong System Tray và ô gạt `"HUD Thiết bị"` trong bảng `Cài đặt`.

---

## 4. Tiêu chí nghiệm thu
1. Chạy unit test phân loại nguồn Anti chứng minh nhận diện đúng `Anti IDE` ở đường dẫn cài đặt bất kỳ (dù `transcriptPath` nằm ở `.gemini/antigravity/`).
2. `cargo clippy --locked --all-targets -- -D warnings` sạch (0 cảnh báo).
3. Toàn bộ `cargo test --locked` (gồm unit test mới cho `anti.rs`, `device.rs`, `config.rs`, `hitboxes.rs`, `renderer`) đạt 100%.
4. Build thành công bản release `win-dynamic-island.exe` và `WinDynamicIsland-Setup.exe`, kiểm tra runtime thật (chạy ở `Medium IL` `0x2000`, `VCRUNTIME_DEP=False`, `NV_MODULES=0`, `IDLE_CPU=0ms`, không rò rỉ handle/bộ nhớ).
