# SỔ TAY KIỂM TRA VÀ ĐỐI CHIẾU BUG (BUG CHECKLIST & VERIFICATION HANDBOOK)

> **QUY TẮC BẮT BUỘC:** Trước khi công bố hoàn thành bất kỳ tính năng, giao diện hoặc bản sửa lỗi nào trong dự án `win-dynamic-island`, bắt buộc phải mở tệp này ra đối chiếu toàn bộ bảng kiểm tra bên dưới để ngăn chặn lỗi hồi quy.

---

## PHẦN 1: BẢNG ĐỐI CHIẾU NHANH TRƯỚC KHI BÁO HOÀN THÀNH

### A. Đối Chiếu Cửa Sổ & Trải Nghiệm Windows 11 (Overlay / Win32)
- [x] **Ẩn khỏi Taskbar & Alt+Tab:** Cửa sổ Dynamic Island có gắn cờ `WS_EX_TOOLWINDOW`, `WS_EX_TOPMOST`, `WS_EX_NOREDIRECTIONBITMAP` và `WS_EX_NOACTIVATE` (`EXSTYLE=0x08200088`), không hiện trên Taskbar hay `Alt + Tab`.
- [x] **Không che mất thao tác chuột:** Khi thu gọn (`Slim`), `SetWindowRgn` cắt vùng `COMPLEXREGION` khớp sát viên thuốc (`159x17 px` quanh cạnh trên), phần trống hai bên xuyên chuột hoàn toàn xuống ứng dụng bên dưới.
- [x] **Tự ẩn khi Fullscreen:** Đã kiểm chứng qua kiểm thử tích hợp Win32 `HWND` thật (`win32_hwnd_fullscreen_integration_check`) và tích hợp thêm `SHQueryUserNotificationState` cho game/trình chiếu D3D toàn màn hình.
- [x] **Đa màn hình & DPI Scaling:** Đã kiểm chứng thuật toán căn giữa màn hình chính/phụ tọa độ âm ở DPI 100%, 125%, 150% và liệt kê màn hình thật qua `EnumDisplayMonitors`.
- [x] **Tiết kiệm năng lượng & Không đánh thức dGPU:** Đã đo kiểm thực tế trên bản release: `CPU_3S_MS=0.00` lúc đứng yên, adapter được chọn là `AMD Radeon(TM) Graphics`, không xuất hiện trong `nvidia-smi` của RTX 3050.

### B. Đối Chiếu Dữ Liệu, Đa Luồng & An Toàn Bộ Nhớ (Rust Craft)
- [x] **Không chặn luồng giao diện (Zero UI Freeze):** GSMTC Media và VSync Ticker chạy trên luồng nền riêng, giao tiếp qua `mpsc` + `PostMessageW`.
- [x] **Không panic vì dữ liệu thiếu (`unwrap` / `expect`):** Xử lý an toàn mọi trường hợp thiếu session/thumbnail/lỗi file cấu hình qua `Option`/`Result` và giá trị mặc định.
- [x] **Không rò rỉ tài nguyên (Handle / Memory Leak):** Stress test 40 chu kỳ chuyển đổi liên tục ghi nhận `PRE_HANDLES=311`, `POST_HANDLES=311`, `PrivateMemory` giữ ổn định (~27.6 MB sau warm-up font/layer; 13.95 MB lúc khởi chạy).
- [x] **Không race condition:** Dữ liệu giữa `MediaWorker`/`VSyncTicker` và UI thread đồng bộ qua `mpsc`, `Condvar`, `AtomicBool` (`tick_pending`).
- [x] **Không tràn RAM vì ảnh/tệp:** Ảnh bìa từ GSMTC giới hạn tối đa 8 MB đầu vào và thu nhỏ bằng WIC về tối đa `96x96 px` (~36 KB BGRA) trên luồng nền, chỉ giải mã lại khi đổi bài hát.

### C. Đối Chiếu Chuẩn Dự Án & Giao Diện
- [x] **Không Emoji:** Đã kiểm tra toàn bộ mã nguồn Rust, giao diện, menu Tray, tài liệu không chứa ký tự emoji.
- [x] **Tiếng Việt chuẩn xác & UTF-8:** Mọi chuỗi hiển thị trên Island và menu System Tray dùng tiếng Việt có dấu chuẩn xác, hiển thị qua DirectWrite với cắt ellipsis tự động.
- [x] **Kiểm chứng thực tế:** Đã vượt qua `cargo clippy --all-targets -- -D warnings`, 37/37 `cargo test`, `cargo build --release` (`v0.2.3`, `714,240 bytes`) và kiểm tra bản chạy nền thực tế trên Desktop (`PID 5944`).

---

## PHẦN 2: NHẬT KÝ CÁC LỖI ĐÃ XỬ LÝ

### Lỗi #001: Rò rỉ refcount tiềm ẩn trong `D2D1_LAYER_PARAMETERS1`
- **Hiện tượng lỗi:** Kiểm tra định nghĩa `D2D1_LAYER_PARAMETERS1` trong `windows 0.62.2` cho thấy trường `geometricMask` và `opacityBrush` bọc `ManuallyDrop<Option<...>>`.
- **Nguyên nhân gốc rễ:** Nếu tạo mới COM pointer đưa vào `ManuallyDrop` mà không gọi `ManuallyDrop::drop` sau `PushLayer`, refcount của `ID2D1Geometry` sẽ tăng vĩnh viễn sau mỗi khung hình.
- **Cách sửa:** Gọi `ManuallyDrop::drop(&mut layer_params.geometricMask)` và `ManuallyDrop::drop(&mut layer_params.opacityBrush)` ngay sau `PushLayer` tại `src/renderer.rs` (cả lớp cắt khung viên thuốc lẫn lớp bo góc ảnh bìa).
- **Cách phòng tránh:** Bắt buộc kiểm tra mọi struct có trường `ManuallyDrop` khi làm việc với `windows-rs` và kiểm chứng bằng bài stress test handle/memory sau nhiều chu kỳ vẽ.

### Lỗi #002: So sánh session GSMTC theo AUMID dễ bỏ sót chuyển tab trình duyệt
- **Hiện tượng lỗi:** Nếu chỉ so sánh `SourceAppUserModelId()` trong `SessionTracker::attach`, khi người dùng đổi sang tab phát nhạc khác trong cùng trình duyệt Chrome/Edge, ứng dụng có thể giữ lại handler gắn trên session cũ.
- **Nguyên nhân gốc rễ:** Nhiều tab trình duyệt chia sẻ cùng một `SourceAppUserModelId` nhưng tạo ra các đối tượng COM `GlobalSystemMediaTransportControlsSession` khác nhau.
- **Cách sửa:** Đổi điều kiện so sánh trong `SessionTracker::attach` (`src/media.rs`) sang so sánh trực tiếp đối tượng COM `a == b` trước khi quyết định tái sử dụng hay thay thế session.
- **Cách phòng tránh:** Luôn so sánh định danh đối tượng COM thay vì chuỗi tên tiến trình khi theo dõi WinRT session.

### Lỗi #003: Bấm "Hiện Dynamic Island" từ Tray không hiện khi đang bật "Tự ẩn khi rảnh"
- **Hiện tượng lỗi:** Khi bật `auto_hide_when_idle = true` và chưa có nhạc, Island đang ở trạng thái ẩn (`Hidden`). Nếu bấm chuột trái vào Tray hoặc chọn mục Hiện trong menu Tray, hàm chỉ đảo `config.visible` nên `effective_mode` vẫn trả về `Hidden`.
- **Nguyên nhân gốc rễ:** Thiếu tách biệt giữa ý định "người dùng chủ động gọi hiện từ Tray" với trạng thái nghỉ tự ẩn.
- **Cách sửa:** Bổ sung `WindowController::toggle_visibility_from_tray` trong `src/window.rs`: nếu Island đang thực sự ẩn mà được người dùng bật lại, tự động chuyển tạm sang `IslandMode::Compact` kèm timer tự thu gọn sau 3 giây khi không có nhạc.
- **Cách phòng tránh:** Kiểm thử đầy đủ ma trận tương tác giữa `visible`, `auto_hide_when_idle` và lệnh từ System Tray.

### Lỗi #004: Sóng nhạc chạy ở tần số VSync cao ngay cả khi viên thuốc đang mảnh (`Slim`) hoặc mở bảng cài đặt
- **Hiện tượng lỗi:** Nếu giữ `VSyncTicker` bật mỗi khi `media.is_playing == true`, máy sẽ render lại 60–144 FPS liên tục suốt thời gian nghe nhạc dù thanh sóng nhạc không hiển thị ở chế độ `Slim`/`Settings`.
- **Nguyên nhân gốc rễ:** Điều kiện bật ticker ban đầu chưa lọc theo `IslandMode::Compact` và chưa tách nhịp thấp cho sóng nhạc.
- **Cách sửa:** Tách `TickerMode` (`Off`, `VSync`, `VisualizerSlow` 42 ms) trong `src/vsync.rs` và chỉ bật `VisualizerSlow` khi `mode == IslandMode::Compact` trong `src/window.rs`.
- **Cách phòng tránh:** Kiểm tra mức CPU ở trạng thái idle trên từng chế độ hiển thị bằng công cụ đo thực tế.

### Lỗi #005: Giữ nguyên logo Chrome thay vì cập nhật ảnh bìa YouTube thật và kéo dãn ảnh bìa 16:9
- **Hiện tượng lỗi:** Khi phát video YouTube trên Google Chrome, thẻ `Expanded` hiển thị logo Chrome (trong khi Windows Flyout hiển thị ảnh bìa video thật).
- **Nguyên nhân gốc rễ:** Chrome phát sự kiện `MediaPropertiesChanged` làm 2 nhịp (nhịp đầu chỉ có tiêu đề/nghệ sĩ cùng icon trình duyệt, nhịp sau mới có thumbnail thật nhưng giữ nguyên `Title` và `Artist`). Bộ lọc cũ trong `src/media.rs` chỉ so sánh chuỗi `title|artist` nên bỏ qua nhịp 2.
- **Cách sửa:** Chuyển sang đối chiếu mã băm FNV-1a 64-bit (`fnv1a64`) của dữ liệu thumbnail qua `MediaPropertiesChanged` và `TimelinePropertiesChanged` trong `src/media.rs`, đồng thời cắt vuông tâm ảnh (`compute_center_crop_rect`) trong `src/renderer.rs` để không kéo dãn ảnh 16:9.
- **Cách phòng tránh:** Không cache thumbnail chỉ theo tiêu đề/nghệ sĩ; luôn đối chiếu nội dung/hash dữ liệu ảnh và kiểm tra tỉ lệ khung hình khi vẽ lên khung vuông.

### Lỗi #006: Thiếu Timeline phát nhạc, lệch căn lề nhãn nguồn phát, chế độ `Slim` quá nhỏ và tự thu sau 3 giây
- **Hiện tượng lỗi:** Chế độ `Expanded` chưa hiện thanh tiến độ/thời gian và nhãn `Google Chrome` bị căn giữa lệch cột; khi rảnh/phát nhạc thanh thu bé `156x14` chỉ hiện 1 chấm xanh và tự thu gọn sau 3 giây.
- **Nguyên nhân gốc rễ:** Chưa đọc `GetTimelineProperties` của GSMTC; dùng nhầm format căn giữa cho nguồn phát; đặt kích thước `Slim` quá hẹp và dùng timer 3 giây thu về `Slim` khi có nhạc.
- **Cách sửa:** Bổ sung `TimelineState` + `TryChangePlaybackPositionAsync` (bấm tua trực tiếp) và timer 1 giây chỉ chạy khi đang mở `Expanded` phát nhạc; chuyển nguồn phát sang `small_left` căn thẳng lề trái; nâng `Slim` lên `196x30 DIP` có chữ `"Dynamic Island"`, `Compact` lên `276x36 DIP`, `Expanded` lên `416x160 DIP`, bỏ timer tự thu và giữ `Compact` xuyên suốt khi đang có nhạc.
- **Cách phòng tránh:** Kiểm thử giao diện thực tế với Chrome/YouTube đang phát nhạc và kiểm tra hành vi từng chế độ sau nhiều giây chờ.

### Lỗi #007: Lớp `HostBackdropBrush` hiển thị đen đặc trên cửa sổ Win32 `DesktopWindowTarget`
- **Hiện tượng lỗi:** Dù `CreateHostBackdropBrush()` trả về thành công và vùng cửa sổ đúng kích thước, viên thuốc trên desktop vẫn đen đặc không nhìn xuyên hậu cảnh.
- **Nguyên nhân gốc rễ:** Cửa sổ Win32 (`src/renderer.rs` khi chưa gọi `DwmSetWindowAttribute(DWMWA_USE_HOSTBACKDROPBRUSH)` và `DwmExtendFrameIntoClientArea`) không được DWM lấy mẫu hậu cảnh vào `HostBackdropBrush`; đồng thời `HostBackdropBrush` thô chưa đi qua bộ lọc `CLSID_D2D1GaussianBlur` (`CompositionEffectBrush`) và lớp phủ `glass_fill` ban đầu (`alpha 0.64..0.76`) quá đục.
- **Cách sửa:** Tại `src/renderer.rs`, bật `DWMWA_USE_HOSTBACKDROPBRUSH` và `DwmExtendFrameIntoClientArea(-1)` trước `CreateDesktopWindowTarget`, triển khai `HostBackdropBlurEffect` (`IGraphicsEffectD2D1Interop` + `CLSID_D2D1GaussianBlur`, `sigma = 18.0`, `D2D1_BORDER_MODE_HARD`), và giảm alpha `glass_fill` xuống `0.36..0.54`.
- **Cách phòng tránh:** Không chỉ dựa vào giá trị trả về thành công của WinRT API hoặc vùng `HRGN`; luôn kiểm thử điểm ảnh và chụp màn hình thực tế ngay trong phiên người dùng tương tác (Session 1) trên nhiều màu nền đối lập.

### Lỗi #008: Kéo thanh timeline khó bắt và dễ tự thu nhỏ viên thuốc khi thả chuột
- **Hiện tượng lỗi:** Khi ở chế độ `Expanded`, người dùng bấm kéo trên thanh timeline thường khó nắm chính xác, không thấy nút kéo trượt theo tay và hễ chuột lệch nhẹ khỏi dải thanh hoặc bấm trượt sang mép thời gian / vùng nút bấm rồi nhả ra thì viên thuốc tự thu nhỏ ngay.
- **Nguyên nhân gốc rễ:** `src/window.rs` chỉ xử lý tua tại `WM_LBUTTONUP` (`handle_left_click`), không có `WM_LBUTTONDOWN` + `SetCapture` để giữ thao tác kéo liên tục; vùng chạm `SeekTimeline` trong `src/state.rs` chỉ cao `20 DIP` (`88..108`) không có biên đón ở hai đầu; và mọi `WM_LBUTTONUP` rơi ra ngoài hitbox trong `Expanded` đều rơi xuống nhánh `self.collapsed_rest_mode()` làm thu gọn tức thì.
- **Cách sửa:** Nâng chiều cao hitbox `SeekTimeline` lên `29 DIP` (`top + 81.0..top + 110.0`) kèm `timeline_seek_hit_bounds` (`+-14 DIP` hai đầu), thêm chế độ `dragging_timeline` với `SetCapture`/`ReleaseCapture` gọi ngoài borrow `&mut WindowController` (kèm `WM_MOUSEACTIVATE -> MA_NOACTIVATE`, `WM_CAPTURECHANGED` và `timeline_scrub_ratio` xem trước trực tiếp trên [renderer.rs](src/renderer.rs)), giữ timeline `850 ms` sau khi nhả để không bị giật ngược bởi snapshot GSMTC trễ, và dùng `should_collapse_expanded_on_blank_click` chỉ cho phép thu gọn khi nhấn gọn ở vùng tiêu đề phía trên (`y < pill.top + 80.0`).
- **Cách phòng tránh:** Với mọi thanh trượt (slider) trên cửa sổ không kích hoạt (`WS_EX_NOACTIVATE`), luôn bắt buộc xử lý trọn bộ `WM_LBUTTONDOWN -> SetCapture -> WM_MOUSEMOVE -> WM_LBUTTONUP -> ReleaseCapture` ngoài `&mut` borrow, và không bao giờ gán thao tác thu gọn/đóng cửa sổ vào click hụt ở khu vực điều khiển.

### Lỗi #009: Kính lỏng bị lóa sáng và tẩy nhạt chữ/nút khi nằm đè lên cửa sổ nền trắng
- **Hiện tượng lỗi:** Khi mở `Settings` (`Bảng điều khiển nhanh`), `Slim` hoặc ở trạng thái chờ trên cửa sổ nền trắng thuần `#FFFFFF` (như Chrome), viên thuốc bị bạc xám sáng lóa, tiêu đề trắng giảm tương phản, phần nền hai bên lệch sáng mạnh khi một nửa đè lên vùng tối và nút `Thoát ứng dụng` bị nhạt thành màu hồng phấn.
- **Nguyên nhân gốc rễ:** Trong `src/renderer.rs`, nhánh `!has_video_aura` của `draw_pill_shell` hạ `glass_fill` xuống `opacity * 0.72` trên bộ gradient vốn đã rất mỏng (`alpha 0.24..0.36`, thực tế chỉ còn `0.17..0.26`), trong khi `glass_sheen` phủ tới `0.34` trắng đục ở mép trên và `ambient_glow_brush` vẫn giữ nguyên độ sáng aura bài hát (`0.28`) ngay cả trong `Settings`; đồng thời `surface_card` (`0.36`) và `danger_bg` (`0.32` đỏ sáng) quá trong suốt trước nền `#FFFFFF`.
- **Cách sửa:** Tại `src/renderer.rs`, nâng lớp kính khói `glass_fill` lên `alpha 0.62..0.72` và bỏ hệ số giảm `0.72` ở nhánh `!has_video_aura` (giữ `0.78` khi có `artwork_aura` kèm trần sáng aura `0.64`), giảm `glass_sheen` xuống `0.11` ở đỉnh, tiết chế `ambient_glow_brush` ở nhánh không dùng aura còn `0.24`, tăng viền bóng ngoài `glass_edge_shadow` (`0.34`), làm đậm thẻ `surface_card` (`0.50` kèm viền `card_border` `0.09`), rãnh `switch_off` và `danger_bg` đỏ trầm (`0.58, 0.10, 0.13, 0.64`); bổ sung unit test GPU `smoked_glass_preserves_translucency_and_contrast_over_white`.
- **Cách phòng tránh:** Luôn kiểm thử độ tương phản và độ sáng tổng hợp trên cả hai cực nền `#000000` và `#FFFFFF` bằng đọc lại pixel D2D (`CPU_READ`) cho các bề mặt `HostBackdropBrush` trước khi phát hành.

### Lỗi #010: Animation giật lệch pha 144Hz và kẹt trạng thái hover / nút bấm / HUD
- **Hiện tượng lỗi:** Co giãn viên thuốc và kéo thanh trượt trên màn hình 144Hz không mượt (khựng khung hình đầu và sượng 64Hz), đôi khi bị kẹt ở chế độ `Expanded`/`Compact`, tự đóng `Settings` khi bấm trượt khe nút hoặc tụt bất ngờ xuống `Slim` dưới con trỏ sau khi HUD tắt.
- **Nguyên nhân gốc rễ:**
  - `src/vsync.rs` dựa vào `EnumOutputs(0)` -> `WaitForVBlank()`, khi trả về `None` rơi vào `thread::sleep(8ms)` (~15.6 ms lệch pha DWM 144Hz); `src/window.rs` bỏ qua khung hình đầu khi bắt đầu animation và dùng `SetTimer(12ms)` cho kéo thanh trượt.
  - `src/renderer.rs` chưa warm-up DirectWrite (frame đầu `Expanded` mất `58.42 ms`), tạo mới `IDWriteTextLayout` và 3 cặp `CreateRoundedRectangleGeometry` + `PushLayer/PopLayer` mỗi frame.
  - `src/state.rs` ép `content_alpha.snap(0.2)` gây nháy nội dung và dùng hệ số `top` bị vọt âm cắt phẳng tại `0.0`.
  - `src/window.rs` cập nhật `SetWindowRgn` GDI mỗi khung hình animation làm DWM cắt mép và dễ rớt `WM_MOUSELEAVE`; thiếu `pressed_action`, `sync_pointer_presence` và xử lý `reset_pointer_and_drag_state` khi ẩn/fullscreen; `src/media.rs` kiểm tra lại ảnh bìa ở mọi sự kiện `TimelinePropertiesChanged`.
- **Cách sửa:** Đồng bộ `VSyncTicker` bằng `DwmFlush()` (fallback `7ms`); vẽ ngay frame đầu và chạy kéo slider theo nhịp `VSync`; dùng `ID2D1BitmapBrush` và cache `IDWriteTextLayout` + `warm_up_text_formats` trong `src/renderer.rs`; bỏ `content_alpha.snap(0.2)` và đặt `top` tắt dần tới hạn trong `src/state.rs`; cập nhật `SetWindowRgn` bằng vùng bao khi đang mở rộng (`update_window_region`), thêm `pressed_action`, `sync_pointer_presence`, bảo vệ `Settings` không đóng khi click trượt khe nút và giữ `Compact` khi chuột vẫn ở trên viên thuốc lúc `clear_hud`.
- **Cách phòng tránh:** Không phụ thuộc `EnumOutputs` đơn lẻ cho nhịp VSync trên laptop Hybrid GPU; đo thời gian từng frame bằng bản `--release` trên phần cứng đích; luôn đồng bộ `mouse_tracked` theo vị trí con trỏ thực tế khi animation kết thúc hoặc thay đổi `SetWindowRgn`.

### Lỗi #011: Khựng HUD khi cắm/rút sạc và nhảy bố cục chữ trong lúc co giãn
- **Hiện tượng lỗi:** Khi cắm hoặc rút sạc trên laptop, HUD Pin/Sạc bị khựng nhẹ một nhịp; trong lúc viên thuốc bung từ `Slim` lên `Compact`/`Expanded`, chữ bị cắt dấu `...` và trượt cache `IDWriteTextLayout` liên tục.
- **Nguyên nhân gốc rễ:** `WM_POWERBROADCAST` gọi đồng bộ `GetSystemPowerStatus()` (truy xuất ACPI EC) trên UI thread ngay lúc hệ thống đổi nguồn và phát `WM_DISPLAYCHANGE` kéo theo `SetWindowPos` dư thừa; `set_target_mode` đặt `last_step = Some(Instant::now())` làm bước animation đầu có `dt ~ 0`; nhánh `DwmFlush()` ngủ `7ms` nếu lần flush đầu chỉ mất `< 2ms`; và `draw_compact` / `draw_compact_hud` / `draw_expanded` / `draw_settings` bám theo `pill.width` đang co giãn từng frame.
- **Cách sửa:** Đọc tức thời AC/DC từ `POWERBROADCAST_SETTING` (`evaluate_ac_hint`), đọc `GetSystemPowerStatus()` bất đồng bộ trên luồng nền (`PowerSync`), cache `last_window_rect` trước `SetWindowPos`; bỏ gán `last_step` sớm trong `set_target_mode`; chỉ fallback ngủ ở `vsync.rs` khi `< 400 us` 3 lần liên tiếp; và dùng `virtual_pill_anchored` neo kích thước nội dung cố định khi co giãn kết hợp `clip_layer`.
- **Cách phòng tránh:** Tuyệt đối không gọi API ACPI/nguồn đồng bộ trên luồng giao diện và luôn neo bố cục nội dung theo kích thước đích cố định để lớp cắt bo tròn thực hiện hiệu ứng mở màn mượt mà không làm nhảy chữ.

### Lỗi #012: Lệch pha kính nền WinRT với viền/chữ, tụt chữ theo chiều dọc, cắt góc `SetWindowRgn` và dao động hover ở 144Hz
- **Hiện tượng lỗi:** Khi co giãn nhanh trên màn hình 144Hz: lớp kính mờ bị tách nhịp khỏi viền và chữ của viên thuốc; nội dung `Compact` tụt thấp giữa khung khi thu gọn từ `Expanded`/`Settings` về `Compact`; mép bo tròn đôi lúc bị cắt góc hoặc nháy; và con trỏ ở vùng rìa ngoài lúc mở dễ làm dao động hover.
- **Nguyên nhân gốc rễ:**
  - `src/renderer.rs`: `WinRtBackdropLayer` (`SpriteVisual`) commit qua hàng đợi WinRT Composition trong khi lớp nội dung D2D dùng `IDXGISwapChain1::Present(0, 0)` tách rời nhịp trình bày, dẫn đến lệch khung giữa nền mờ và viền/chữ; `virtual_pill_anchored` chưa kẹp biên lệch dọc (`center_y`) khi khung đang thu nhỏ từ chiều cao lớn hơn nhiều so với `Compact`; và chuyển cảnh thiếu hàm ẩn/hiện mượt theo kích thước (`mode_content_reveal_alpha`).
  - `src/window.rs`: `update_window_region` cần phân biệt rành mạch `diameter == 0` cho vùng chữ nhật bao ngoài lúc đang animate; và `handle_mouse_move` kiểm tra `!pill_contains_dip(..., 2.0)` khi không phân biệt `requested_mode == IslandMode::Slim`.
- **Cách sửa:**
  - `Cargo.toml` & `src/renderer.rs`: Thêm `"Graphics_DirectX"`, chuyển nhánh `CompositionHost::WinRtGlass` sang `CompositionGraphicsDevice` + `CompositionDrawingSurface` + `ICompositionDrawingSurfaceInterop` (`BeginDraw` / `EndDraw`, giữ `IDXGISwapChain1` cho `DirectCompFallback`) dùng chung `DrawCanvas<'_>` để vẽ và commit đồng pha cùng `WinRtBackdropLayer`; thêm `mode_content_reveal_alpha` và kẹp độ lệch dọc của `virtual_pill_anchored` về `[-10.0, 0.0]`.
  - `src/window.rs`: Tách `apply_rect_region` (`diameter == 0`) cho suốt giai đoạn `is_animating()` trong `update_window_region` (chỉ áp `apply_round_region` khi dừng hẳn) và giới hạn điều kiện bỏ qua `pill_contains_dip` trong `handle_mouse_move` chỉ dành cho `IslandMode::Slim`.
- **Cách phòng tránh:** Khi kết hợp hiệu ứng nền `SpriteVisual` của `Windows::UI::Composition` với nét vẽ vector Direct2D trên cửa sổ không viền, luôn vẽ nội dung lên `CompositionDrawingSurface` thuộc cùng `Compositor` thay vì trộn `IDXGISwapChain1::Present` riêng lẻ; không áp `CreateRoundRectRgn` từng khung hình khi lò xo đang chạy.

### Lỗi #013: Điều khiển hệ thống nhuốm màu ảnh bìa bài hát, lệch trọng tâm `Expanded`, trống đáy `AudioDevices` và `mode_content_reveal_alpha` phụ thuộc chiều cao cố định
- **Hiện tượng lỗi:** Khi phát nhạc tông đỏ, công tắc bật, nút `"Quay lại"`, nhãn `"Đổi"` trong `Settings` và thẻ `"Đang dùng"` trong `AudioDevices` bị nhuốm đỏ đồng dạng nút thoát; nút `Cài đặt` ở góc trên phải `Expanded` làm cắt sớm tiêu đề và lệch đáy; `AudioDevices` chừa mảng đen lớn dưới 3 thiết bị; và nếu co chiều cao `AudioDevices` xuống `107 DIP` (1 thiết bị) thì công thức cũ `(pill.height - 175.0)/80.0` làm ẩn hoàn toàn nội dung.
- **Nguyên nhân gốc rễ:** `BrushPalette` (`src/renderer.rs`) dùng chung `brushes.accent` / `brushes.accent_soft` (vốn bị `apply_media_theme_colors` ghi đè khi đổi ảnh bìa) cho cả bảng cấu hình hệ thống; `target_dimensions_for_mode(IslandMode::AudioDevices)` cố định `284.0 DIP`; `mode_content_reveal_alpha` giả định cứng `AudioDevices` luôn cao như `Settings`.
- **Cách sửa:** Tách `sys_accent` / `sys_accent_soft` cố định trong `BrushPalette`, chuyển nút `"Quay lại"` sang `text_primary`; đưa `OpenSettings` xuống góc dưới phải `Expanded` (`424 x 160 DIP`); tính chiều cao `AudioDevices` linh hoạt theo `audio_devices_height_for_count` và chuẩn hóa `mode_content_reveal_alpha` theo `target_h` thực tế; đồng thời tách `settings_return_mode` / `audio_devices_return_mode` trong `src/window.rs`.
- **Cách phòng tránh:** Luôn tách riêng bảng màu thích ứng media với màu điều khiển hệ thống, luôn chụp ảnh thực tế tất cả các màn hình sau khi thay đổi giao diện, và trong unit test `IslandAnimator::step` phải tăng thời gian mô phỏng cố định (như `+8 ms/step`) thay vì gọi `Instant::now()` liên tục gây kéo dài thời gian test.

### Lỗi #014: Tắt `Gọn mép 4s` vẫn bị thu gọn về `Slim` khi khởi động, rời chuột lúc không nhạc hoặc quay lại từ `Settings`
- **Hiện tượng lỗi:** Dù người dùng đã tắt `Gọn mép 4s` (`notch_collapse_after_4s = false`) và tắt `Tự ẩn khi rảnh` (`auto_hide_when_idle = false`), viên thuốc vẫn khởi động ở dạng `Slim` (`152 x 16`) hoặc tự co về `Slim` khi không có nhạc/rời chuột/tắt timer 4s/bấm `"Quay lại"` từ `Settings`.
- **Nguyên nhân gốc rễ:** Trong `src/window.rs`, `IslandWindow::create` luôn gán `requested_mode = IslandMode::Slim`; hàm `collapsed_rest_mode()` và các điểm chuyển cảnh (`handle_media_update`, `clear_hud`, `handle_mouse_leave`, `handle_compact_to_notch_timer`, `handle_auto_collapse_timer`, `ToggleNotchCollapse`, `IDM_TOGGLE_EXPANDED`) ép về `Slim` mỗi khi `!self.media_state.has_session` mà không kiểm tra `!self.config.notch_collapse_after_4s`; riêng `ToggleNotchCollapse` khi tắt từ `Slim` không cập nhật `settings_return_mode`.
- **Cách sửa:** Hợp nhất chính sách vào `WindowController::should_collapse_to_slim()` (`notch_collapse_after_4s || (auto_hide_when_idle && !media_state.has_session)`), cập nhật `collapsed_rest_mode()`, `IslandWindow::create`, toàn bộ nhánh chuyển đổi trong `src/window.rs`, thêm `sync_mode_after_collapse_policy_change()` cho `ToggleNotchCollapse` và `ToggleAutoHideIdle`, đồng thời bổ sung kịch bản kiểm thử bước #13.
- **Cách phòng tránh:** Gom mọi điều kiện chuyển về chế độ thu gọn (`Slim`) vào một hàm chính sách duy nhất (`should_collapse_to_slim`) và viết test tích hợp cho cả hai cực cấu hình (`notch_collapse_after_4s = true` và `false`) khi không có session media.
