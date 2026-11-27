# KẾ HOẠCH KIẾN TRÚC: NÂNG CẤP GIAO DIỆN THANH THU GỌN (`COMPACT-PILL-POLISH`)

- **Đặc tả nguồn:** `specs/compact-pill-polish/spec.md`
- **Trạng thái:** Đã hoàn thành triển khai & kiểm chứng Phase 7 (`cargo clippy`, `36/36` unit/integration tests, `--release`).

---

## 1. Thiết kế chi tiết thanh `Compact` (`316 x 40 DIP`, `r = 20.0`)

```
+-------------------------------------------------------------------------+
| ( [Art] )  「Vietsub | Pinyin」Tinh Vệ – Nhất Khỏa...         | | | | | |
| ( 26x26 )  Hoa Chúc, Vương Thắng Kiều                          | | | | | |
|   r=13     [===================------------------]  (2.0 DIP)           |
+-------------------------------------------------------------------------+
  ^ 7 DIP     ^ left+42 .. right-40 (234 DIP)                     ^ 5 vạch
                                                             (hoặc || pause)
```

---

## 2. Chi tiết thay đổi theo từng tệp

### 2.1. `src/state.rs`
- Cập nhật `target_dimensions_for_mode(IslandMode::Compact) -> (316.0, 40.0, 20.0, 1.0)` (giữ `target_top_for_mode` = `2.0 DIP` để `compact_rgn.top == 0` như hiện tại).
- `fill_volume_hud_hitboxes` giữ nguyên công thức tương đối theo `pill.width` và `pill.height` (`316 x 40`), kiểm tra test `volume_hud_hitboxes_stay_inside_compact_pill_and_do_not_overlap` vẫn nằm trọn bên trong.

### 2.2. `src/renderer.rs`
- Mở rộng `TextFormats` / `TextFormatSlot`:
  - Tinh chỉnh `Compact` lên `12.0pt SemiBold` (tiêu đề bài hát nổi bật) và bổ sung `CompactSub` (`10.0pt Medium`, căn trái) cho dòng nghệ sĩ/nguồn phát.
  - Bổ sung vào `warm_up_text_formats`.
- Cập nhật `draw_artwork_or_placeholder`: thêm tham số hoặc dùng `glass_inner_rim`/`card_border` viền sáng kính mảnh (`0.85 DIP`) thay cho viền đen cứng `self.brushes.border` khi vẽ ảnh bìa tròn đồng tâm (`draw_artwork_with_rim`).
- Thiết kế lại `draw_compact`:
  - Neo kích thước `(316.0, 40.0)` qua `virtual_pill_anchored`.
  - Vẽ ảnh bìa `26 x 26 DIP` bo tròn đồng tâm (`radius = 13.0`, `x = p.left + 7.0`, `y = p.top + 7.0`) với viền sáng mảnh.
  - Vẽ 2 tầng chữ (`Title` ở `p.top + 5.5 .. 21.0`, `Artist/SourceApp` với màu dịu alpha `0.78` ở `p.top + 20.5 .. 32.5`) trong dải `p.left + 42.0 .. p.left + p.width - 40.0` (`234 DIP`).
  - Vẽ thanh tiến độ siêu mảnh `2.0 DIP` (`p.top + 34.8 .. 36.8`, bo tròn `1.0 DIP`) từ dưới chữ theo `progress_ratio` của GSMTC.
  - Cập nhật `draw_visualizer_bars`: vẽ **sóng 5 cột mượt hình chuông** (`step_x = 4.0, bar_w = 2.2`, cân sát đầu phải `p.left + p.width - 31.0`), và khi đang có session mà Tạm dừng (hoặc tắt visualizer) thì vẽ icon **Tạm dừng 2 vạch bo tròn dịu (`||`)** tại đầu phải để viên thuốc luôn cân đối hai đầu.
- Cập nhật `draw_compact_hud`: neo `316.0 x 40.0`, căn lại chữ/icon/thanh âm lượng gọn gàng trên chiều cao `40.0 DIP`.
- Tinh chỉnh nhẹ `mode_content_reveal_alpha` (`Compact` dùng `w_fade = ((pill.width - 176.0) / 70.0).clamp(0.0, 1.0)`) để hiện mượt từ `Slim (152)` lên `Compact (316)`, và giữ nguyên hệ thống màu `HostBackdropBrush` + `aura` hòa nền hiện tại.

### 2.3. `src/window.rs`
- Trong `sync_timeline_timer` và `WM_TIMER -> TIMER_TIMELINE_TICK_ID`: bổ sung `IslandMode::Compact` khi đang phát (`!visualizer_enabled` và `active_hud.is_none()`) để thanh tiến độ mảnh chạy đều mỗi giây mà vẫn giữ `0% CPU` lúc rảnh/tạm dừng.

---

## 3. Kế hoạch kiểm chứng (Phase 7)
1. `cargo clippy --all-targets -- -D warnings` (0 cảnh báo).
2. `cargo test --locked -- --nocapture` (toàn bộ 36/36 tests đạt).
3. `cargo build --release --locked` và đo kiểm runtime (`EXE_BYTES`, `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `CPU_2S_MS=0`, `NV_MODULES=0`, `LEFTOVER_PROCS=0`).
4. Cập nhật tài liệu `specs/compact-pill-polish/`, `NHAT_KY_HOAT_DONG.md` và `TASK_MEMORY.md`.
