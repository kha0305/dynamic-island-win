# ĐẶC TẢ KỸ THUẬT: THEO DÕI ĐA ANTI (`agy CLI` + `Anti IDE`) & VIÊN THUỐC PHỤ BÊN TRÁI

- **Mã tính năng (Slug):** `dual-anti-left-pill`
- **Ngày lập:** 2026-11-28
- **Trạng thái:** Đã chốt yêu cầu (Phase 3) -> Chờ duyệt kế hoạch (Phase 4)

---

## 1. Bối cảnh và Bài toán
Qua kiểm tra thực tế khi chạy song song `agy CLI` và `Antigravity IDE` trên Windows 11:
1. **`Antigravity IDE` không chạy `~/.gemini/config/hooks.json` (`--anti-hook`):**
   - Chỉ `agy CLI` (`hooks_manager.go`) thực thi `--anti-hook` để ghi vào `%LOCALAPPDATA%\WinDynamicIsland\anti-progress.json`.
   - `Antigravity IDE` (`lsVersion 2.36.0`) ghi tiến độ trực tiếp vào `C:\Users\<User>\.gemini\antigravity\brain\<conversation_id>\.system_generated\logs\transcript.jsonl` mà không gọi hook.
2. **Ẩn nhầm khi đứng trong cửa sổ Anti & đè trạng thái khi mở 2 Anti:**
   - Khi đang mở cửa sổ `Antigravity IDE.exe`, `is_foreground_anti_app` trả về `true` nên Dynamic Island ẩn sạch cả tiến độ của `agy CLI` đang chạy nền.
   - `anti-progress.json` và `WindowController` hiện chỉ lưu 1 snapshot duy nhất nên không thể hiển thị đồng thời 2 Anti.

---

## 2. Yêu cầu đã chốt với người dùng (Phase 3 - Clarify)
1. **Phân bổ vị trí khi mở 2 Anti:**
   - **Viên giữa (Chính):** Hiện Anti đang chạy và cập nhật mới nhất (`primary`). Khi chỉ có 1 Anti hoạt động thì chỉ hiện ở giữa như bình thường.
   - **Viên phụ bên trái (Phụ):** Khi có từ 2 Anti hoạt động đồng thời (khác nguồn hoặc khác phiên đang chạy), Anti còn lại (`secondary`) hiển thị ở một **viên thuốc kính mờ gọn bên trái** viên giữa.
2. **Hiển thị kể cả khi đang đứng trong cửa sổ Anti:**
   - Luôn hiển thị cả Anti chính (giữa) và Anti phụ (bên trái) ngay cả khi người dùng đang focus vào cửa sổ `Antigravity IDE` hoặc cửa sổ `agy CLI` (vẫn tự ẩn khi bật Fullscreen hoặc tắt ứng dụng / bấm `Ẩn nhanh`).
3. **Giao diện và tương tác của viên thuốc phụ bên trái:**
   - Kích thước gọn `232 x 40 DIP`, bo tròn `20 DIP`, nằm cách mép trái viên giữa `10 DIP` (đối xứng với `Khay tệp` bên phải trên vùng vẽ `MAX_SURFACE_WIDTH_DIP = 1040.0`).
   - Sử dụng chung lớp kính mờ `WinRtBackdropLayer` trên cùng `Compositor` + `CompositionDrawingSurface` và hợp vùng `CombineRgn(RGN_OR)` để vùng trống xung quanh vẫn xuyên chuột.
   - Hiển thị: vòng chấm xoay trạng thái (6 chấm khi đang chạy, chấm tĩnh khi xong/lỗi), tên nguồn (`Anti IDE` / `agy CLI` / `Anti App`), nhãn trạng thái ngắn, chi tiết 1 dòng và thanh ánh sáng đáy.
   - Tương tác: Bấm vào viên phụ bên trái để mở bảng chi tiết `AntiExpanded` cho chính phiên Anti phụ đó (với đầy đủ 3 nút `Mở cửa sổ`, `Ẩn nhanh`, `Cài đặt`), hoặc bấm nút đóng/ẩn nhanh riêng từng thẻ.

---

## 3. Yêu cầu kỹ thuật chi tiết
1. **Theo dõi đồng thời Hook + `transcript.jsonl` của `Antigravity IDE` (`src/anti.rs`):**
   - Trong `AntiWatcher::run_anti_watch_loop`, sử dụng `WaitForMultipleObjects` chờ đồng thời trên `stop_event`, `ReadDirectoryChangesW` cho `%LOCALAPPDATA%\WinDynamicIsland` (`anti-progress.json`) và `ReadDirectoryChangesW` (đệ quy `bWatchSubtree = true`) cho `%USERPROFILE%\.gemini\antigravity\brain` (chỉ xử lý sự kiện tạo/sửa file `transcript.jsonl`).
   - Chỉ đọc `128 KiB` cuối tệp `transcript.jsonl` bằng `SeekFrom::Start`, phân tích dòng cuối cùng để suy luận trạng thái:
     - `USER_INPUT` -> `Analyzing` (`Đang tiếp nhận yêu cầu`)
     - `PLANNER_RESPONSE` có `tool_calls` -> dùng `classify_tool_call` (hỗ trợ `ReadingProject`, `Coding`, `Testing`, `Building`, `WaitingApproval`, `FixingError`, `Analyzing`)
     - `PLANNER_RESPONSE` có `content` không còn `tool_calls` (`status == "DONE"`) -> `Done` (`Hoàn tất tác vụ`)
     - `GENERIC` đang `RUNNING` -> giữ/hiển thị tác vụ đang chạy; có chuỗi lỗi rõ ràng -> đánh dấu `had_error`
     - `status == "CANCELED"` -> `Canceled` (`Người dùng hủy`)
   - Khi khởi động không tự bung các `transcript.jsonl` lịch sử cũ; chỉ nhận sự kiện cập nhật mới phát sinh hoặc tệp vừa ghi trong vài giây gần nhất.
   - Tách riêng bộ đệm theo dõi tối đa 2 slot hiển thị (`AntiSlot` theo `AntiSource` hoặc `conversation_id`), lưu bền vững theo `AntiSlotKey` (khác `source` hoặc khác `conversation_id` khi chạy 2 phiên cùng nguồn).
2. **Định tuyến `Primary` / `Secondary` & `AntiExpanded` (`src/window/workers.rs`, `src/window/modes.rs`, `src/window/input.rs`):**
   - Sắp xếp các snapshot hợp lệ (chưa stale, chưa bị người dùng bấm `Ẩn nhanh`) theo tiêu chí: đang chạy (`is_running()`) ưu tiên trước, sau đó theo `updated_unix_ms` mới nhất.
   - Phần tử #0 -> `primary` (hiển thị ở viên giữa khi không bị HUD ngắn như Âm lượng/Pin/Bluetooth che), Phần tử #1 -> `secondary` (hiển thị ở viên phụ bên trái).
   - Bỏ khóa `self.foreground_is_anti` chặn hiển thị Anti để luôn hiện cả khi đang mở cửa sổ Anti.
   - Cho phép `AntiExpanded` hiển thị chi tiết của `Primary` hoặc `Secondary` (qua `anti_expanded_target`), nút `Mở cửa sổ` gọi `focus_anti_window` theo đúng `source` của thẻ đang mở, nút `Ẩn nhanh` chỉ ẩn đúng phiên đang mở (`dismiss_anti_slot`).
3. **Vẽ kính mờ & vùng cửa sổ bên trái (`src/state/hitboxes.rs`, `src/renderer/compositor.rs`, `src/renderer/anti_view.rs`, `src/renderer/mod.rs`):**
   - Bổ sung `left_anti_pill_rect(pill, mode)` tại `src/state/hitboxes.rs`.
   - Bổ sung `left_anti_backdrop: Option<WinRtBackdropLayer>` tại `src/renderer/compositor.rs`.
   - Tái sử dụng helper `draw_anti_spinner` và `draw_anti_activity_bar` trong `src/renderer/anti_view.rs` để vẽ viên thuốc phụ bên trái (`draw_left_anti_surface`).
   - Kết hợp `left_anti_rgn` trong `WindowController::update_window_region` (`CombineRgn(RGN_OR)`), `handle_nchittest`, `is_cursor_actually_over_pill` và `handle_left_button_up`.

---

## 4. Tiêu chí nghiệm thu
1. Mở song song `agy CLI` và `Antigravity IDE`, khi `Antigravity IDE` chạy bước đọc/sửa/lệnh và `agy CLI` chạy bước khác, Dynamic Island hiển thị đồng thời cả 2: cái mới/đang chạy ở viên **giữa**, cái còn lại ở viên phụ **bên trái** (vẫn hiện ngay cả khi đang focus trong cửa sổ `Antigravity IDE` hoặc Terminal).
2. Khi chỉ có 1 Anti hoạt động (chỉ CLI hoặc chỉ Anti IDE), chỉ hiển thị 1 viên ở **giữa**, không hiện viên phụ bên trái.
3. Khi đang mở `Khay tệp` bên phải + 2 Anti (giữa và trái), cả 3 bề mặt kính mờ đều sắc nét, không đè lên nhau, vùng trống giữa các viên thuốc xuyên chuột bình thường.
4. `cargo clippy --locked --all-targets -- -D warnings` sạch, 100% unit/integration tests đạt, kiểm thử chạy thật bản release trên Windows 11 (`Medium IL 0x2000`, `0% CPU` khi đứng yên, `NV_MODULES=0`).
