# DANH SÁCH ĐẦU VIỆC TRIỂN KHAI (`ISLAND-NOTIFICATIONS` - PHASE 5)

- **Đặc tả & Kế hoạch:** [spec.md](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/specs/island-notifications/spec.md), [plan.md](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/specs/island-notifications/plan.md).
- **Trạng thái:** Hoàn tất toàn bộ T1–T4 (Phase 7 đã nghiệm thu).

## Các đầu việc cuốn chiếu
- [x] **T1 (`src/state.rs`):** Thêm `IslandMode::Notice` (`416 x 136 DIP`), `NoticeKind`, `IslandNoticeView`, `UiAction::NoticePrimary` / `NoticeSecondary`, `fill_notice_hitboxes` và unit tests không chồng lấn.
- [x] **T2 (`src/renderer.rs`):** Thêm `TextFormatSlot::BodyLeft` (`DWRITE_WORD_WRAPPING_WRAP`), hàm vẽ `draw_notice` với huy hiệu tròn icon vector Direct2D phân biệt màu theo loại thông báo, 1 hoặc 2 nút bấm bo tròn, `warm_up_text_formats` và test `winrt_glass_backdrop_initializes_and_renders`.
- [x] **T3 (`src/window.rs`):** Gỡ bỏ toàn bộ `MessageBoxW`, `DeferredUpdateModal`, `show_deferred_update_modal`; bổ sung `ActiveNotice`, `TIMER_NOTICE_EXPIRE_ID` (`4000 ms`), tự bung khi phát hiện bản mới lúc khởi động (nếu không fullscreen), giữ khi hover, thử lại / đóng / cập nhật ngay, và test tích hợp bước #12.
- [x] **T4 (Phase 7):** Xác minh 0 còn lời gọi `MessageBoxW` trong `src/*.rs`, chạy `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, `cargo build --release --locked`, đo kiểm bản `.exe` thực tế, cập nhật `NHAT_KY_HOAT_DONG.md` và `TASK_MEMORY.md`.
