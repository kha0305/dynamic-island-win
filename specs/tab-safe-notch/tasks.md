# DANH SÁCH ĐẦU VIỆC: TAB-SAFE NOTCH (PHASE 5 - TASKS)

1. `src/state.rs`: Bổ sung `AppConfig::notch_collapse_after_4s` (mặc định `true`), `UiAction::ToggleNotchCollapse`, `target_top_for_mode` + trường `top: SpringScalar` trong `IslandAnimator`, chỉnh kích thước `IslandMode::Slim` thành `(152.0, 16.0, 8.0, 1.0)` tại `top = 0.0`, chia hàng 3 `Settings` thành 2 cột (`ToggleNotchCollapse` và `CycleMonitor`), và bổ sung unit test.
2. `src/config.rs`: Đọc/ghi khóa `notch_collapse_after_4s` và cập nhật unit test roundtrip.
3. `src/renderer.rs`: Vẽ `draw_slim` (`10x10` art mini bên trái, thanh tiến độ mảnh `2.0 DIP` ở giữa, 3 vạch sóng nhạc thu gọn bên phải), cho phép `draw_visualizer_bars` co chiều cao vừa khung `16 DIP`, và vẽ tile `"Gọn mép 4s"` trong `draw_settings`.
4. `src/tray.rs`: Thêm `IDM_TOGGLE_NOTCH_COLLAPSE` vào menu chuột phải khay hệ thống.
5. `src/window.rs`: Thêm `TIMER_COMPACT_TO_NOTCH_ID` (4 giây), bung `Compact` 4s khi phát hiện phiên mới/đổi bài, thu về `Slim` khi rời chuột hoặc hết timer, cập nhật `update_vsync_activity`, `collapsed_rest_mode` và `ToggleNotchCollapse`.
6. Kiểm chứng `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, `cargo build --release --locked`, chạy thử `.exe` trực tiếp và đối chiếu `SO_TAY_KIEM_TRA_BUG.md`, `NHAT_KY_HOAT_DONG.md`.
