# KẾ HOẠCH KIẾN TRÚC: THEO DÕI ĐA ANTI & VIÊN THUỐC PHỤ BÊN TRÁI

- **Slug:** `dual-anti-left-pill`
- **Đặc tả:** [spec.md](spec.md)
- **Trạng thái:** Phase 4 — Chờ người dùng duyệt trước khi viết mã nguồn

---

## 1. Luồng dữ liệu và Thay đổi kiến trúc

```text
agy CLI (--anti-hook) ──> %LOCALAPPDATA%\WinDynamicIsland\anti-progress.json ──┐
                                                                               ├──> AntiWatcher (1 luồng, WaitForMultipleObjects)
Anti IDE (tự ghi)     ──> %USERPROFILE%\.gemini\antigravity\brain\*\transcript.jsonl ──┘      │
                                                                                              v
                                                                               AntiWatcherUpdate (Vec<AntiProgressSnapshot>)
                                                                                              │
                                                                                              v
                                                                          WindowController (chọn Primary giữa + Secondary trái)
                                                                            ├── Viên giữa: Primary (hoặc Media/HUD nếu không có Anti)
                                                                            ├── Viên trái: Secondary (232x40 DIP, kính mờ độc lập)
                                                                            └── Viên phải: Khay tệp (File Shelf)
```

---

## 2. Danh sách tệp sẽ tạo / sửa đổi

| Tệp | Thay đổi dự kiến |
| :--- | :--- |
| [src/anti.rs](../../src/anti.rs) | Thêm `parse_ide_transcript_tail` (đọc tối đa 128 KiB cuối `transcript.jsonl` của `Antigravity IDE` / `Antigravity App`), giữ danh sách nhiều `AntiProgressSnapshot` theo `(source, conversation_id)` với cơ chế tự hết hạn/xếp thứ tự `is_running -> updated_unix_ms`, và mở rộng `AntiWatcher` theo dõi thêm `%USERPROFILE%\.gemini\antigravity\brain` (đệ quy `ReadDirectoryChangesW`) cùng lúc với `anti-progress.json` bằng `WaitForMultipleObjects` (`0% CPU` khi chờ). |
| [src/state/mod.rs](../../src/state/mod.rs) | Bổ sung `UiAction::SelectSecondaryAnti` (khi bấm vào viên phụ bên trái để bung `AntiExpanded` cho chính phiên đó). |
| [src/state/hitboxes.rs](../../src/state/hitboxes.rs) | Bổ sung `left_anti_pill_rect(pill, mode)` (`232 x 40 DIP`, cách mép trái `pill` `10 DIP`, giữ đối xứng trên `MAX_SURFACE_WIDTH_DIP = 1040.0`) và `left_anti_contains_dip`. |
| [src/renderer/compositor.rs](../../src/renderer/compositor.rs) | Bổ sung `left_anti_backdrop: Option<WinRtBackdropLayer>` vào `CompositionHost::WinRtGlass`, cập nhật trong `update_backdrops`, `hide_backdrop`, `resize_content`. |
| [src/renderer/mod.rs](../../src/renderer/mod.rs) | Thêm `anti_secondary: Option<&AntiProgressSnapshot>` và `anti_secondary_rect` vào `RenderFrameInput`; gọi `update_backdrops` cho cả 3 lớp kính (giữa, trái, phải) và gọi `draw_left_anti_surface` trong `draw_scene`. |
| [src/renderer/anti_view.rs](../../src/renderer/anti_view.rs) | Tách logic vẽ chung cho thẻ Anti gọn để thêm `draw_left_anti_surface` (vẽ vỏ kính `draw_pill_shell` + spinner 6 chấm + tên nguồn + trạng thái + chi tiết + thanh nhịp đáy). |
| [src/window/mod.rs](../../src/window/mod.rs) | Đổi trường đơn `anti_snapshot` sang quản lý nhiều slot (`anti_slots` + `dismissed_anti_keys` + `anti_expanded_key`), thêm `last_left_anti_region` cho vùng `SetWindowRgn`. |
| [src/window/modes.rs](../../src/window/modes.rs) | Bỏ chặn `self.foreground_is_anti` trong `anti_display_snapshot()`, thêm `anti_primary_snapshot()`, `anti_secondary_snapshot()`, `anti_expanded_snapshot()`, `active_left_anti_rect()`, và kết hợp `left_anti_rgn` trong `update_window_region` + `is_anti_spinner_active`. |
| [src/window/workers.rs](../../src/window/workers.rs) | Cập nhật `handle_anti_update` và `handle_anti_expire_timer` để hết hạn từng phiên `Done`/`Error`/`Canceled` độc lập mà không làm mất phiên Anti còn lại đang chạy. |
| [src/window/input.rs](../../src/window/input.rs) | Bổ sung nhận vùng chuột (`handle_nchittest`, `is_cursor_actually_over_pill`, `handle_left_button_down`, `handle_mouse_move`, `handle_left_button_up`) và xử lý click `UiAction::SelectSecondaryAnti`, `FocusAntiWindow`, `DismissAnti` theo đúng phiên Anti đang chọn. |
| [src/window/tests.rs](../../src/window/tests.rs) | Bổ sung kịch bản kiểm thử tích hợp Win32 khi có 2 Anti chạy đồng thời (`Primary` giữa + `Secondary` trái), kiểm tra `WM_NCHITTEST` xuyên chuột giữa hai viên, kiểm tra không ẩn khi đang mở cửa sổ Anti, và kiểm tra bấm vào viên trái mở `AntiExpanded` đúng phiên phụ. |

---

## 3. Ràng buộc an toàn và hiệu năng
- Không polling định kỳ trên UI thread hay background thread; `AntiWatcher` ngủ hoàn toàn trên `WaitForMultipleObjects` với `INFINITE` (`u32::MAX`) cho tới khi file thay đổi.
- Không đọc toàn bộ `transcript.jsonl` vào RAM (luôn giới hạn `128 KiB` cuối tệp).
- Không giữ `&mut` khi gọi `SetCapture`/`ReleaseCapture`.
- Không sử dụng emoji; giữ `0` cảnh báo `clippy`, `VCRUNTIME_DEP=False`, `NV_MODULES=0`, `Medium IL` (`0x2000`).
