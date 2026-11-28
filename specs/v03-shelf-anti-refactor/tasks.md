# DANH SÁCH CÔNG VIỆC TRIỂN KHAI `v0.3.0` (PHASE 5 - TASKS)

- **Mã tính năng (Slug):** `v03-shelf-anti-refactor`
- **Tham chiếu:** [spec.md](spec.md) | [plan.md](plan.md)
- **Nguyên tắc:** Làm cuốn chiếu theo 3 chặng; kiểm thử đạt chặng nào mới chuyển sang chặng tiếp theo.

---

## Chặng 1 — Tối ưu và tách nhỏ toàn bộ module
- [x] **T1.1 — Tách `src/json_mini.rs` & tinh gọn `src/updater.rs`:** Đưa bộ parser JSON không phụ thuộc và `sha256_hex` sang `src/json_mini.rs` (tổng quát hóa thêm helper đọc chuỗi/bool/u64/mảng JSON phục vụ Chặng 2); giảm `src/updater.rs` xuống 525 dòng gọn sạch.
- [x] **T1.2 — Tách `src/hud/`:** Phân rã `src/hud.rs` thành `src/hud/mod.rs`, `src/hud/policy_config.rs`, `src/hud/power.rs`, `src/hud/tests.rs`; xóa `src/hud.rs`.
- [x] **T1.3 — Tách `src/state/`:** Phân rã `src/state.rs` thành `src/state/mod.rs`, `src/state/animator.rs`, `src/state/hitboxes.rs`, `src/state/tests.rs`; xóa `src/state.rs`.
- [x] **T1.4 — Tách `src/renderer/`:** Phân rã `src/renderer.rs` thành `src/renderer/{mod,compositor,theme,media_views,expanded_view,hud_views,settings_views,tests}.rs`; xóa `src/renderer.rs`.
- [x] **T1.5 — Tách `src/window/`:** Phân rã `src/window.rs` thành `src/window/{mod,modes,input,workers,tests}.rs`; xóa `src/window.rs`.
- [x] **T1.6 — Tinh gọn các module còn lại & lọc feature `Cargo.toml`:** Rà soát `media.rs`, `tray.rs`, `watcher.rs`, `config.rs`, `vsync.rs`, `main.rs`; gỡ 8 feature bắc cầu dư thừa trong `Cargo.toml`; nghiệm thu Chặng 1 bằng `cargo clippy --all-targets -- -D warnings` + `37/37` tests.

## Chặng 2 — Theo dõi Anti (`agy CLI`, `Anti IDE`, `Anti app`)
- [x] **T2.1 — Miền dữ liệu & cổng `--anti-hook` (`src/anti.rs`, `src/main.rs`):** Định nghĩa `AntiAgentState` đủ 10 trạng thái, bộ suy luận sự kiện hook -> trạng thái, ghi atomic `%LOCALAPPDATA%\WinDynamicIsland\anti-progress.json`, và xử lý nhanh CLI `--anti-hook <EVENT>` (không tạo UI).
- [x] **T2.2 — Cài/gỡ Global Hook an toàn (`~/.gemini/config/hooks.json`) & `AppConfig`:** Thêm `anti_monitor_enabled` trong `src/config.rs` / `src/state/mod.rs`, hàm merge/remove an toàn khóa `"win-dynamic-island-status"` (tạo backup `.bak`, không đè hook khác).
- [x] **T2.3 — Luồng theo dõi `ReadDirectoryChangesW` & phát hiện Foreground Anti (`src/anti.rs`, `src/watcher.rs`):** Thêm feature `Win32_Storage_FileSystem`; đánh thức UI chỉ khi tệp thay đổi (0% CPU lúc chờ); phát hiện tiến trình foreground `agy`/`Antigravity` để chỉ hiện tiến độ Anti khi chuyển sang app khác.
- [x] **T2.4 — Giao diện 10 trạng thái + animation xoay (`src/renderer/anti_view.rs`, `src/window/*`, `src/vsync.rs`):** Thêm pha xoay `anti_spinner_phase`, ưu tiên Anti trên Island khi chạy ở app khác, tự ẩn sau 4s khi đạt `Đã xong` / `Bị lỗi` / `Đã hủy`, thêm điều khiển trong `Settings`.
- [x] **T2.5 — Kiểm thử Chặng 2:** Unit tests + test mô phỏng sự kiện hook từ 3 nguồn `agy CLI` / `Anti IDE` / `Anti app`, kiểm tra `clippy`, toàn bộ `40/40` `cargo test`, `IDLE_CPU_2S_MS=0`.

## Chặng 3 — File Shelf cạnh Dynamic trên màn hình FHD (`1920x1080`)
- [x] **T3.1 — Bố trí khay cạnh phải Dynamic & hợp vùng `CombineRgn` (`src/state/*`, `src/renderer/compositor.rs`, `src/window/modes.rs`):** Mở rộng bề mặt `MAX_SURFACE_WIDTH_DIP = 1040.0` giữ trung tâm Dynamic + khay dock/panel bên phải cách `10 DIP`, hỗ trợ 2 `WinRtBackdropLayer` đồng pha và `CombineRgn(RGN_OR)` giữ xuyên chuột vùng trống xung quanh/giữa.
- [x] **T3.2 — Miền dữ liệu khay & OLE Drag-Drop 2 chiều (`src/shelf.rs`, `src/main.rs`, `src/window/input.rs`):** Chuyển `main.rs` sang `OleInitialize(None)`, thêm `Win32_System_Ole` + `Win32_System_Memory` + `Win32_UI_Shell`; cài `IDropTarget` nhận `CF_HDROP` và `IDropSource` + `IDataObject` (`DROPFILES` đa thư mục) + `DoDragDrop` kéo ra app khác.
- [x] **T3.3 — Giao diện `File Shelf`, tickbox & logic `Delete All` (`src/renderer/shelf_view.rs`, `src/state/hitboxes.rs`):** Vẽ khay dock mini và panel danh sách có `[tickbox]` từng dòng + `"Chọn tất cả"`; ẩn hoàn toàn nút xóa khi `selected == 0` và chỉ hiện `Delete All` khi `selected >= 1` (dọn khay tạm, giữ nguyên tệp gốc trên đĩa).
- [x] **T3.4 — Kiểm thử Chặng 3 & đóng gói phát hành `v0.3.0`:** Kiểm thử `44/44` unit/GPU/HWND tests, xác minh bản release `841,728 bytes`, chụp ảnh FHD kiểm chứng `Theo dõi Anti` + `Khay tệp`, đối chiếu `SO_TAY_KIEM_TRA_BUG.md`, cập nhật `NHAT_KY_HOAT_DONG.md` & `TASK_MEMORY.md`, đóng gói `dist/` (`v0.3.0`), dọn `target/` và đẩy GitHub.
