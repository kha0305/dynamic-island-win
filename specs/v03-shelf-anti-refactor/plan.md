# KẾ HOẠCH KIẾN TRÚC & TRIỂN KHAI `v0.3.0` (PHASE 4 - PLAN)

- **Mã tính năng (Slug):** `v03-shelf-anti-refactor`
- **Ngày lập:** 2026-11-28
- **Tham chiếu đặc tả:** [specs/v03-shelf-anti-refactor/spec.md](spec.md)
- **Trạng thái:** Phase 4 — Trình duyệt kế hoạch trước khi viết mã.

---

## 1. Tổng quan kiến trúc (Thuần Rust + Native Windows 11 APIs)
Toàn bộ kế hoạch chia thành **3 chặng cuốn chiếu**, kết thúc mỗi chặng đều chạy `cargo clippy --all-targets -- -D warnings`, toàn bộ `cargo test`, và xác minh thực tế trước khi bước sang chặng tiếp theo:
1. **Chặng 1 (Tái cấu trúc & tối ưu toàn bộ module):** Tách các tệp khổng lồ (`renderer.rs` 5.370 dòng, `window.rs` 3.259 dòng, `state.rs` 1.262 dòng, `hud.rs` 1.068 dòng) và tinh gọn các module còn lại; lọc lại feature `windows` và loại bỏ mã trùng lặp (ví dụ tách bộ parse JSON nhẹ dùng chung cho cả `updater` và `anti_tracker`).
2. **Chặng 2 (Theo dõi Anti):** Xây dựng bridge 1 tệp tiến độ chung (`--anti-hook <EVENT>` + cấu hình gộp an toàn vào `~/.gemini/config/hooks.json`) cho cả `agy CLI`, `Anti IDE`, `Anti app`, luồng theo dõi thụ động bằng `ReadDirectoryChangesW`, kiểm tra tiến trình foreground để chỉ hiện khi chuyển app khác, đủ 10 trạng thái với animation xoay mượt.
3. **Chặng 3 (File Shelf):** Gắn khay thả `File Shelf` cạnh phải Dynamic Island trên cùng hệ thống `Compositor` + `CompositionDrawingSurface`, tách vùng click bằng `CombineRgn(RGN_OR)`, chuyển khởi tạo COM sang `OleInitialize` để hỗ trợ kéo thả OLE 2 chiều (`IDropTarget` nhận thả và `SHCreateDataObject` + `IDropSource` + `DoDragDrop` kéo ra), kèm tickbox chọn từng mục / `"Chọn tất cả"` trước khi hiện `Delete All`.

---

## 2. Chi tiết 3 chặng & danh sách tệp sẽ tạo / sửa / xóa

### Chặng 1 — Tối ưu & tách nhỏ toàn bộ module
- **Cấu trúc tệp mới:**
  - Chuyển `src/renderer.rs` -> thư mục `src/renderer/`:
    - `src/renderer/mod.rs`: `IslandRenderer`, `RenderFrameInput`, `DrawCanvas`, vòng vẽ `render()`, `draw_scene()`, `draw_pill_shell()`, cache `IDWriteTextLayout` (`get_or_create_text_layout`, `measure_text_width`, `draw_text`), neo layout (`virtual_pill_anchored`, `mode_content_reveal_alpha`).
    - `src/renderer/compositor.rs`: `WinRtBackdropLayer`, `CompositionHost`, hiệu ứng `IGraphicsEffectD2D1Interop`, `create_low_power_d3d11_device()` (ép iGPU AMD Radeon).
    - `src/renderer/theme.rs`: `BrushPalette`, `create_brushes()`, `apply_media_theme_colors()`, `build_ambient_aura_bgra()`, `extract_vibrant_accent_rgb()`, upload bitmap WIC.
    - `src/renderer/media_views.rs`: `draw_slim()`, `draw_compact()`, `draw_expanded()`, thanh timeline, visualizer, nút điều khiển nhạc, `draw_gear_icon()`.
    - `src/renderer/hud_views.rs`: `draw_compact_hud()`, `draw_speaker_icon()`, `draw_battery_icon()`, `draw_audio_devices()`.
    - `src/renderer/settings_views.rs`: `draw_settings()`, `draw_toggle_tile()`, `draw_notice()`, `draw_notice_glyph()`.
    - `src/renderer/tests.rs`: Toàn bộ bài test GPU/D2D/Contrast từ `renderer.rs`.
  - Chuyển `src/window.rs` -> thư mục `src/window/`:
    - `src/window/mod.rs`: `IslandWindow`, `WindowController`, `create()`, `wnd_proc()`, `execute_ui_action()`, vòng đời `Drop`.
    - `src/window/modes.rs`: Điều phối chuyển mode (`effective_mode`, `should_collapse_to_slim`, `collapsed_rest_mode`, `apply_target_and_render`, `update_window_region`).
    - `src/window/input.rs`: Chuột, hover, wheel, seek, capture/drag (`handle_left_button_down`, `handle_mouse_move`, `handle_mouse_leave`, `handle_left_button_up`, `handle_mouse_wheel`).
    - `src/window/workers.rs`: Nhận sự kiện từ `MediaWorker`, `AudioMonitor`, `Updater`, Power, Monitor/Display.
    - `src/window/tests.rs`: Kiểm thử tích hợp WASAPI & chuột/kéo.
  - Chuyển `src/state.rs` -> thư mục `src/state/`:
    - `src/state/mod.rs`: Enums & structs miền dữ liệu (`IslandMode`, `UiAction`, `HudContent`, `MediaDisplayState`, `TimelineState`, `AppConfig`, `PillRectDip`, `UiHitbox`).
    - `src/state/animator.rs`: `SpringScalar`, `IslandAnimator`, hàm tính toán khung đích & `effective_mode`.
    - `src/state/hitboxes.rs`: Các hàm `fill_*_hitboxes()` và phép tính vùng click.
    - `src/state/tests.rs`: Toàn bộ kiểm thử vật lý lò xo & hitbox.
  - Chuyển `src/hud.rs` -> thư mục `src/hud/`:
    - `src/hud/mod.rs`: `AudioMonitor`, `WheelVolumeAccumulator`, `AudioCommand`, `HudWorkerEvent`, vòng chạy audio worker.
    - `src/hud/policy_config.rs`: COM `IPolicyConfig`, `set_default_audio_device()`, liệt kê/sắp xếp thiết bị âm thanh.
    - `src/hud/power.rs`: `PowerSnapshot`, `PowerStateTracker`, `read_system_power_status()`.
    - `src/hud/tests.rs`: Toàn bộ test âm lượng, cuộn chuột, pin/sạc, `IPolicyConfig`.
  - **Tối ưu các module còn lại & loại mã/thư viện thừa:**
    - `src/json_mini.rs` (hoặc tách từ `src/updater.rs`): Đưa bộ parser JSON không phụ thuộc crate ngoài & `sha256` ra helper gọn sạch để Chặng 2 tái sử dụng đọc `anti-progress.json` và gộp `hooks.json` mà không cần thêm `serde`/`serde_json`.
    - Rà soát `src/media.rs`, `src/tray.rs`, `src/watcher.rs`, `src/config.rs`, `src/vsync.rs`, `src/main.rs` để gom helper trùng lặp, xóa dead code nếu có, kiểm chứng từng feature trong `Cargo.toml` xem có thể gỡ bỏ `Win32_UI_Controls` hay feature thừa nào khác.
- **Tệp sẽ xóa sau khi tách module:** Các tệp đơn cũ `src/renderer.rs`, `src/window.rs`, `src/state.rs`, `src/hud.rs` (được thay bằng `<ten>/mod.rs` tương ứng).

---

### Chặng 2 — Theo dõi Anti (`agy CLI`, `Anti IDE`, `Anti app`)
- **Kiến trúc chi tiết:**
  - **Tệp tiến độ chung duy nhất:** `%LOCALAPPDATA%\win-dynamic-island\anti-progress.json`, cập nhật qua cơ chế ghi tệp tạm rồi đổi tên atomic (`std::fs::write` -> `std::fs::rename`), chịu lỗi an toàn nếu bị đọc đồng thời.
  - **10 trạng thái chuẩn (`AntiAgentState`):**
    1. `ReadingProject` -> `"Đang đọc dự án"` (`read_file`, `list_resources`, tìm kiếm đọc mã)
    2. `Analyzing` -> `"Đang phân tích"` (`PreInvocation`, `research`, `search_web`)
    3. `Coding` -> `"Đang code"` (`write_file`, `edit_file`)
    4. `Testing` -> `"Đang test"` (`run_command` chứa `test`, `pytest`, `cargo test`, `vitest`, `jest`...)
    5. `Building` -> `"Đang build"` (`run_command` chứa `build`, `check`, `clippy`, `rustc`, `tsc`...)
    6. `WaitingApproval` -> `"Đang chờ duyệt"` (`ask_question`, công cụ cần duyệt / chờ phản hồi người dùng)
    7. `FixingError` -> `"Đang sửa lỗi"` (lượt gọi kế tiếp sau khi vừa nhận lỗi từ tool)
    8. `Error` -> `"Bị lỗi"` (`PostToolUse` có `error`, hoặc `Stop` với `terminationReason == "ERROR"`)
    9. `Canceled` -> `"Đã hủy"` (`Stop` với `terminationReason == "USER_CANCELED"`)
    10. `Done` -> `"Đã xong"` (`Stop` với `terminationReason == "NO_TOOL_CALL"` và `fullyIdle == true`)
  - **Cổng hook cực nhẹ trong `src/main.rs` (`--anti-hook <EVENT>`):** Khi được gọi từ `hooks.json` của cả 3 bề mặt (`agy CLI`, `Anti IDE`, `Anti app`), `win-dynamic-island.exe --anti-hook <SessionStart|PreInvocation|PreToolUse|PostToolUse|PostInvocation|Stop>` đọc JSON từ `stdin`, tự xác định nguồn qua `transcriptPath` (`antigravity-cli` / `antigravity-ide` / `antigravity`), ánh xạ sang đúng 1 trong 10 trạng thái, ghi vào `anti-progress.json`, xuất `{}` ra `stdout` và thoát ngay (< 5ms, không tạo cửa sổ hay chạm mutex).
  - **Cài/gỡ Hook an toàn trong `Settings`:** Bổ sung `UiAction::ToggleAntiMonitor` và `UiAction::InstallAntiHooks` (`AppConfig.anti_monitor_enabled`), lưu trong `config.ini`; chỉ khi người dùng bấm cho phép cài hook, app mới đọc `~/.gemini/config/hooks.json` (nếu có), bảo toàn nguyên vẹn mọi hook khác và chỉ gắn/gỡ nhánh `"win-dynamic-island-status"`.
  - **Theo dõi thụ động & quy tắc chuyển app:**
    - `src/anti.rs`: Luồng nền dùng `ReadDirectoryChangesW` (feature `Win32_Storage_FileSystem`) trên `%LOCALAPPDATA%\win-dynamic-island`, gửi `WM_APP_ANTI_UPDATE` về UI khi tệp đổi. Có cờ TTL dọn trạng thái treo nếu quá thời hạn mà tiến trình Anti đã thoát.
    - Mở rộng `src/watcher.rs` thêm helper `is_foreground_anti_app()` (`QueryFullProcessImageNameW` + kiểm tra tiêu đề cửa sổ terminal chạy `agy` / `Antigravity`): khi đang focus ngay trong cửa sổ Anti thì không bật thẻ Anti; khi chuyển sang app khác mà tác vụ Anti đang hoạt động (`ReadingProject..FixingError`), Island ưu tiên hiển thị tiến độ Anti kèm vòng/chấm xoay (`anti_spinner_phase` gắn vào `TickerMode` chỉ khi đang ở trạng thái động); khi chuyển sang `Done`/`Error`/`Canceled`, giữ hiển thị 4 giây qua `TIMER_ANTI_EXPIRE_ID` rồi tự thu gọn hoặc trả về nhạc.
- **Tệp sẽ tạo / sửa:**
  - Tạo: `src/anti.rs`, `src/renderer/anti_view.rs`.
  - Sửa: `Cargo.toml` (thêm `Win32_Storage_FileSystem`), `src/main.rs`, `src/config.rs`, `src/watcher.rs`, `src/vsync.rs`, `src/state/*`, `src/renderer/{mod,media_views,settings_views}.rs`, `src/window/{mod,modes,workers}.rs`.

---

### Chặng 3 — File Shelf (Khay tệp cạnh Dynamic Island trên màn hình FHD)
- **Kiến trúc chi tiết:**
  - **Bố trí đồng pha trên FHD (`1920x1080`) & click-through:**
    - Tính toán lại `MAX_SURFACE_WIDTH_DIP` (~`740 DIP`, phần dư ngoài khay vẫn trong suốt hoàn toàn) đặt căn giữa mép trên màn FHD (`1920 DIP`, từ `~590` đến `~1330 DIP`).
    - Giữ Dynamic Island ở trục chính giữa; **khay thả mini `File Shelf`** nằm ngay **cạnh phải Dynamic Island** (cách viên thuốc `8 DIP`), hiển thị gọn `"Khay"` hoặc `"Khay (N)"` ở mép trên; khi rê chuột hoặc bấm hoặc đang kéo tệp tới vùng này, bung panel danh sách `File Shelf` ngay bên dưới khay dock (rộng ~`276 DIP`, cao linh hoạt `1..=6` dòng).
    - Nâng cấp `CompositionHost` trong `src/renderer/compositor.rs` quản lý **2 `WinRtBackdropLayer`** (`pill_backdrop` + `shelf_backdrop`) chung 1 `Compositor` và chung 1 `CompositionDrawingSurface`; nâng cấp `update_window_region()` dùng `CombineRgn(..., RGN_OR)` (cùng các hàm vùng chữ nhật bao lúc đang animate) để kết hợp vùng viên thuốc Dynamic + vùng khay `File Shelf` thành vùng nhận chuột chính xác, **giữ nguyên xuyên chuột 100% ở mọi vùng trống giữa và hai bên**.
  - **Kéo thả 2 chiều OLE Drag-Drop (API Win32 thuần, không thêm crate ngoài):**
    - Đổi `ComApartmentGuard` tại `src/main.rs` sang **`OleInitialize(None)` / `OleUninitialize()`** (`Win32_System_Ole`).
    - Tạo `src/shelf.rs` (hoặc `src/window/drag_drop.rs` + `src/shelf.rs`):
      - Lưu trữ danh sách `ShelfItem { path: PathBuf, name: String, size_label: String, selected: bool }`, khử trùng đường dẫn, tự kiểm tra tồn tại tệp trên đĩa khi hiển thị, lưu cấu hình danh sách đường dẫn khay tại `%LOCALAPPDATA%\win-dynamic-island\shelf-items.txt` (không tạo/xóa tệp gốc).
      - **Kéo vào (`#[implement(IDropTarget)]` + `RegisterDragDrop`):** Nhận `CF_HDROP` qua `DragQueryFileW`, bật trạng thái nổi sáng vùng thả khi `DragEnter/DragOver` nằm trên vùng Dynamic hoặc vùng File Shelf, giải phóng `ReleaseStgMedium` an toàn, đẩy danh sách đường dẫn về UI và mở panel `File Shelf`.
      - **Kéo ra (`#[implement(IDropSource)]` + `SHCreateDataObject` + `ILCreateFromPathW` + `DoDragDrop`):** Khi nhấn giữ và kéo một dòng tệp vượt ngưỡng kéo hệ thống (`SM_CXDRAG`/`SM_CYDRAG`), nếu dòng đó đang được tick và có nhiều dòng khác cũng đang được tick thì kéo cả nhóm đã tick (ngược lại kéo đúng dòng đó) bằng `SHCreateDataObject` -> `DoDragDrop(..., DROPEFFECT_COPY | DROPEFFECT_LINK)`. Nhờ dùng `SHCreateDataObject` chuẩn Shell, thả được vào Zalo, Discord, Chrome, Edge và Windows Explorer.
  - **Tickbox & nút `Delete All`:**
    - Panel `File Shelf` vẽ bởi `src/renderer/shelf_view.rs`:
      - Header: `"Khay tệp (N)"` + tickbox `"Chọn tất cả"`.
      - Danh sách dòng tệp: `[tickbox]`, ký hiệu loại tệp gọn, tên tệp + kích thước; click vào ô `[tickbox]` đảo chọn dòng đó, giữ-kéo trên thân dòng để kéo tệp ra app khác.
      - Footer: Khi **chưa tick mục nào** (`selected_count == 0`), ẩn nút `Delete All` và chỉ hiện nút `"Thu gọn"` cùng gợi ý ngắn `"Kéo thả tệp ra app khác"`. Chỉ khi **có ít nhất 1 mục được tick** (`selected_count >= 1`) mới hiện nút **`Delete All`** (xóa sạch hoặc xóa các mục đã chọn? Đảm bảo đúng yêu cầu *"tickbox rồi mới hiện Delete All"*: hiện nút `Delete All` dọn danh sách khỏi khay tạm; tệp gốc trên đĩa được giữ nguyên).
- **Tệp sẽ tạo / sửa:**
  - Tạo: `src/shelf.rs`, `src/renderer/shelf_view.rs`.
  - Sửa: `Cargo.toml` (thêm `Win32_System_Ole`, và `Win32_UI_Shell_Common` nếu cần cho `ITEMIDLIST`), `src/main.rs`, `src/config.rs`, `src/state/*`, `src/renderer/{mod,compositor,shelf_view}.rs`, `src/window/{mod,modes,input,workers}.rs`.

---

## 3. Rủi ro kỹ thuật & biện pháp phòng ngừa
1. **Rủi ro treo chuột/kẹt `SetWindowRgn` khi kết hợp 2 vùng (Dynamic + File Shelf):**
   - *Phòng ngừa:* Khi một trong hai vùng đang chạy animation lò xo (`is_animating()`), áp dụng hộp bao hình chữ nhật tổng (`CombineRgn` 2 `CreateRectRgn`) và chỉ chuyển sang `CombineRgn` của 2 `CreateRoundRectRgn` khi cả 2 lò xo đã dừng hẳn; luôn hủy handle `HRGN` cũ đúng chuẩn sau `SetWindowRgn` thành công hoặc lỗi.
2. **Rủi ro xung đột `CoInitializeEx` với OLE `RegisterDragDrop` / `DoDragDrop`:**
   - *Phòng ngừa:* Dùng `OleInitialize(None)` duy nhất trên UI thread tại `main.rs` ngay trước khi tạo cửa sổ và `DispatcherQueue`; hủy `RevokeDragDrop(hwnd)` trong `IslandWindow::Drop` trước khi `DestroyWindow`.
3. **Rủi ro đánh thức GPU rời NVIDIA hoặc ngốn CPU khi xoay spinner Anti / theo dõi hook:**
   - *Phòng ngừa:* Toàn bộ vẽ vector D2D giữ trên `CompositionDrawingSurface` chạy ở iGPU AMD Radeon; `--anti-hook` chạy xong và thoát trong < 5ms không chạm GPU; luồng watch `anti-progress.json` chặn chờ sự kiện Win32 `ReadDirectoryChangesW` (0% CPU); vòng quay `TickerMode` chỉ chạy khi trạng thái Anti thuộc nhóm đang xử lý (`ReadingProject..FixingError`) và đang hiển thị.
4. **Rủi ro mất dữ liệu cấu hình `hooks.json` có sẵn của người dùng:**
   - *Phòng ngừa:* Khi cài hoặc gỡ hook, đọc nguyên văn JSON hiện tại, chỉ thêm/xóa đúng key `"win-dynamic-island-status"`, tạo bản sao lưu `.bak` trước khi ghi và không bao giờ tự chạy khi chưa có thao tác cho phép từ người dùng.
5. **Rủi ro phá vỡ layout hay tính năng cũ khi tách module (Chặng 1):**
   - *Phòng ngừa:* Không gộp thay đổi logic khi đang tách file ở Chặng 1; tách xong từng nhóm và chạy ngay `cargo clippy --all-targets -- -D warnings` cùng `37/37` tests.

---

## 4. Kế hoạch kiểm chứng từng chặng
- **Sau Chặng 1:**
  - `cargo clippy --all-targets -- -D warnings` = 0 cảnh báo.
  - `cargo test --locked -- --nocapture` đạt đủ 37/37 tests, kiểm tra thêm kích thước mỗi file `< ~600` dòng (gọn rạch ròi theo vai trò).
- **Sau Chặng 2:**
  - Unit tests cho parser 10 trạng thái (`ReadingProject`..`Done`), phân biệt nguồn `agy CLI` / `Anti IDE` / `Anti app`, `--anti-hook` stdin/stdout roundtrip, ghi atomic JSON, TTL dọn trạng thái treo, và ưu tiên Anti khi đổi app foreground.
  - Test tích hợp chạy `--anti-hook PreToolUse` / `Stop` ghi vào tệp tiến độ chung và kiểm tra `win-dynamic-island` hiển thị đủ 10 trạng thái cùng vòng xoay, chụp ảnh kiểm chứng trên FHD và kiểm tra `IDLE_CPU_2S_MS = 0` ở trạng thái kết thúc.
- **Sau Chặng 3:**
  - Unit tests hitbox khay `File Shelf` cạnh phải Dynamic trên `1920x1080` (không chồng lấn, không vượt biên `MAX_SURFACE_WIDTH_DIP`), state tickbox ẩn `Delete All` khi `selected == 0` và hiện `Delete All` khi `selected >= 1`, kiểm tra xóa khỏi khay không động tới tệp mẫu thật trên đĩa.
  - Thử nghiệm kéo tệp thật vào vùng khay thả cạnh Dynamic trên Desktop FHD, tick chọn -> bấm `Delete All`, và kéo thả từ khay ra Explorer/trình duyệt; chạy `cargo clippy`, toàn bộ `cargo test`, `cargo build --release --locked`, kiểm tra `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `NV_MODULES=0`, chụp ảnh kiểm chứng cuối trước khi đóng gói `dist/` và phát hành `v0.3.0`.
