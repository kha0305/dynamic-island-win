# DANH SÁCH ĐẦU VIỆC: BẢN VÁ UI v0.2.1 (`UI-POLISH-V021`)

- **Kế hoạch nguồn:** [specs/ui-polish-v021/plan.md](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/specs/ui-polish-v021/plan.md)

## Các bước triển khai (Phase 6)
- [x] **T1 (`src/state.rs`):** Nâng `MAX_SURFACE_HEIGHT_DIP = 396.0`, cập nhật kích thước `Expanded` (`424 x 160`) và `Settings` (`424 x 360`), thêm `audio_devices_height_for_count` + `target_dimensions_for_state`, thêm `UiAction::CheckUpdate`, thiết kế lại `fill_expanded_hitboxes` và `fill_settings_hitboxes`, cập nhật unit tests hitbox/dimension.
- [x] **T2 (`src/renderer.rs`):** Tách `sys_accent` / `sys_accent_soft` khỏi màu thích ứng bài hát; thêm icon bánh răng vector; hoàn thiện `draw_slim` (tối giản khi rảnh + neo layout), `draw_compact_hud` (hover knob khi hiện tên thiết bị), `draw_expanded` (tiêu đề tràn phải, đáy đối xứng), `draw_settings` (3 nhóm + phiên bản + kiểm tra cập nhật), `draw_audio_devices` (chiều cao động, màu trung tính) và cập nhật tests GPU.
- [x] **T3 (`src/window.rs`):** Nối `target_dimensions_for_state` cho `AudioDevices` và cập nhật khi đổi số thiết bị; thêm `settings_return_mode` & `audio_devices_return_mode`; xử lý `UiAction::CheckUpdate`; truyền trạng thái cập nhật vào `RenderFrameInput` và cập nhật integration tests.
- [x] **T4 (Kiểm chứng & Phát hành `v0.2.1`):** Nâng `Cargo.toml` / `Cargo.lock` lên `0.2.1`, chạy `cargo clippy`, `cargo test --locked`, `cargo build --release --locked`, chụp ảnh nghiệm thu thực tế bản mới trên màn hình, cập nhật `NHAT_KY_HOAT_DONG.md` / `TASK_MEMORY.md` / `SO_TAY_KIEM_TRA_BUG.md`, đồng bộ `dist/`, commit và đẩy tag `v0.2.1` lên GitHub.
