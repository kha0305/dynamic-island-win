# KẾ HOẠCH KIẾN TRÚC: HỆ THỐNG THÔNG BÁO DYNAMIC ISLAND (`ISLAND-NOTIFICATIONS`)

- **Đặc tả nguồn:** `specs/island-notifications/spec.md`
- **Trạng thái:** Đã hoàn thành triển khai & kiểm chứng Phase 7 (`cargo clippy`, `36/36` unit/integration tests, `--release`).

---

## 1. Tổng quan luồng trạng thái

```
[Tray: Kiểm tra cập nhật] / [Startup Timer (10s)]
        |
        +--> (Thủ công) mở ngay IslandMode::Notice (NoticeKind::Checking)
        |
        v (WM_APP_UPDATE_EVENT từ updater.rs)
   +----+-------------------------+--------------------------+
   |                              |                          |
   v CheckUpToDate                v CheckAvailable           v CheckError / DownloadError
[NoticeKind::UpToDate]     [NoticeKind::UpdateAvailable]  [NoticeKind::Error]
- Xanh ngọc + tích V        - Xanh dương + mũi tên tải     - Viền đỏ + dấu !
- Nút: "Đã hiểu"            - Nút: "Cập nhật ngay"         - Nút: "Thử lại" / "Đóng"
- Tự đóng sau 4.0s            và "Để sau" (giữ chờ chọn)   - Mô tả đa dòng đủ nghĩa
  (trừ khi đang hover)            |
                                  v (Bấm "Cập nhật ngay")
                           [NoticeKind::Downloading]
                           - "Đang tải và xác thực SHA-256..."
                           - Thành công -> spawn_updated_process() & thoát sạch
```

---

## 2. Chi tiết thay đổi theo từng tệp

### 2.1. `src/state.rs`
- Mở rộng `IslandMode`: thêm `IslandMode::Notice`:
  - `target_top_for_mode(IslandMode::Notice) -> TOP_MARGIN_DIP` (`6.0`).
  - `target_dimensions_for_mode(IslandMode::Notice) -> (416.0, 136.0, 26.0, 1.0)` (nằm gọn trong `MAX_SURFACE_WIDTH_DIP = 464.0`, `MAX_SURFACE_HEIGHT_DIP = 336.0`).
  - Cập nhật `effective_mode`: giữ nguyên `IslandMode::Notice` khi có HUD khác (không bị HUD âm lượng/pin đè mất hộp thoại đang đọc), vẫn ẩn nếu `auto_hide_fullscreen && fullscreen_active` (và tôn trọng `visible`, trừ khi `Notice` chủ động bật `visible = true` lúc kiểm tra thủ công).
- Mở rộng `UiAction`:
  - `NoticeSecondary` (nút phụ bên trái: `"Để sau"` hoặc `"Thử lại"`).
  - `NoticePrimary` (nút chính bên phải: `"Đã hiểu"`, `"Thu gọn"`, `"Cập nhật ngay"`, `"Đóng"`).
- Định nghĩa dữ liệu thông báo dùng chung giữa `state`, `window`, `renderer`:
  - `pub enum NoticeKind { Checking, UpToDate, UpdateAvailable, Downloading, Error }`
  - `pub struct IslandNoticeView<'a> { pub kind: NoticeKind, pub title: &'a str, pub message: &'a str, pub primary_label: &'a str, pub secondary_label: Option<&'a str> }`
- Hitbox `pub fn fill_notice_hitboxes(pill: PillRectDip, has_secondary: bool, out: &mut Vec<UiHitbox>)`:
  - Nếu có nút phụ: đặt 2 nút cạnh nhau hàng dưới (`top: pill.top + pill.height - 46.0 .. pill.top + pill.height - 14.0`), nút chính bên phải (`136 DIP`), nút phụ bên cạnh trái (`104 DIP`, cách nhau `8 DIP`).
  - Nếu chỉ có 1 nút chính: đặt gọn bên phải (`width = 112 DIP`, cùng hàng đáy).
- Unit tests trong `src/state.rs`: xác minh hitbox của cả 1 nút và 2 nút trên `IslandMode::Notice` nằm trọn trong pill và không chồng lấn.

### 2.2. `src/renderer.rs`
- Mở rộng `TextFormats` & `TextFormatSlot`:
  - Thêm `BodyLeft` (`11.5pt`, `DWRITE_FONT_WEIGHT_NORMAL`, `DWRITE_TEXT_ALIGNMENT_LEADING`, `DWRITE_PARAGRAPH_ALIGNMENT_NEAR`, `DWRITE_WORD_WRAPPING_WRAP` + dấu `...` nếu quá 3 dòng) để nội dung lỗi/cập nhật tự xuống dòng gọn đẹp không bao giờ bị cắt cụt 1 dòng.
- Mở rộng `RenderFrameInput<'a>`: thêm `pub notice: Option<IslandNoticeView<'a>>`.
- Cập nhật `mode_content_reveal_alpha`, `draw_scene` (`show_media_aura`), `draw_clipped_contents`: thêm nhánh `IslandMode::Notice`.
- Viết hàm `draw_notice`:
  - Dùng `virtual_pill_anchored(input.pill, 416.0, 136.0, false)`.
  - Nếu `NoticeKind::Error`, phủ thêm viền nhấn đỏ trầm (`danger_border` alpha nhẹ) ở dải thẻ trong; vẽ huy hiệu tròn `38 x 38 DIP` bên trái với màu tương ứng (`power_ok` / `accent` / `danger_border`) cùng glyph vector Direct2D (dấu tích `V` cho `UpToDate`, mũi tên đi xuống cho `UpdateAvailable`/`Downloading`, chữ `i`/kính lúp cho `Checking`, dấu `!` cho `Error`).
  - Vẽ tiêu đề (`Title`) ở `p.top + 14..36` và mô tả ngắt dòng (`BodyLeft`) ở `p.top + 38..84`.
  - Vẽ nút từ `fill_notice_hitboxes`, nhấn nổi `NoticePrimary` (`accent_soft` + viền `accent`, hoặc `danger_bg` + viền `danger_border` tùy ngữ cảnh) và `NoticeSecondary` (`surface_card` + viền `card_border`).
- Warm-up các tiêu đề thông báo trong `warm_up_text_formats` và cập nhật test `winrt_glass_backdrop_initializes_and_renders` cho `IslandMode::Notice`.

### 2.3. `src/window.rs`
- Xóa sạch toàn bộ import và hàm dùng `MessageBoxW` (`MessageBoxW`, `IDYES`, `MB_ICONERROR`, `MB_ICONINFORMATION`, `MB_TOPMOST`, `MB_YESNO`, `DeferredUpdateModal`, `show_deferred_update_modal`).
- Thêm hằng `TIMER_NOTICE_EXPIRE_ID: usize = 6` (`4_000 ms`) và cấu trúc nội bộ `ActiveNotice` trên `WindowController`:
  - Lưu `kind`, `title`, `message`, `primary_label`, `secondary_label`, `pending_release: Option<UpdateReleaseInfo>`, `return_mode: IslandMode`, `auto_expire: bool`.
- Luồng xử lý:
  - `open_notice(...)`: lưu `return_mode` (nếu hiện chưa ở `Notice`), đặt `self.requested_mode = IslandMode::Notice`, bật `self.config.visible = true` nếu kiểm tra thủ công từ Tray, hủy `auto_collapse` / `compact_to_notch` / `hud_expire`, đặt `TIMER_NOTICE_EXPIRE_ID` nếu `auto_expire == true` và `!self.mouse_tracked`, rồi `apply_target_and_render()`.
  - `dismiss_notice()`: hủy `TIMER_NOTICE_EXPIRE_ID`, xóa `active_notice`, trả `requested_mode` về `return_mode` hợp lệ (`Expanded` / `Compact` / `Slim`) và `apply_target_and_render()`.
  - `WM_MOUSELEAVE` / `WM_MOUSEMOVE`: dừng timer tự đóng khi chuột đang rê lên `Notice`, chạy lại timer `4.0s` cho `UpToDate` khi chuột rời đi (`WM_MOUSELEAVE`).
  - Bấm `IDM_CHECK_UPDATE` từ Tray: mở `NoticeKind::Checking` (hoặc `UpdateAvailable` nếu đã phát hiện sẵn), **không** bật `MessageBoxW`.
  - Startup check (`CheckAvailable` khi `silent = true`): nếu không fullscreen và không đang mở `Settings`/`AudioDevices`/`Notice`, tự mở `NoticeKind::UpdateAvailable` ngay trên Island; ngược lại vẫn giữ `available_update` để hiện qua Tray.
  - `handle_left_button_up`: không tự đóng `Notice` khi bấm trượt vào khoảng trống giữa các thẻ/chữ (chỉ đóng qua nút hoặc hết hạn `auto_expire`).
- Bổ sung bước kiểm thử tích hợp mới vào `window.rs` (xác minh `CheckUpToDate`, `CheckAvailable -> Install -> Downloading`, `CheckError -> Retry`, tự đóng và giữ khi hover).

---

## 3. Kế hoạch kiểm chứng (Phase 7)
1. Xác nhận `Select-String -Path 'src\*.rs' -Pattern 'MessageBoxW'` trả về **0 kết quả**.
2. `cargo clippy --all-targets -- -D warnings` (0 cảnh báo).
3. `cargo test --locked -- --nocapture` (đạt 100% unit tests + Win32/Direct2D/Notice integration tests).
4. `cargo build --release --locked` và đo kiểm runtime bản release (`VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `CPU_2S_MS=0`, `NV_MODULES=0`, `LEFTOVER_PROCS=0`).
5. Cập nhật `specs/island-notifications/`, `NHAT_KY_HOAT_DONG.md` và `TASK_MEMORY.md`.
