# DANH SÁCH ĐẦU VIỆC TRIỂN KHAI (`COMPACT-PILL-POLISH` - PHASE 5)

- **Đặc tả & Kế hoạch:** [spec.md](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/specs/compact-pill-polish/spec.md), [plan.md](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/specs/compact-pill-polish/plan.md).
- **Trạng thái:** Hoàn tất toàn bộ T1–T4 (Phase 7 đã nghiệm thu).

## Các đầu việc cuốn chiếu
- [x] **T1 (`src/state.rs`):** Cập nhật `target_dimensions_for_mode(IslandMode::Compact) -> (316.0, 40.0, 20.0, 1.0)`, kiểm tra và cập nhật unit tests hitbox/kích thước.
- [x] **T2 (`src/renderer.rs`):** Thêm `TextFormatSlot::CompactSub`, nâng font `Compact`, nâng viền artwork mỏng sáng (`use_glass_rim`), thiết kế lại `draw_compact` (`316 x 40`, artwork tròn đồng tâm `26x26 r=13`, 2 tầng tiêu đề + ca sĩ/nguồn, thanh tiến độ mảnh `2.0 DIP`, sóng nhạc 5 cột cân đầu phải và icon Tạm dừng `||` khi pause), cập nhật `draw_compact_hud` và `mode_content_reveal_alpha`.
- [x] **T3 (`src/window.rs`):** Đồng bộ `sync_timeline_timer` và `WM_TIMER` cho `IslandMode::Compact` khi tắt visualizer và đang phát nhạc (giữ `0% CPU` khi tạm dừng/rảnh).
- [x] **T4 (Phase 7):** Chạy `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, `cargo build --release --locked`, đo kiểm runtime bản release, cập nhật tài liệu `NHAT_KY_HOAT_DONG.md`, `TASK_MEMORY.md`.
