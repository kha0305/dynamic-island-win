# NHẬT KÝ HOẠT ĐỘNG DỰ ÁN WIN DYNAMIC ISLAND

Tài liệu theo dõi tiến độ kỹ thuật, các quyết định thiết kế, kết quả kiểm thử và trạng thái bàn giao giữa các phiên làm việc.

---

## Mục #001: Khởi tạo thư mục dự án, bộ quy chuẩn và ngữ cảnh ban đầu

- **Thời gian thực hiện:** 2026-11-27T17:47:00+07:00
- **Phạm vi công việc:** Thiết lập thư mục dự án mới tại `C:\Users\ASUS\Desktop\New folder\win-dynamic-island` theo đầy đủ quy chuẩn toàn cục (`global_rules.md`) và lưu trữ trọn vẹn ngữ cảnh hình thành ý tưởng từ phiên trao đổi trước.
- **Các tệp đã tạo:**
  1. `GEMINI.md` — Luật riêng và ràng buộc kỹ thuật cho dự án Dynamic Island bằng Rust trên Windows 11.
  2. `NHAT_KY_HOAT_DONG.md` — Nhật ký hoạt động và thông tin bàn giao phiên.
  3. `SO_TAY_KIEM_TRA_BUG.md` — Sổ tay đối chiếu và phòng ngừa lỗi đặc thù Rust + Win32/Overlay.
  4. `TASK_MEMORY.md` — Bộ nhớ tổng hợp bối cảnh dự án, hiện trạng máy và các quyết định đang mở.
  5. `specs/dynamic-island/spec.md` — Đặc tả bài toán Phase 2 theo quy trình Spec-First & Ponytail.
  6. `README.md` — Tài liệu tổng quan và hướng dẫn tiếp quản phiên mới.
  7. `.gitignore` — Loại trừ tệp build nặng (`target/`, `node_modules/`, `dist/`, `.pdb`, `.log`).
- **Kết quả kiểm tra:** Đã kiểm tra tồn tại hệ thống công cụ Rust (`rustc 1.100.0`, `cargo 1.100.0`) và thông số máy (`Windows 11 Pro Build 26300`, AMD Radeon + NVIDIA RTX 3050 Laptop GPU); toàn bộ tài liệu khởi tạo không chứa emoji, chuẩn UTF-8 tiếng Việt có dấu.
- **Trạng thái:** ĐÃ HOÀN TẤT THIẾT LẬP KHUNG DỰ ÁN

---

## Bài học rút ra (Post-Task Learning)
- **Kiểm soát dung lượng build Rust:** Từ kinh nghiệm dọn dẹp 26.22 GB thư mục `target/` debug ở không gian làm việc trước đó, dự án này cần cấu hình `.gitignore` chuẩn ngay từ đầu và phân tách rõ tệp `.exe` release thành phẩm khi xuất bản.
- **Tránh đánh thức GPU rời trên laptop Hybrid:** Trên máy có cả AMD Radeon và NVIDIA RTX 3050 Laptop GPU, các thành phần giao diện nền Windows nếu gọi trực tiếp tài nguyên tăng tốc nặng không cần thiết có thể đánh thức GPU rời gây tốn pin hoặc trễ thao tác. Cần thiết kế lớp vẽ gọn nhẹ và ưu tiên iGPU.

---

## Mục #002: Hoàn tất Bước 1 — Khung dự án, Cửa sổ Overlay DirectComposition và System Tray

- **Thời gian thực hiện:** 2026-11-27T17:58:35+07:00
- **Phạm vi công việc:** Triển khai và kiểm chứng toàn bộ Bước 1 (T1.1 đến T1.4) theo kế hoạch đã duyệt.
- **Các tệp đã tạo/cập nhật:**
  1. `Cargo.toml` — Chỉ dùng duy nhất crate `windows = "0.62.2"` (15 gói phụ thuộc trong `Cargo.lock`).
  2. `src/state.rs` — Định nghĩa `IslandMode`, `AppConfig`, tính toán bề mặt cố định, căn giữa đa màn hình/DPI và vùng `SetWindowRgn` có phần đệm antialiasing 2 px.
  3. `src/tray.rs` — Tạo biểu tượng viên thuốc 16x16 trong bộ nhớ (`CreateIconIndirect`) và System Tray (`Shell_NotifyIconW`) với menu tiếng Việt có dấu chuẩn, dọn dẹp tài nguyên qua `Drop`.
  4. `src/renderer.rs` — Khởi tạo DXGI chọn GPU tiết kiệm điện (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`), D3D11 + DirectComposition + Direct2D vẽ viên thuốc bo tròn lên SwapChain kích thước cố định.
  5. `src/window.rs` — Cửa sổ `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE`, cập nhật `SetWindowRgn` xuyên chuột vùng trống, xử lý hover/click và DPI.
  6. `src/main.rs` — Kích hoạt `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`, khởi tạo COM apartment guard và vòng lặp thông điệp Win32.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test`: 5/5 test đạt, xác nhận adapter DXGI được chọn là `AMD Radeon(TM) Graphics` (không chọn NVIDIA RTX 3050).
  - Chạy thử `target\debug\win-dynamic-island.exe`: Cửa sổ `WinDynamicIslandWindowClass` có cờ `EXSTYLE=0x08200088`, `RECT=(740,0,1180,164)`, `RGN_BOX=(142,4,297,21)` (`COMPLEXREGION`), Working Set `23.83 MB`, nhận lệnh thoát `IDM_EXIT` với `CLEAN_EXIT=True`, `EXIT_CODE=0`.
- **Trạng thái:** ĐÃ HOÀN TẤT BƯỚC 1 — Sẵn sàng chuyển sang Bước 2 (Vật lý lò xo mượt VSync & DirectWrite).

---

## Mục #003: Hoàn tất MVP v0.1 (Bước 2–4), Menu trực quan và Stress Test không lỗi

- **Thời gian thực hiện:** 2026-11-27T18:18:00+07:00
- **Phạm vi công việc:** Triển khai hoàn chỉnh vật lý lò xo đồng bộ VSync, lớp vẽ Direct2D/DirectWrite, tích hợp Windows GSMTC + WIC thumbnail, bảng điều khiển trực quan ngay trên Island, menu System Tray phân nhóm kèm submenu màn hình, phát hiện Fullscreen, lưu/nạp cấu hình và stress test bản release.
- **Các tệp đã tạo/cập nhật:**
  1. `Cargo.toml` — Thêm `windows-numerics = "0.3.1"` và bật các feature WinRT/WIC/DWM/Accessibility/Registry/Security của `windows = "0.62.2"`.
  2. `src/state.rs` — Bổ sung `SpringScalar` sub-step cố định, `IslandAnimator` cross-fade nội dung, `MediaDisplayState` và hệ thống hitbox dùng chung (`fill_interactive_hitboxes`).
  3. `src/vsync.rs` — Luồng đồng bộ VBlank/sóng nhạc với 3 mức `TickerMode` (`Off`, `VSync`, `VisualizerSlow`) ngủ hoàn toàn khi đứng yên.
  4. `src/renderer.rs` — Vẽ Direct2D + DirectWrite hỗ trợ cắt ellipsis UTF-8, clip bo tròn `D2D1_LAYER_PARAMETERS1`, tái sử dụng buffer UTF-16 & hitbox, upload ảnh bìa bo góc và giao diện `Settings` trực quan.
  5. `src/media.rs` — Luồng nền WinRT GSMTC (`RoInitialize`), lắng nghe đổi bài/trạng thái phát, giải mã và thu nhỏ ảnh bìa về tối đa `96x96 px` bằng WIC.
  6. `src/config.rs` — Đọc/ghi cấu hình `%LOCALAPPDATA%\WinDynamicIsland\config.ini` và quản lý Autostart Registry `HKCU\...\Run` khi người dùng chọn.
  7. `src/watcher.rs` — Liệt kê đa màn hình + DPI (`EnumDisplayMonitors`), hook `EVENT_SYSTEM_FOREGROUND` (`SetWinEventHook`) và phát hiện Fullscreen kết hợp `SHQueryUserNotificationState`.
  8. `src/tray.rs` — Nâng cấp menu System Tray phân nhóm rõ ràng, hiển thị trạng thái, submenu danh sách màn hình thật và các nút điều khiển nhanh.
  9. `src/window.rs` & `src/main.rs` — Hợp nhất điều phối sự kiện, con trỏ bàn tay/mũi tên, ẩn tức thì khi Fullscreen và xử lý khôi phục từ Tray khi tự ẩn lúc rảnh.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test`: **16/16** test đạt (bao gồm kiểm thử tích hợp Win32 `HWND` thật cho Fullscreen và kiểm chứng chọn adapter `AMD Radeon(TM) Graphics`).
  - `cargo build --release`: Tạo tệp `target\release\win-dynamic-island.exe` dung lượng **407.5 KB**.
  - Kiểm thử bản release `.exe`:
    - Khởi tạo idle 3 giây: `CPU_3S_MS=0.00`, `WS_MB=42.16`, `PRIV_MB=13.95`, `HANDLES=300`.
    - Stress test 40 chu kỳ co giãn liên tục: `PRE_HANDLES=311` -> `POST_HANDLES=311` (0 rò rỉ handle), `PRE_PRIV_MB=27.58` -> `POST_PRIV_MB=27.69`, `POST_IDLE_CPU_2S_MS=0.00`.
    - `nvidia-smi`: Không ghi nhận `win-dynamic-island.exe` chạy trên GPU rời NVIDIA RTX 3050.
    - Kiểm thử toàn bộ nút trên giao diện `Settings` (`Tự ẩn khi rảnh`, `Ẩn toàn màn hình`, `Sóng nhạc động`, `Quay lại`, `Cài đặt`, `Ẩn tạm`, `Thoát ứng dụng`), lưu `config.ini`, khôi phục từ Tray và thoát sạch (`CLEAN_EXIT=True`, `EXIT_CODE=0`).
- **Trạng thái:** ĐÃ HOÀN TẤT NGHIỆM THU MVP v0.1

---

## Mục #004: Bổ sung Timeline + Tua nhạc, sửa bắt ảnh bìa Chrome và tối ưu kích thước hiển thị

- **Thời gian thực hiện:** 2026-11-27T18:38:30+07:00
- **Phạm vi công việc:** Xử lý toàn bộ phản hồi thực tế từ ảnh chụp màn hình trên Windows 11 + Chrome YouTube.
- **Các thay đổi đã triển khai:**
  1. `src/state.rs`: Thêm `TimelineState` (tính tiến độ theo `FILETIME` ticks, format `MM:SS`/`H:MM:SS`, tính vị trí tua `seek_ticks_for_ratio`), nâng kích thước `Slim` (`196x30` DIP), `Compact` (`276x36` DIP), `Expanded` (`416x160` DIP) và hitbox `SeekTimeline`.
  2. `src/media.rs`: Lắng nghe `TimelinePropertiesChanged`, đọc `GetTimelineProperties()`, hỗ trợ `MediaCommand::Seek` (`TryChangePlaybackPositionAsync`), kiểm tra thay đổi ảnh bìa bằng mã băm `fnv1a64` để bắt được ảnh YouTube tải ở nhịp 2 của Chrome.
  3. `src/renderer.rs`: Cắt vuông tâm ảnh (`compute_center_crop_rect`) cho thumbnail 16:9, sửa nhãn nguồn phát sang `small_left` căn trái thẳng hàng, vẽ thanh Timeline có điểm chỉ báo hover và hiển thị rõ ràng hơn ở `Slim`.
  4. `src/window.rs`: Bỏ timer 3 giây tự thu về thanh mảnh; giữ `Compact` xuyên suốt khi đang có nhạc, giữ `Expanded` cho tới khi người dùng chủ động đóng, và chỉ bật timer timeline 1 giây khi đang mở `Expanded` phát nhạc.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test`: **18/18** test đạt (bổ sung test `fnv1a` và `timeline_progress_and_formatting_work_accurately`).
  - `cargo build --release`: Tạo `target\release\win-dynamic-island.exe` dung lượng **419.0 KB**.
  - Kiểm thử chạy thật trên bản release cùng session Chrome đang phát: Tự mở `Compact` (`276x36`, `INIT_RGN=(92,4,371,43)`, `IDLE_3S_MS=15.63`, `PRIV_MB=19.58`), bung `Expanded` (`416x160`, `EXPANDED_RGN=(22,4,441,167)`) giữ nguyên sau 4 giây không tự thu, bấm Timeline và nút `Cài đặt` mở `Settings`, thoát sạch (`CLEAN_EXIT=True`, `EXIT_CODE=0`).
- **Trạng thái:** ĐÃ HOÀN TẤT SỬA LỖI VÀ NGHIỆM THU LẠI

---

## Mục #005: Nâng cấp nút điều khiển tròn trịa bằng hình học vector Direct2D

- **Thời gian thực hiện:** 2026-11-27T18:41:25+07:00
- **Phạm vi công việc:** Thay thế các ký tự ASCII `<<`, `||`, `>`, `>>` bằng biểu tượng vector Direct2D bo góc tròn mềm mại và nút tròn hoàn hảo theo phản hồi giao diện.
- **Các thay đổi đã triển khai:**
  1. `src/state.rs`: Chỉnh kích thước 3 nút thành hình tròn hoàn hảo (`34x34` cho Trước/Tiếp và `38x38` cho Phát/Tạm dừng).
  2. `src/renderer.rs`: Tạo sẵn `IconGeometries` (`ID2D1PathGeometry` + `ID2D1StrokeStyle` với `D2D1_CAP_STYLE_ROUND` và `D2D1_LINE_JOIN_ROUND`) và các thanh bo tròn `FillRoundedRectangle` cho biểu tượng Pause/Prev/Next; bo tròn capsule `height * 0.5` cho nút `Cài đặt`.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test`: **18/18** test đạt.
  - `cargo build --release`: Tạo `target\release\win-dynamic-island.exe` (`420.5 KB`).
  - Kiểm thử chạy thật bản release: `EXPANDED_RGN=(22,4,441,167)`, `WS_MB=41.14`, `PRIV_MB=20.64`, bấm nút Phát/Tạm dừng ổn định; kiểm chứng tự thu gọn sau khi rời chuột (`WM_MOUSELEAVE`, cấu hình 10 giây), `CLEAN_EXIT=True`, `EXIT_CODE=0`.
- **Trạng thái:** ĐÃ HOÀN TẤT NÂNG CẤP GIAO DIỆN NÚT TRÒN

---

## Mục #006: Hiệu ứng nền kính mờ (Glass Backdrop) thời gian thực và chỉnh tự thu gọn 5 giây

- **Thời gian thực hiện:** 2026-11-27T19:01:00+07:00
- **Phạm vi công việc:** Triển khai nền hiệu ứng kính mờ (Glass) phía sau viên thuốc và rút ngắn thời gian tự thu gọn sau khi rời chuột từ 10 giây xuống 5 giây.
- **Các thay đổi đã triển khai:**
  1. `Cargo.toml`: Bổ sung các feature `System`, `UI_Composition`, `UI_Composition_Desktop`, `Win32_System_WinRT_Composition` của crate `windows = "0.62.2"`.
  2. `src/window.rs`: Chỉnh `AUTO_COLLAPSE_DELAY_MS` từ `10_000` xuống `5_000`; gọi `self.renderer.hide_backdrop()` khi cửa sổ chuyển về trạng thái ẩn (`should_hide_window()`) và cập nhật tooltip khay hệ thống.
  3. `src/renderer.rs`:
     - Bổ sung `CompositionHost` ưu tiên `WinRtGlass` (`CreateDispatcherQueueController` + `Windows::UI::Composition::Compositor` + `DesktopWindowTarget` + `HostBackdropBrush` cắt bo tròn bằng `CompositionRoundedRectangleGeometry`) ghép dưới `SwapChain` nội dung, kèm cơ chế fallback an toàn về `IDCompositionDevice`.
     - Thêm `BackdropState` delta-cache trong `WinRtBackdropLayer` để bỏ qua hoàn toàn các lời gọi WinRT Composition dư thừa khi kích thước/vị trí/độ mờ viên thuốc không đổi (tiết kiệm CPU khi chạy visualizer/timeline).
     - Thay lớp nền đặc bằng 3 `ID2D1LinearGradientBrush` tái sử dụng (`glass_fill` tối mờ giữ tương phản chữ, `glass_sheen` ánh kính phía trên, `glass_border` viền sáng phản quang) và bổ sung unit test `winrt_glass_backdrop_initializes_and_renders`.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test -- --nocapture`: **19/19** test đạt (`WinRT Glass backdrop active: true | GPU: AMD Radeon(TM) Graphics`).
  - `cargo build --release`: Tạo `target\release\win-dynamic-island.exe` (**435.5 KB**).
  - Kiểm thử chạy thật bản release: `EXSTYLE=0x8200088`, `INIT_RGN=(92,4,371,43)`, `SETTLED_2S_CPU_MS=0`, `WS_MB=41.61`, `PRIV_MB=17.71`, `HANDLES=355`, `NV_DRIVER_MODULES=0`; mở rộng `EXPANDED_RGN=(22,4,441,167)` -> vẫn giữ mở tại 2,5 giây (`AT_2_5S_RGN=(22,4,441,167)`) -> tự thu gọn sau 5 giây (`AT_5_9S_RGN=(92,4,371,43)`), `CLEAN_EXIT=True`, `EXIT_CODE=0`, `LEFTOVER_PROCS=0`, `EMOJI_HITS=0`.
- **Trạng thái:** ĐÃ HOÀN TẤT HIỆU ỨNG GLASS VÀ TỰ THU GỌN 5 GIÂY

---

## Mục #007: Kích hoạt DWM HostBackdrop + bộ lọc GaussianBlur thật cho nền Glass

- **Thời gian thực hiện:** 2026-11-27T19:13:00+07:00
- **Phạm vi công việc:** Khắc phục tình trạng viên thuốc vẫn đen đặc ở lần dựng `435.5 KB` và hoàn thiện hiệu ứng kính mờ nhìn xuyên hậu cảnh rõ ràng.
- **Các thay đổi đã triển khai:**
  1. `Cargo.toml`: Bổ sung phụ thuộc `windows-core = "0.62.2"` và hai feature `Graphics_Effects`, `Win32_System_WinRT_Graphics_Direct2D`.
  2. `src/renderer.rs`:
     - Bật `DWMWA_USE_HOSTBACKDROPBRUSH` (`TRUE`) và `DwmExtendFrameIntoClientArea(-1)` cho `HWND` trước khi gắn `DesktopWindowTarget`.
     - Triển khai `HostBackdropBlurEffect` (`IGraphicsEffect` + `IGraphicsEffectSource` + `IGraphicsEffectD2D1Interop`) dùng `CLSID_D2D1GaussianBlur` (`blur_sigma = 18.0`, `D2D1_BORDER_MODE_HARD`) làm mờ `HostBackdropBrush` thông qua `CompositionEffectBrush`.
     - Giảm độ đục `glass_fill` xuống `0.36 -> 0.44 -> 0.54`, tăng độ rõ `glass_sheen` (`0.22`) và `glass_border` (`0.48`) để ánh sáng và màu hậu cảnh xuyên qua mềm mại.
     - Cập nhật kiểm thử `winrt_glass_backdrop_initializes_and_renders` kiểm tra thêm `has_blur_effect()`.
  3. `src/window.rs`: Hiển thị trạng thái `Kính mờ Acrylic` trên tooltip khay hệ thống.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test -- --nocapture`: **19/19** test đạt (`WinRT Glass backdrop active: true | Blur effect active: true | GPU: AMD Radeon(TM) Graphics`).
  - `cargo build --release`: Tạo `target\release\win-dynamic-island.exe` (**449.5 KB**).
  - Kiểm chứng điểm ảnh và chụp màn hình thực tế trong phiên người dùng (Session 1):
    - Nền xanh lơ `CYAN_OUT=(0,230,255)` -> bên trong viên thuốc `CYAN_INSIDE=(6,123,140)`; nền hồng `MAG_OUT=(255,55,145)` -> bên trong viên thuốc `MAG_INSIDE=(135,35,84)`; chụp trực tiếp trên Windows Terminal xác nhận chữ và hình nền mờ xuyên qua rõ rệt.
    - `EXP_RGN=(22,4,441,167)`, `SETTLED_2S_CPU_MS=0`, `WS_MB=41.37`, `PRIV_MB=20.09`, `HANDLES=346`, tự thu gọn sau 5 giây, `CLEAN_EXIT=True`.
- **Trạng thái:** ĐÃ HOÀN TẤT KHẮC PHỤC VÀ NGHIỆM THU HIỆU ỨNG KÍNH MỜ THẬT

---

## Mục #008: Tinh chỉnh phong cách Liquid Glass trong suốt và ánh sáng thích ứng theo ảnh bìa

- **Thời gian thực hiện:** 2026-11-27T19:22:00+07:00
- **Phạm vi công việc:** Nâng độ trong suốt của lớp nền kính lên chuẩn Liquid Glass giống mẫu tham chiếu người dùng gửi, giữ đọc rõ cả trên thanh Chrome sáng lẫn nền tối.
- **Các thay đổi đã triển khai:**
  1. `src/renderer.rs`:
     - Nối chuỗi hiệu ứng WinRT Composition `HostBackdropBrush -> CLSID_D2D1Saturation (1.32) -> CLSID_D2D1GaussianBlur (sigma = 7.5, D2D1_BORDER_MODE_HARD)` để hậu cảnh xuyên qua giữ được sắc màu và hình khối khúc xạ mềm mại thay vì xám đục.
     - Giảm alpha `glass_fill` xuống `0.17 -> 0.22 -> 0.28`, thêm vòng khúc xạ bên trong `glass_inner_rim` và viền sáng Fresnel 2 chiều `glass_border` (`0.86` đỉnh -> `0.58` đáy).
     - Bổ sung `extract_vibrant_accent_rgb` và `ID2D1RadialGradientBrush` (`ambient_glow_brush`) tỏa sáng nhẹ theo màu chủ đạo ảnh bìa bài hát; thêm `text_shadow` nhẹ (`alpha 0.36`, lệch `0.6 DIP`) và nút điều khiển kính tối mờ (`surface_card alpha 0.30`).
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test -- --nocapture`: **19/19** test đạt (`WinRT Glass backdrop active: true | Blur effect active: true | GPU: AMD Radeon(TM) Graphics`).
  - `cargo build --release`: Tạo `target\release\win-dynamic-island.exe` (**459.5 KB**).
  - Kiểm chứng chụp màn hình thật trong Session 1 (trên nền Chrome sáng lẫn Windows Terminal): Chữ, đường viền tab Chrome và nền phía sau xuyên qua trong trẻo; `SETTLED_2S_CPU_MS=0`, `WS_MB=42.28`, `PRIV_MB=20.5`, `HANDLES=368`, tự thu gọn sau 5 giây (`EXP_RGN=(22,4,441,167)` -> `AT_2_5S=(22,4,441,167)` -> `AT_5_9S=(92,4,371,43)`), `CLEAN_EXIT=True`, `LEFTOVER_PROCS=0`, `EMOJI_HITS=0`.
- **Trạng thái:** ĐÃ HOÀN TẤT NÂNG CẤP LIQUID GLASS

---

## Mục #009: Đồng bộ bảng màu Liquid Glass và điểm nhấn giao diện theo nền video/ảnh bìa

- **Thời gian thực hiện:** 2026-11-27T19:28:30+07:00
- **Phạm vi công việc:** Phối màu kính mờ và các thành phần nhấn (`timeline`, nút `Play/Pause`, sóng nhạc, nguồn phát) tự động đồng bộ theo sắc màu thực tế của video/bài hát đang phát.
- **Các thay đổi đã triển khai:**
  1. `src/renderer.rs`:
     - Bổ sung `build_ambient_aura_bgra` (tạo lưới màu làm mờ 16x10 từ ảnh bìa trên stack chỉ 640 byte khi đổi bài, không cấp phát heap trong vòng lặp vẽ) và `artwork_aura_bitmap`, vẽ hòa trộn bên dưới lớp bóng `glass_sheen` trong `draw_pill_shell` cho chế độ `Compact` và `Expanded`.
     - Bổ sung `boost_accent_for_readability` và `apply_media_theme_colors` cập nhật màu `brushes.accent` / `brushes.accent_soft` bằng `SetColor` mỗi khi đổi thumbnail hoặc khôi phục về mặc định khi dừng phát.
     - Thêm kiểm thử `artwork_aura_and_accent_match_video_colors` cùng kịch bản cập nhật/xóa `artwork_aura_bitmap` trong `winrt_glass_backdrop_initializes_and_renders`.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test -- --nocapture`: **20/20** test đạt.
  - `cargo build --release`: Tạo `target\release\win-dynamic-island.exe` (**463.5 KB**).
  - Kiểm chứng chạy thật và chụp màn hình trong Session 1: Nền kính mang sắc xanh dịu khớp hoàn toàn với thumbnail video, `timeline` và nút trung tâm đổi màu đồng bộ, nền terminal/hình nền vẫn khúc xạ xuyên qua rõ nét; `EXP_RGN=(22,4,441,167)` -> tự thu sau 5 giây `AFTER_5S_RGN=(92,4,371,43)`, `SETTLED_2S_CPU_MS=0`, `WS_MB=50.05`, `PRIV_MB=25.88`, `HANDLES=369`, `CLEAN_EXIT=True`, `LEFTOVER_PROCS=0`.
- **Trạng thái:** ĐÃ HOÀN TẤT ĐỒNG BỘ MÀU NỀN THEO VIDEO

---

## Mục #010: Nâng độ sâu màu, độ bão hòa và tương phản điều khiển trên nền sáng

- **Thời gian thực hiện:** 2026-11-27T19:36:00+07:00
- **Phạm vi công việc:** Khắc phục tình trạng màu kính video bị dìm nhạt khi hiển thị đè lên nền trắng (Chrome/YouTube Light).
- **Các thay đổi đã triển khai:**
  1. `src/renderer.rs`:
     - Tăng độ sâu lớp nền tối mờ `glass_fill` (`alpha 0.24 -> 0.30 -> 0.36`, vẽ ở `opacity 1.0` khi có `artwork_aura_bitmap`) để nền trắng phía dưới không làm nhạt sắc màu video.
     - Nâng hệ số bão hòa lưới màu `build_ambient_aura_bgra` lên `1.68` và độ phủ aura lên `0.62`.
     - Trong `boost_accent_for_readability`, pha thêm ánh sáng trắng dịu (`~34%`) sau khi đẩy sáng màu rực để thanh timeline và nhãn nguồn phát luôn nổi bật trên nền màu đậm; thêm nét viền tương phản mảnh `glass_edge_shadow` (`alpha 0.16`) và tối nhẹ nền nút `surface_card` / `switch_off` / `accent_soft`.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test -- --nocapture`: **20/20** test đạt (`WinRT Glass backdrop active: true | Blur effect active: true | GPU: AMD Radeon(TM) Graphics`).
  - `cargo build --release`: Tạo `target\release\win-dynamic-island.exe` (**464.0 KB**).
  - Kiểm chứng chạy thật trong Session 1: Viên thuốc giữ sắc màu đậm và nút điều khiển nổi rõ, `EXP_RGN=(22,4,441,167)` -> tự thu gọn sau 5 giây `AFTER_5S=(92,4,371,43)`, `SETTLED_2S_CPU_MS=0`, `WS_MB=42.66`, `PRIV_MB=22.62`, `HANDLES=368`, `CLEAN_EXIT=True`, `LEFTOVER_PROCS=0`.
- **Trạng thái:** ĐÃ HOÀN TẤT TỐI ƯU HIỂN THỊ NỀN SÁNG

---

## Mục #011: Đóng gói Portable không phụ thuộc `VCRUNTIME140.dll`, chống mở trùng và WARP fallback

- **Thời gian thực hiện:** 2026-11-27T19:55:30+07:00
- **Phạm vi công việc:** Đóng gói bản phân phối độc lập dễ chia sẻ qua mạng (`dist\WinDynamicIsland-win64.zip`) chạy ngay trên máy Windows 11 sạch không cần Visual C++ Redistributable.
- **Các thay đổi đã triển khai:**
  1. `.cargo/config.toml`: Bật `rustflags = ["-C", "target-feature=+crt-static"]` cho `x86_64-pc-windows-msvc`.
  2. `src/main.rs` & `src/window.rs`: Bổ sung `SingleInstanceGuard` (`CreateMutexW` `Local\WinDynamicIslandSingleInstanceMutex`) và `WM_APP_WAKE_INSTANCE` (`wake_from_duplicate_launch`) để bản mở thứ hai tự đánh thức bản đang chạy rồi thoát êm với mã `0`; thêm unit test `single_instance_mutex_blocks_duplicate_process`.
  3. `src/renderer.rs`: Bổ sung fallback cuối `D3D_DRIVER_TYPE_WARP` trong `create_low_power_d3d11_device` cho môi trường máy ảo/RDP/thiếu driver GPU.
  4. `dist\WinDynamicIsland-win64.zip`: Đóng gói `win-dynamic-island.exe` kèm `HUONG_DAN_SU_DUNG.txt`.
- **Kết quả kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt (0 cảnh báo).
  - `cargo test -- --nocapture`: **21/21** test đạt.
  - `cargo build --release`: Tạo `target\release\win-dynamic-island.exe` (**566.5 KB**) và `dist\WinDynamicIsland-win64.zip` (**259.4 KB**).
  - Kiểm tra bảng PE import của `.exe`: `VCRUNTIME_DEP=False` (chỉ còn các DLL gốc có sẵn của hệ điều hành Windows).
  - Kiểm thử chạy thật 2 tiến trình cùng lúc: `DUP_EXITED=True`, `DUP_CODE=0`, `PROCS_DURING_DUP=1`, `SETTLED_2S_CPU_MS=0`, `WS_MB=42.48`, `PRIV_MB=21.82`, `HANDLES=364`, `CLEAN_EXIT=True`, `LEFTOVER=0`.
- **Trạng thái:** ĐÃ HOÀN TẤT ĐÓNG GÓI PORTABLE SẴN SÀNG CHIA SẺ

### Mục #012 — Tự động cập nhật qua GitHub Releases (WinHTTP + SHA-256), CI/CD Release Workflow & Đẩy mã nguồn
- **Phạm vi thực hiện:**
  - Viết đặc tả kỹ thuật [specs/auto-update/spec.md](specs/auto-update/spec.md) cho luồng tự cập nhật an toàn trên Windows bằng Win32 `WinHTTP` thuần không kéo crate HTTP/TLS ngoài (`Win32_Networking_WinHttp` trong `Cargo.toml`).
  - Tạo [src/updater.rs](src/updater.rs): kiểm tra `releases/latest` của `kha0305/dynamic-island-win` trên nền (sau 10 giây kể từ lúc khởi động hoặc khi chọn từ khay hệ thống), xác thực URL HTTPS thuộc `github.com` / `objects.githubusercontent.com` / `release-assets.githubusercontent.com`, cài đặt hàm băm SHA-256 thuần Rust kiểm tra khớp `.exe.sha256`, đổi tên hoán đổi an toàn (`.old` / `.new`), khởi chạy lại với cờ `--post-update` và dọn tệp cũ sau khi tiến trình cũ thoát.
  - Cập nhật [src/tray.rs](src/tray.rs) (`IDM_CHECK_UPDATE`), [src/window.rs](src/window.rs) (`TIMER_STARTUP_UPDATE_ID`, `WM_APP_UPDATE_EVENT`, tách `show_deferred_update_modal` ra ngoài borrow `&mut WindowController` để tránh reentrancy `MessageBoxW`), và [src/main.rs](src/main.rs) (vòng chờ mutex `--post-update` 40x100ms + dọn `.old`).
  - Tạo [.github/workflows/release.yml](.github/workflows/release.yml) tự động biên dịch bản Static CRT, kiểm tra Clippy + test (`--locked`), tạo tệp `.sha256` và gói `WinDynamicIsland-win64.zip` lên GitHub Releases mỗi khi đẩy thẻ `v*`, đồng thời cập nhật [README.md](README.md).
- **Kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: 0 cảnh báo, 0 lỗi (sau khi rút gọn `UpdaterEvent::DownloadReadyToRestart` và chuyển vòng lặp SHA-256 sang `as_chunks`).
  - `cargo test -- --nocapture`: 25/25 test đạt (`0.21s`), gồm 4 unit test mới cho `updater` (semver, parser JSON release, SHA-256 vector chuẩn NIST, lọc host HTTPS tin cậy).
  - Bản phát hành `target\release\win-dynamic-island.exe`: `741,888 bytes` (`724.5 KB`), kiểm tra PE Imports xác nhận `VCRUNTIME_DEP=False`, `WINHTTP_DEP=True`.
  - Chạy thực tế `.exe` qua 12 giây: mở rộng khi nhấn (`419x163`), tự thu gọn sau 5 giây về `279x39`, tiến trình thứ 2 tự thoát với `DUP_CODE=0`, hoàn tất kiểm tra cập nhật nền không treo modal, `SETTLED_2S_CPU_MS=0`, `WS_MB=47.7`, `PRIV_MB=23.71`, `HANDLES=506`, `NV_MODULES=0`, thoát sạch `CLEAN_EXIT=True` (`EXIT_CODE=0`), `LEFTOVER_PROCS=0`, `EMOJI_HITS=0`.
- **Trạng thái:** HOÀN TẤT TỰ ĐỘNG CẬP NHẬT & ĐÓNG GÓI MỚI NHẤT

---

## Chốt bàn giao cuối phiên (Session Handoff)
- **Trạng thái hiện tại:** Đã hoàn thành toàn bộ Phase 1–7 cho bản **v0.1.0** đầy đủ: Liquid Glass đồng màu video, tự thu gọn 5 giây, Single-Instance, Static-CRT Portable (`VCRUNTIME_DEP=False`), Tự động cập nhật GitHub Releases (`WinHTTP` + `SHA-256`) và CI Release Workflow [.github/workflows/release.yml](.github/workflows/release.yml).
- **Hạng mục cho giai đoạn tiếp theo (v0.2 trong `spec.md`):** Khi người dùng có nhu cầu mở rộng, sẽ tiếp tục lập spec/plan cho HUD âm lượng/pin/CapsLock, File Shelf kéo thả tệp tạm và Pomodoro mini.
