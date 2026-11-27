# KẾ HOẠCH KIẾN TRÚC: CHẾ ĐỘ BÁM MÉP KHÔNG VƯỚNG TAB (PHASE 4 - PLAN)

- **Đặc tả nguồn:** [specs/tab-safe-notch/spec.md](spec.md)
- **Mục tiêu:** Giảm vùng chiếm chỗ của Dynamic Island khi đang nghe nhạc lâu xuống dải `152 x 16 DIP` sát `y = 0` sau 4 giây, giữ nguyên hiệu ứng lò xo, Liquid Glass và 0% idle CPU khi tạm dừng.

---

## 1. Thiết kế kỹ thuật
1. **Animation tọa độ `top` trong `IslandAnimator` (`src/state.rs`):**
   - Mở rộng `target_dimensions_for_mode(mode)` trả về `(top, width, height, corner_radius, opacity)` (hoặc bổ sung `target_top_for_mode(mode)` + trường `top: SpringScalar` trong `IslandAnimator`).
   - Giá trị mục tiêu `top` (DIP):
     - `IslandMode::Slim` & `IslandMode::Hidden`: `0.0` (bám sát mép trên `y = 0`).
     - `IslandMode::Compact`, `Expanded`, `Settings`: `6.0` (`TOP_MARGIN_DIP`).
   - Kích thước `IslandMode::Slim`: `(152.0, 16.0, 8.0, 1.0)`.
   - Thêm `pub notch_collapse_after_4s: bool` vào `AppConfig` (mặc định `true`) và `UiAction::ToggleNotchCollapse`.
   - Chia hàng 3 của `fill_settings_hitboxes` thành 2 cột đối xứng: trái = `"Gọn mép 4s"`, phải = `"Màn hình: M1"` (giữ nguyên kích thước tổng của `Settings`, không cần phình SwapChain).

2. **Lưu cấu hình (`src/config.rs`):**
   - Bổ sung đọc/ghi khóa `notch_collapse_after_4s` trong `parse_config` và `serialize_config`, cập nhật unit test roundtrip.

3. **Vẽ dải Notch `152x16` & Ô cài đặt (`src/renderer.rs`):**
   - Cập nhật `draw_slim`:
     - Vẽ `draw_artwork_or_placeholder` cỡ mini `10 x 10 DIP` ở bên trái (bo tròn `5.0`).
     - Vẽ thanh tiến độ phát nhạc mảnh cao `2.0 DIP` ở giữa (khi có timeline), hoặc dải chờ mảnh dịu khi chưa có nhạc.
     - Vẽ 3 cột sóng nhạc thu gọn ở bên phải (`max_h` co theo chiều cao `16 DIP` của `Slim`) khi `has_session && is_playing && visualizer_enabled`.
   - Thêm nhánh render `UiAction::ToggleNotchCollapse` trong `draw_settings`.

4. **Điều phối bộ đếm 4 giây & System Tray (`src/window.rs`, `src/tray.rs`):**
   - `src/tray.rs`: Thêm `pub const IDM_TOGGLE_NOTCH_COLLAPSE: usize = 1009;` và mục tích chọn `"Thu gọn mép trên sau 4 giây"`.
   - `src/window.rs`:
     - Thêm `TIMER_COMPACT_TO_NOTCH_ID = 5` (`COMPACT_TO_NOTCH_DELAY_MS = 4_000`).
     - Trong `handle_media_update`: khi phát hiện bài mới (`!prev_has_session || title đổi || artist đổi` khi `has_session == true`), nếu đang ở `Slim`/`Compact` -> đặt `Compact` để hiển thị tên bài 4 giây và gọi `schedule_compact_to_notch()` (nếu `notch_collapse_after_4s && !mouse_tracked`).
     - Trong `handle_mouse_move`: khi chuột chạm `Slim`, mở lên `Compact` và hủy `TIMER_COMPACT_TO_NOTCH_ID`; khi rời chuột khỏi `Compact` (`handle_mouse_leave`), nếu còn nhạc & bật `notch_collapse_after_4s` -> đặt lại timer 4 giây.
     - Trong `collapsed_rest_mode`: trả về `Slim` khi `media_state.has_session && config.notch_collapse_after_4s` (khi tự thu gọn 5s từ `Expanded`), và cập nhật `update_vsync_activity` cho `Slim` chỉ chạy `VisualizerSlow` lúc `is_playing && visualizer_enabled && media_state.has_session`.

---

## 2. Danh sách tệp sẽ tạo / sửa
| Tệp | Thay đổi chính |
| :--- | :--- |
| `src/state.rs` | Thêm `top: SpringScalar`, chỉnh kích thước `Slim` (`152x16` ở `top=0`), thêm `AppConfig::notch_collapse_after_4s`, `UiAction::ToggleNotchCollapse`, chia 2 cột hàng 3 `Settings` |
| `src/config.rs` | Đọc/ghi `notch_collapse_after_4s` trong `config.ini` + unit test |
| `src/renderer.rs` | Thiết kế lại `draw_slim` (`10px` art + vạch tiến độ mảnh + sóng nhạc mini) và vẽ tile `"Gọn mép 4s"` trong `draw_settings` |
| `src/tray.rs` | Thêm `IDM_TOGGLE_NOTCH_COLLAPSE` vào menu khay hệ thống |
| `src/window.rs` | Quản lý `TIMER_COMPACT_TO_NOTCH_ID` (4s), bung `Compact` 4s khi đổi bài, thu về `Slim` khi rời chuột/hết 4s, xử lý `ToggleNotchCollapse` |

---

## 3. Kế hoạch kiểm chứng (Phase 7)
1. `cargo clippy --all-targets -- -D warnings`, `cargo test --locked` (bổ sung unit test cho `top` animation, cấu hình `notch_collapse_after_4s`, kích thước `Slim` tại `y = 0`, không chồng chéo hitbox `Settings`).
2. `cargo build --release --locked` + chạy thử trực tiếp trên `.exe`: kiểm chứng chuyển đổi `Slim (y=0, h~16)` <-> `Compact (276x36)` khi chạm chuột, sau 4 giây tự rút về Notch, `SETTLED_2S_CPU_MS`, `NV_MODULES=0`, thoát sạch `CLEAN_EXIT=True`.
