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

### Mục #013 — Cải tiến kéo trượt Timeline mượt và chống thu nhỏ nhầm (Lỗi #008)
- **Nguyên nhân:** Chỉ bắt sự kiện `WM_LBUTTONUP` không có `SetCapture`, vùng chạm timeline hẹp (`20 DIP`), và nhấn/nhả hụt bất kỳ khoảng trống nào trong `Expanded` đều tự chuyển về `Compact/Slim`.
- **Thay đổi:** Mở rộng vùng chạm lên `29 DIP` + `+-14 DIP` hai đầu (`src/state.rs`), cập nhật thanh tua + thời gian tức thời + halo nút kéo khi kéo (`src/renderer.rs`), triển khai `WM_LBUTTONDOWN -> SetCapture -> WM_MOUSEMOVE -> WM_LBUTTONUP -> ReleaseCapture` cùng `WM_MOUSEACTIVATE (MA_NOACTIVATE)` và `should_collapse_expanded_on_blank_click` (`src/window.rs`).
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch, `cargo test --locked` đạt `25/25` (`0.14s`), `cargo build --release --locked` tạo `.exe` `747,008 bytes` / `ZIP` `335,459 bytes`; chạy thử `.exe` xác nhận giữ nguyên `419` khi kéo vượt biên hoặc bấm trượt quanh vùng nút/thời gian, thu gọn về `279` khi bấm vùng tiêu đề, `NV_MODULES=0`, `CLEAN_EXIT=True`. Nâng phiên bản lên `v0.1.1` để đẩy GitHub Release tự động cập nhật.

### Mục #014 — Chế độ Notch bám mép trên `y = 0` không vướng tab (`tab-safe-notch`)
- **Đặc tả & Kế hoạch:** [specs/tab-safe-notch/spec.md](specs/tab-safe-notch/spec.md), [specs/tab-safe-notch/plan.md](specs/tab-safe-notch/plan.md), [specs/tab-safe-notch/tasks.md](specs/tab-safe-notch/tasks.md).
- **Thay đổi:**
  - `src/state.rs`: Bổ sung `top: SpringScalar` vào `IslandAnimator`, đặt `Slim` thành `152 x 16 DIP` ở `top = 0.0` (và `Compact` ở `top = 2.0` để giữ vùng chạm `y = 0`), thêm `AppConfig::notch_collapse_after_4s` (mặc định `true`), `UiAction::ToggleNotchCollapse`, và chia đôi hàng 3 `Settings` (`Gọn mép 4s` | `Màn hình`).
  - `src/config.rs` & `src/tray.rs`: Lưu/đọc khóa `notch_collapse_after_4s` và thêm `IDM_TOGGLE_NOTCH_COLLAPSE` (`Thu gọn mép trên sau 4 giây`) vào menu System Tray.
  - `src/renderer.rs`: Thiết kế lại `draw_slim` (chấm ảnh bìa `10x10` + vạch tiến độ mảnh `2.4 DIP` + 3 vạch sóng nhạc thu gọn) qua `draw_visualizer_bars_sized` dùng chung và vẽ tile `Gọn mép 4s` trong `Settings`.
  - `src/window.rs`: Thêm `TIMER_COMPACT_TO_NOTCH_ID` (4 giây) bung `Compact` 4s khi có bài mới rồi thu về `Slim` Notch, hủy timer khi rê chuột chạm dải Notch và tiếp tục giữ `0%` CPU khi tạm dừng.
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch, `cargo test --locked` đạt `26/26` (`0.13s`), `cargo build --release --locked` biên dịch [target/release/win-dynamic-island.exe](target/release/win-dynamic-island.exe) (`753,152 bytes` trước khi nâng tag `0.1.2`), `EMOJI_HITS=0`; nâng phiên bản `v0.1.2` và đẩy GitHub Releases (giữ nguyên `dist\win-dynamic-island.exe` `v0.1.1` theo yêu cầu người dùng để thử tự cập nhật).

### Mục #015 — Cân bằng kính khói tối chống lóa trên nền cửa sổ trắng sáng (Lỗi #009)
- **Nguyên nhân:** Khi mở `Settings` hoặc ở chế độ không dùng aura video (`Slim`/chờ), `draw_pill_shell` hạ `glass_fill` xuống `0.17..0.26` trong khi `glass_sheen` phủ `0.34` trắng và `ambient_glow_brush` giữ mức phát sáng media trên nền trắng `#FFFFFF`, khiến kính bị bạc lóa và làm nhạt chữ/thẻ cài đặt/`Thoát ứng dụng`.
- **Thay đổi trong [src/renderer.rs](src/renderer.rs):**
  - Cân chỉnh `glass_fill` thành kính khói tối `0.62..0.72` (giữ đầy đủ `opacity` khi `!has_video_aura`, `opacity * 0.78` khi có aura kết hợp hạ trần sáng aura `0.78 -> 0.64`).
  - Giảm lớp phủ trắng `glass_sheen` (`0.34 -> 0.11`), dịu viền `glass_border`/`glass_inner_rim`, tăng `glass_edge_shadow` (`0.34`, dày `2.0 DIP`) để tạo ranh giới rõ trên nền trắng, và giảm `ambient_glow_brush` còn `0.24` khi không hiện aura video.
  - Làm đầm thẻ `surface_card` (`0.50` kèm viền `card_border`), rãnh `switch_off` (`0.54`) và nút `danger_bg` đỏ trầm (`0.58, 0.10, 0.13, 0.64`).
  - Bổ sung unit test `smoked_glass_preserves_translucency_and_contrast_over_white` đọc lại pixel trực tiếp từ GPU D2D (`CPU_READ`).
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch, `cargo test --locked` đạt `27/27` (`0.19s`, đọc pixel giả lập trên `#FFFFFF` cho `glass a=0.651` với độ tương phản chữ trắng `5.85:1`, `card=0.239..0.271`, `exit red=0.486 vs green=0.176`), `cargo build --release --locked` biên dịch `target/release/win-dynamic-island.exe` (`753,152 bytes`, nâng phiên bản lên `v0.1.3`), đồng bộ `dist\WinDynamicIsland-win64.zip` (`336,814 bytes`), `EMOJI_HITS=0`.

### Mục #016 — HUD Âm lượng (WASAPI + Kéo/Cuộn chuột) và HUD Pin / Sạc (`system-hud`)
- **Đặc tả & Kế hoạch:** [specs/system-hud/spec.md](specs/system-hud/spec.md), [specs/system-hud/plan.md](specs/system-hud/plan.md), [specs/system-hud/tasks.md](specs/system-hud/tasks.md).
- **Thay đổi:**
  - `Cargo.toml`: Bổ sung các feature Win32 gốc `Win32_Media_Audio`, `Win32_Media_Audio_Endpoints`, `Win32_System_Power`, `Win32_System_Com_StructuredStorage`, `Win32_System_Variant` (không thêm crate ngoài).
  - `src/hud.rs` *(tạo mới)*: Luồng COM MTA `AudioMonitor` đăng ký `IAudioEndpointVolumeCallback` + `IMMNotificationClient` (tự chuyển khi đổi thiết bị phát mặc định, gắn `APP_VOLUME_EVENT_CONTEXT` chống dội lặp echo), `PowerStateTracker` đọc `GetSystemPowerStatus` không spam từng %, và `WheelVolumeAccumulator` hỗ trợ cả chuột nấc `120` lẫn touchpad mịn.
  - `src/state.rs`, `src/config.rs`, `src/tray.rs`: Bổ sung `HudContent`, `effective_mode` ưu tiên bung HUD `Compact` `2.0s` khi ở `Slim`/`Tự ẩn khi rảnh` (giữ yên `Expanded`/`Settings`/`Fullscreen`), 3 công tắc cấu hình độc lập `HUD Âm lượng` (`hud_volume_enabled`), `Cuộn chỉnh loa` (`hud_wheel_enabled`), `HUD Pin / Sạc` (`hud_power_enabled`), hitbox `ToggleVolumeMute` + `SeekVolume`, và lưới `Settings` 5 hàng `424 x 298 DIP` (`MAX_SURFACE_HEIGHT_DIP = 336.0`).
  - `src/renderer.rs`: Vẽ icon Loa/Pin vector bằng Direct2D sắc nét, thanh kéo âm lượng, trạng thái sạc/pin yếu và 3 công tắc mới trong `Settings`.
  - `src/window.rs` & `src/main.rs`: Xử lý `WM_APP_HUD_EVENT` (`WM_APP + 7`), `RegisterPowerSettingNotification`/`WM_POWERBROADCAST`, `WM_MOUSEWHEEL` (`ScreenToClient` + bỏ qua khi giữ `Ctrl`/`Shift`), kéo thanh âm lượng qua `SetCapture`/`ReleaseCapture` ngoài `&mut WindowController`, và `TIMER_HUD_EXPIRE_ID` (`2000 ms`).
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch, `cargo test --locked` đạt `33/33` (`0.27s`, gồm unit test tích lũy touchpad/pin và test Win32 HWND thật với `SPI_GETMOUSEWHEELROUTING=2`), `cargo build --release --locked` biên dịch xong; chạy kiểm thử thực tế trên `.exe` ghi nhận `WHEEL_VOL_BEFORE=0.40 -> AFTER=0.44` (`FG_NOACTIVATE=True`), `AFTER_EXPIRE=155x17`, `SETTINGS_RGN=427x301`, sau khi tắt `Cuộn chỉnh loa` trong UI `WHEEL_DISABLED_BEFORE=0.44 AFTER=0.44`, đổi loa ngoài tự bật `EXTERNAL_VOL_HUD=279x39`, `VCRUNTIME_DEP=False`, `NV_MODULES=0`, `CLEAN_EXIT=True`.

### Mục #017 — Tối ưu độ mượt 144Hz theo `DwmFlush` và khắc phục kẹt trạng thái tương tác (Lỗi #010)
- **Nguyên nhân gốc rễ đã đo đạc:**
  - `VSyncTicker::find_output_for_monitor` trả về `None` khi `EnumOutputs(0)` trên iGPU không có output trực tiếp, khiến `TickerMode::VSync` rơi xuống `thread::sleep(8ms)` (làm tròn ~15.6 ms lệch pha màn hình 144Hz); kết hợp `apply_target_and_render` bỏ qua frame 1 và `SetTimer(12ms)` khi kéo slider gây sượng khung hình.
  - Khung đầu vẽ `Expanded` mất `58.42 ms` do khởi tạo DirectWrite lần đầu và dựng lại `CreateRoundedRectangleGeometry` + `PushLayer/PopLayer` + `CreateTextLayout` mỗi khung hình.
  - Gọi `SetWindowRgn` GDI mỗi khung hình đang animation làm cắt mép và dễ làm rớt `WM_MOUSELEAVE`, dẫn tới `mouse_tracked` kẹt `true`; đồng thời thiếu `pressed_action`, `Settings` dễ đóng nhầm khi bấm trượt khe nút, và `TimelinePropertiesChanged` trong `src/media.rs` đặt `check_artwork: true` làm đọc/băm lại ảnh bìa liên tục.
- **Thay đổi:**
  - `src/vsync.rs`: Thay `WaitForVBlank` + `sleep(8ms)` bằng `DwmFlush()` (đo thực tế trên máy `~6.4–7.9 ms`, khớp 144Hz) kèm fallback `7ms` nếu DWM không chặn; xóa `tick_pending` khi chuyển sang `VSync` và tăng `VisualizerSlow` lên `50 ms`.
  - `src/state.rs`: Bỏ `content_alpha.snap(0.2)`, chỉnh lò xo `top` tắt dần tới hạn (`380.0 / 39.0`) không vọt âm đập mép trên, tinh chỉnh độ nảy kích thước (`380.0 / 34.0`) và ngưỡng `is_settled` (`0.12 / 0.45`).
  - `src/renderer.rs`: Thay `PushLayer` của aura/artwork bằng `ID2D1BitmapBrush` (`create_clamped_bitmap_brush`), cache `IDWriteTextLayout` (32 mục) + `warm_up_text_formats()`, và bỏ `PushLayer` tổng khi nội dung đã ổn định đúng kích thước.
  - `src/window.rs`: Vẽ ngay khung hình đầu khi bắt đầu animation; chạy kéo slider theo `TickerMode::VSync` (bỏ `TIMER_DEFER_RENDER_ID`); đặt `SetWindowRgn` vùng bao khi đang mở rộng và đặt khít khi settle (`update_window_region`); thêm `pressed_action`, `sync_pointer_presence` chống kẹt `mouse_tracked`, `reset_pointer_and_drag_state` khi ẩn/fullscreen, chặn bung nhầm vùng đệm ngoài `pill`, không đóng `Settings` khi bấm trượt khe nút và giữ `Compact` nếu chuột đang nằm trên viên thuốc lúc HUD tắt.
  - `src/media.rs` & `src/hud.rs`: Đổi `TimelinePropertiesChanged` sang `check_artwork: false`; sửa lỗi borrow `E0503` ở `run_audio_worker` và cho phép đồng bộ sự kiện âm lượng bên ngoài (`!is_self_event`) kể cả khi trùng batch.
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch (0 cảnh báo), `cargo test --locked` đạt `33/33` (`0.19s`, gồm kịch bản bấm trượt nút trong `Settings`, di chuột vùng đệm `Slim` và `clear_hud`), `cargo build --release --locked` tạo `target/release/win-dynamic-island.exe` (`838,144 bytes`); chạy kiểm thử `.exe` release ghi nhận `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `CPU_2S_MS=0`, `WS_MB=44.55`, `PRIV_MB=24.15`, `HANDLES=408`, 0 module GPU NVIDIA (chỉ có `USERENV.dll` hệ thống khớp chuỗi `nv`), `LEFTOVER_PROCS=0`.

### Mục #018 — Xử lý khựng khi cắm/rút sạc (ACPI async + `POWERBROADCAST_SETTING`) và cố định bố cục khi co giãn (Lỗi #011)
- **Nguyên nhân:** `WM_POWERBROADCAST` gọi đồng bộ `GetSystemPowerStatus()` trên UI thread (lần đọc ACPI EC đầu đo được `6.93 ms`, dễ khựng lúc laptop đổi nguồn AC/DC và phát `WM_DISPLAYCHANGE`); `IslandAnimator::set_target_mode` đặt `last_step = Some(Instant::now())` làm frame đầu có `dt ~ 0`; `DwmFlush()` dưới `2 ms` khi mới thức dậy nhảy vào `sleep(7ms)`; và các layout `Compact`/`Expanded`/`Settings` tính chiều rộng theo `pill.width` đang co giãn khiến chữ nhảy cắt `...` và trượt cache `IDWriteTextLayout` ở mọi khung hình.
- **Thay đổi:**
  - `src/hud.rs` & `src/window.rs`: Đọc trạng thái AC/DC tức thì từ `POWERBROADCAST_SETTING` (`PBT_POWERSETTINGCHANGE`) qua `evaluate_ac_hint`, đẩy đọc `GetSystemPowerStatus()` sang luồng nền (`PowerSync` / `AudioCommand::RefreshPower`), và lưu `last_window_rect` để không gọi `SetWindowPos` thừa khi `WM_DISPLAYCHANGE` không đổi kích thước/vị trí.
  - `src/state.rs`: Bỏ `self.last_step = Some(Instant::now())` trong `set_target_mode` để frame đầu bước ngay `1/144s`; nâng giới hạn `clamped_dt` lên `0.08s`.
  - `src/vsync.rs`: Chỉ fallback `sleep(7ms)` khi `!flushed` hoặc `DwmFlush()` trả về `< 400 us` 3 lần liên tiếp (`fast_flush_streak`).
  - `src/renderer.rs`: Thêm `virtual_pill_anchored` neo cố định kích thước nội dung trong `draw_compact`, `draw_compact_hud`, `draw_expanded`, `draw_settings` (kết hợp `need_clip` của `draw_clipped_contents`) và pre-warm các chuỗi `"Đang cắm sạc"`, `"Đang dùng pin"`, `"Pin yếu"`, `"100%"` trong `warm_up_text_formats`.
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch, `cargo test --locked` đạt `33/33` (`0.23s`, bổ sung kiểm tra `evaluate_ac_hint` và `PowerSync` trong `wasapi_audio_monitor_connects_and_shuts_down_cleanly`), `cargo build --release --locked` tạo `target/release/win-dynamic-island.exe` (`841,216 bytes`), chạy thử bản release ghi nhận `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `CPU_2S_MS=0`, `WS_MB=43.9`, `PRIV_MB=19.34`, `HANDLES=404`, `LEFTOVER_PROCS=0`.

### Mục #019 — Đồng bộ nguyên tử WinRT Glass Surface và sửa 4 lỗi hoạt ảnh 144Hz (Lỗi #012)
- **Nguyên nhân:**
  - `WinRtBackdropLayer` (`SpriteVisual`) được DWM commit qua cây `Windows::UI::Composition` trong khi nội dung Direct2D trước đây vẽ lên `IDXGISwapChain1::Present(0, 0)` độc lập, gây lệch nhịp khiến lớp kính mờ tách khỏi viền/chữ khi co giãn nhanh ở 144Hz.
  - `virtual_pill_anchored(..., center_y = true)` đẩy nội dung `Compact` tụt xuống giữa khung lớn khi đang thu từ `Expanded`/`Settings` về `Compact`.
  - `update_window_region` có thể giữ vùng bo tròn cũ hoặc đặt ranh giới thiếu `diam == 0` lúc đang animate làm cắt góc bo tròn; đồng thời `handle_mouse_move` kiểm tra `!pill_contains_dip` trên cả chế độ đã bung dễ gây dao động/kẹt hover ở mép ranh giới.
- **Thay đổi:**
  - `Cargo.toml`: Bổ sung feature `"Graphics_DirectX"` cho crate `windows` (`0.62.2`).
  - `src/renderer.rs`: Thay `IDXGISwapChain1` ở nhánh `CompositionHost::WinRtGlass` bằng `CompositionGraphicsDevice` + `CompositionDrawingSurface` + `ICompositionDrawingSurfaceInterop` (`BeginDraw` / `EndDraw`, giữ `IDXGISwapChain1` cho `DirectCompFallback`) và truyền `DrawCanvas<'_>` suốt toàn bộ hàm vẽ để nội dung D2D và `WinRtBackdropLayer` commit cùng một nhịp nguyên tử trong cây Composition; bổ sung `mode_content_reveal_alpha`, kẹp độ lệch dọc `virtual_pill_anchored` về `[-10.0, 0.0]`, và gỡ tham số thừa `dpi_scale` ở `CompositionHost::new` (`clippy::too_many_arguments`).
  - `src/window.rs`: Tách `apply_rect_region` (`diameter == 0`) và `apply_round_region` trong `update_window_region` để giữ vùng chữ nhật bao trùm suốt lúc `is_animating()`, chỉ cắt `CreateRoundRectRgn` khít khi lò xo dừng hẳn; giới hạn nhánh chặn biên `pill_contains_dip(..., 2.0)` trong `handle_mouse_move` chỉ áp dụng khi `requested_mode == IslandMode::Slim`.
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch (0 cảnh báo), `cargo test --locked -- --nocapture` đạt `33/33` (`0.27s`, `WinRT Glass backdrop active: true | Blur effect active: true | GPU: AMD Radeon(TM) Graphics`), `cargo build --release --locked` tạo `target/release/win-dynamic-island.exe` (`842,240 bytes`), đo kiểm bản release ghi nhận `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `CPU_2S_MS=0`, `WS_MB=47.02`, `PRIV_MB=19.09`, `HANDLES=397`, `NV_MODULES=0`, `AMD_MODULES=4`, `LEFTOVER_PROCS=0`.

### Mục #020 — Tính năng Chuyển nhanh thiết bị âm thanh (`audio-switcher`) và cập nhật quy tắc `GEMINI.md`
- **Đặc tả & Kế hoạch:** [specs/audio-switcher/spec.md](specs/audio-switcher/spec.md), [specs/audio-switcher/plan.md](specs/audio-switcher/plan.md), [specs/audio-switcher/tasks.md](specs/audio-switcher/tasks.md).
- **Thay đổi:**
  - `GEMINI.md`: Bổ sung quy tắc đồng bộ `CompositionDrawingSurface` + `CreateRoundRectRgn` vào mục 5 theo xác nhận của người dùng.
  - `Cargo.toml`: Thêm 2 feature Win32 gốc `"Win32_Devices_FunctionDiscovery"` và `"Win32_UI_Shell_PropertiesSystem"` (không thêm crate ngoài).
  - `src/hud.rs`: Thêm `AudioOutputDevice`, liệt kê thiết bị `eRender` đang hoạt động qua `IMMDeviceEnumerator::EnumAudioEndpoints` + `PKEY_Device_FriendlyName` kèm RAII `CoTaskMemPwstr` / `PropVarGuard`, lắng nghe `OnDeviceAdded`/`OnDeviceRemoved`/`OnDeviceStateChanged`/`OnPropertyValueChanged`, và triển khai COM `policy_com::IPolicyConfig::SetDefaultEndpoint` chuyển đồng bộ cả 3 vai trò `eConsole`, `eMultimedia`, `eCommunications`.
  - `src/state.rs` & `src/renderer.rs`: Thêm `IslandMode::AudioDevices` (`424 x 284 DIP`, `MAX_AUDIO_DEVICES_VISIBLE = 5`), nút mở nhanh ở góc dưới-trái `Expanded` (`pill.left + 16.0 .. center_x - 76.0`), danh sách chọn `draw_audio_devices` kèm nhãn `"Đang dùng"` và hỗ trợ cuộn chuột nếu `> 5` thiết bị, đồng thời hiển thị tên thiết bị + % âm lượng trong `2.0s` Compact HUD (`show_device_name: true`) khi đổi thiết bị từ ngoài hoặc Tray.
  - `src/tray.rs` & `src/window.rs`: Thêm menu con `"Thiết bị phát âm thanh"` (`IDM_AUDIO_DEVICE_BASE..=IDM_AUDIO_DEVICE_MAX`), xử lý cuộn/chọn thiết bị trên `AudioDevices` và Tray, giữ `AudioDevices` cập nhật tại chỗ không tự đóng khi click khoảng trống.
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch (0 cảnh báo), `cargo test --locked -- --nocapture` đạt `35/35` (`0.22s`, phát hiện `3` thiết bị phát thực tế trên máy và gọi thành công `IPolicyConfig::SetDefaultEndpoint`, kèm `mouse_wheel_screen_coords_delta_settings_and_drag_integration` bước #11 và `audio_devices_hitboxes_stay_inside_pill_and_do_not_overlap`), `cargo build --release --locked` tạo `target/release/win-dynamic-island.exe` (`892,928 bytes`), đo kiểm bản release ghi nhận `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `CPU_2S_MS=0`, `WS_MB=44.38`, `PRIV_MB=19.09`, `HANDLES=401`, `NV_MODULES=0`, `AMD_MODULES=4`, `LEFTOVER_PROCS=0`.

### Mục #021 — Thay thế toàn bộ hộp thoại `MessageBoxW` bằng thẻ thông báo `Liquid Glass` (`island-notifications`)
- **Đặc tả & Kế hoạch:** [specs/island-notifications/spec.md](specs/island-notifications/spec.md), [specs/island-notifications/plan.md](specs/island-notifications/plan.md), [specs/island-notifications/tasks.md](specs/island-notifications/tasks.md).
- **Thay đổi:**
  - `src/state.rs`: Bổ sung `IslandMode::Notice` (`416 x 136 DIP`, `corner_radius = 26.0`, `TOP_MARGIN_DIP = 6.0`), `NoticeKind` (`Checking`, `UpToDate`, `UpdateAvailable`, `Downloading`, `Error`), `IslandNoticeView`, `UiAction::NoticePrimary` / `NoticeSecondary` và `fill_notice_hitboxes` kèm test chống chồng lấn.
  - `src/renderer.rs`: Thêm `TextFormatSlot::BodyLeft` bật `DWRITE_WORD_WRAPPING_WRAP` cho mô tả nhiều dòng không bị cắt chữ; triển khai `draw_notice` và `draw_notice_glyph` vẽ huy hiệu tròn `17.5 DIP` với icon vector Direct2D và màu phân biệt theo trạng thái (`power_ok`, `accent`, `danger_border`), nút tương tác bo tròn và mở rộng test `winrt_glass_backdrop_initializes_and_renders`.
  - `src/window.rs`: Xóa sạch toàn bộ `MessageBoxW` / `DeferredUpdateModal` / `show_deferred_update_modal`; bổ sung `ActiveNotice`, `TIMER_NOTICE_EXPIRE_ID` (`4000 ms` tự ẩn cho `UpToDate` khi không hover), tự bung nhẹ khi phát hiện bản mới lúc khởi động (nếu không fullscreen/không mở bảng cấu hình), hỗ trợ thử lại tải/kiểm tra khi lỗi và test tích hợp bước #12.
- **Kiểm chứng:** Xác nhận 0 còn lời gọi `MessageBoxW` thực thi trong `src/*.rs`, `cargo clippy --all-targets -- -D warnings` sạch (0 cảnh báo), `cargo test --locked -- --nocapture` đạt `36/36` (`0.25s`), `cargo build --release --locked` tạo `target/release/win-dynamic-island.exe` (`904,704 bytes`), đo kiểm bản release ghi nhận `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `CPU_2S_MS=0`, `WS_MB=44.39`, `PRIV_MB=19.52`, `HANDLES=401`, `NV_MODULES=0`, `AMD_MODULES=4`, `LEFTOVER_PROCS=0`.

### Mục #022 — Nâng cấp bố cục thanh thu gọn `Compact` (`compact-pill-polish`)
- **Đặc tả & Kế hoạch:** [specs/compact-pill-polish/spec.md](specs/compact-pill-polish/spec.md), [specs/compact-pill-polish/plan.md](specs/compact-pill-polish/plan.md), [specs/compact-pill-polish/tasks.md](specs/compact-pill-polish/tasks.md).
- **Thay đổi:**
  - `src/state.rs`: Nâng `IslandMode::Compact` từ `(276.0, 36.0, 18.0)` lên `(316.0, 40.0, 20.0, 1.0)` (giữ `top = 2.0 DIP`, `compact_rgn.top == 0`).
  - `src/renderer.rs`: Thêm `TextFormatSlot::CompactSub`, nâng `Compact` thành `11.8pt SemiBold`, nâng viền sáng `draw_artwork_with_rim(..., soft_rim = true)` và `draw_visualizer_bars_count(..., 5)` 5 vạch sóng hình chuông cân tâm cung tròn đầu phải `width - 20.0`, đối xứng hoàn toàn với đĩa ảnh bìa tròn đồng tâm `26 x 26` (`radius = 13.0`, tâm `left + 20.0`); bố trí 2 tầng Tiêu đề + Nghệ sĩ/Nguồn phát kèm thanh tiến độ mảnh `2.0 DIP` ở đáy; hiện biểu tượng Tạm dừng `||` hoặc chấm trạng thái khi nghỉ để không còn mảng trống bên phải, đồng thời giữ nguyên cơ chế kính hòa màu background.
  - `src/window.rs`: Bổ sung `IslandMode::Compact` vào `sync_timeline_timer` và `WM_TIMER` (`TIMER_TIMELINE_TICK_ID`) khi tắt visualizer lúc đang phát nhạc.
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch (0 cảnh báo), `cargo test --locked -- --nocapture` đạt `36/36` (`0.25s`, gồm test render `Compact` đang phát lẫn tạm dừng trong `winrt_glass_backdrop_initializes_and_renders`), `cargo build --release --locked` tạo `target/release/win-dynamic-island.exe` (`907,264 bytes`), đo kiểm bản release ghi nhận `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `CPU_2S_MS=0`, `WS_MB=44.96`, `PRIV_MB=23.48`, `HANDLES=405`, `NV_MODULES=0`, `AMD_MODULES=4`, `LEFTOVER_PROCS=0`.

### Mục #023 — Dọn mã thừa, hoàn thiện tài liệu và phát hành `v0.2.0`
- **Phạm vi thực hiện:**
  - Gỡ phương thức `IslandRenderer::dpi_scale` không sử dụng tại [src/renderer.rs](src/renderer.rs) để vượt qua `cargo clippy --all-targets -- -D warnings`.
  - Nâng phiên bản `Cargo.toml` và `Cargo.lock` từ `0.1.3` lên `0.2.0`; cập nhật [README.md](README.md), [TASK_MEMORY.md](TASK_MEMORY.md) và [NHAT_KY_HOAT_DONG.md](NHAT_KY_HOAT_DONG.md) đồng bộ với toàn bộ tính năng của `v0.2.0` (Mục #016–#022).
- **Kiểm chứng:** `cargo clippy --all-targets -- -D warnings` sạch (0 cảnh báo), `cargo test --locked -- --nocapture` đạt `36/36` (`0.40s`), `cargo build --release --locked` biên dịch `target/release/win-dynamic-island.exe` (`907,776 bytes`), giữ nguyên thư mục `dist/` cục bộ theo yêu cầu người dùng để CI `.github/workflows/release.yml` tự build và phát hành tag `v0.2.0` trên GitHub Releases.

### Mục #024 — Đẩy GitHub `v0.2.0`, đồng bộ `dist/` và dọn sạch thư mục `target/`
- **Thời gian thực hiện:** 2026-11-28T00:27:00+07:00
- **Phạm vi công việc:** Đẩy toàn bộ mã nguồn + tag phát hành `v0.2.0` lên GitHub, sau đó dọn rác build (~6.02 GB) nhưng vẫn duy trì ứng dụng đang chạy liên tục từ bản Portable trong `dist/`.
- **Kết quả thực hiện & kiểm chứng:**
  - **Git & GitHub (`origin`):** Đã commit `9097ee3` (`29` tệp thay đổi) và đẩy nhánh `main` (`48b3b9b..9097ee3`) cùng tag `v0.2.0` (`ef5fc30d605af5e45f6c1af28525229f874d40ea`) lên `https://github.com/kha0305/dynamic-island-win.git`.
  - **Đồng bộ `dist/` cục bộ:** Sao chép bản thực thi `v0.2.0` (`908,288 bytes`) sang `dist\win-dynamic-island.exe`, tái tạo `dist\win-dynamic-island.exe.sha256` và `dist\WinDynamicIsland-win64.zip` (`393,165 bytes`).
  - **Dọn sạch `target/`:** Thoát êm tiến trình đang chạy từ `target\release` (`PID 18608` thay cho `PID 8156` sau tự cập nhật), xóa toàn bộ thư mục `target/` (`6,165.97 MB`, gồm `target\debug` `4,335.05 MB` và `target\release` `1,830.92 MB`).
  - **Khởi chạy lại & dung lượng hiện tại:** Khởi chạy lại thành công từ `dist\win-dynamic-island.exe` (`PID 22228`); tổng dung lượng toàn dự án giảm xuống còn **2.33 MB** (`dist` `1.24 MB`, `.git` `0.49 MB`, `src` `0.44 MB`, `specs` `0.07 MB`).

### Mục #025 — Bản vá tinh chỉnh giao diện `v0.2.1` (`ui-polish-v021`)
- **Thời gian thực hiện:** 2026-11-28T09:46:40+07:00
- **Đặc tả & Kế hoạch:** [specs/ui-polish-v021/spec.md](specs/ui-polish-v021/spec.md), [specs/ui-polish-v021/plan.md](specs/ui-polish-v021/plan.md), [specs/ui-polish-v021/tasks.md](specs/ui-polish-v021/tasks.md).
- **Phạm vi thay đổi:**
  - `Cargo.toml` & `Cargo.lock`: Nâng phiên bản gói lên `0.2.1`.
  - [src/state.rs](src/state.rs): Nâng `MAX_SURFACE_HEIGHT_DIP = 396.0`, đồng bộ bề rộng `Expanded` (`424 x 160 DIP`) và mở rộng `Settings` (`424 x 360 DIP`), thêm `audio_devices_height_for_count` + `target_dimensions_for_state`, thêm `UiAction::CheckUpdate`, đưa `OpenSettings` xuống góc dưới phải đối xứng `OpenAudioDevices` và sắp xếp lại `fill_settings_hitboxes` theo 3 nhóm (`14` hitboxes).
  - [src/renderer.rs](src/renderer.rs): Tách `sys_accent` / `sys_accent_soft` cố định khỏi màu ảnh bìa bài hát; vẽ icon bánh răng vector (`draw_gear_icon`) và mở rộng vùng tiêu đề `Expanded` sang hết lề phải (`+70 DIP`); phân 3 nhóm rõ ràng (`Hiển thị`, `HUD & Âm thanh`, `Hệ thống`), hiển thị `v0.2.1`, thêm mục `Thiết bị phát` và `Kiểm tra cập nhật` cùng thẳng hàng cột phải cho `Settings`; co chiều cao `AudioDevices` linh hoạt theo số thiết bị (`107..293 DIP`, 3 thiết bị = `193 DIP`); tối giản `Slim` khi rảnh và đồng bộ hover knob + halo cho `Volume HUD` khi hiển thị tên thiết bị; chuẩn hóa `mode_content_reveal_alpha` theo `target_h`.
  - [src/window.rs](src/window.rs): Lưu `settings_return_mode` & `audio_devices_return_mode` để `"Quay lại"` trở về đúng màn hình nguồn, đồng bộ chiều cao động khi `DevicesChanged`, nối `UiAction::CheckUpdate` và bổ sung test tích hợp.
- **Kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt sạch (0 cảnh báo).
  - `cargo test --locked -- --nocapture`: **37/37** tests đạt trong `0.27s` (`WinRT Glass backdrop active: true | Blur effect active: true | GPU: AMD Radeon(TM) Graphics`, tương phản kính trên `#FFFFFF` đạt `5.58:1`).
  - `cargo build --release --locked`: Tạo `target\release\win-dynamic-island.exe` (`916,992 bytes`), đồng bộ sang `dist\win-dynamic-island.exe`, `dist\win-dynamic-island.exe.sha256` (`b730574494563e12998859635b3f2102b0a80cad482b331c98f7342840cae1bb`) và `dist\WinDynamicIsland-win64.zip` (`396,576 bytes`).
  - Đo kiểm runtime thực tế bản `dist\win-dynamic-island.exe` (`PID 18972`): `VCRUNTIME_DEP=False`, `WINHTTP_DEP=True`, `EMOJI_HITS=0`, `IDLE_CPU_2S_MS=0`, `WS_MB=49.34`, `PRIV_MB=26.31`, `HANDLES=527`, `NV_MODULES=0`.
  - Đã chụp ảnh kiểm chứng trực tiếp trên màn hình `1920x1080` các chế độ `Compact`, `Expanded`, `Settings` và `AudioDevices` của bản `v0.2.1`: đáy `Expanded` đối xứng, `Settings` phân 3 nhóm sắc nét không bị nhuốm đỏ theo bài hát, `AudioDevices` ôm gọn 3 thiết bị không dư khoảng trống đáy.

### Mục #026 — Sửa lỗi nút `Cài đặt` rộng và sửa lỗi `Gọn mép 4s` tắt vẫn thu về `Slim` (`v0.2.2`)
- **Thời gian thực hiện:** 2026-11-28T09:56:16+07:00
- **Phạm vi thay đổi:**
  - `Cargo.toml` & `Cargo.lock`: Nâng phiên bản lên `0.2.2`.
  - [src/state.rs](src/state.rs) & [src/renderer.rs](src/renderer.rs): Thu gọn nút `OpenSettings` ở góc dưới phải `Expanded` từ `128 DIP` xuống `94.0 DIP` (`pill.left + pill.width - 110.0 .. pill.left + pill.width - 16.0`) với lề icon/chữ cân xứng (`21.0 DIP` / `32.0..right-10.0`); cập nhật test hitbox `Expanded`.
  - [src/window.rs](src/window.rs): Thêm `should_collapse_to_slim()` và `sync_mode_after_collapse_policy_change()`, chuẩn hóa `IslandWindow::create`, `collapsed_rest_mode()`, `handle_media_update`, `clear_hud`, `handle_mouse_leave`, `handle_compact_to_notch_timer`, `handle_auto_collapse_timer`, `ToggleNotchCollapse`, `ToggleAutoHideIdle` và `IDM_TOGGLE_EXPANDED` để khi tắt `Gọn mép 4s` (và tắt `Tự ẩn khi rảnh`), viên thuốc giữ ổn định dáng `Compact` không bao giờ tự co về `Slim` kể cả khi không có nhạc; bổ sung kiểm thử bước #13.
  - [src/tray.rs](src/tray.rs): Điều chỉnh nhãn thu gọn trong menu ngữ cảnh phù hợp theo trạng thái `notch_collapse_after_4s`.
- **Kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt sạch (0 cảnh báo).
  - `cargo test --locked -- --nocapture`: **37/37** tests đạt trong `0.26s`.
  - `cargo build --release --locked`: Tạo `target\release\win-dynamic-island.exe` (`917,504 bytes`), đồng bộ sang `dist\win-dynamic-island.exe`, `dist\win-dynamic-island.exe.sha256` (`abd34e81355d0aab0115206cd019b5e8d9ba4a08bf0ccd8a4854dd264d9460ed`) và `dist\WinDynamicIsland-win64.zip` (`396,828 bytes`).
  - Đo kiểm runtime bản `dist\win-dynamic-island.exe` (`PID 7372`): `VCRUNTIME_DEP=False`, `WINHTTP_DEP=True`, `EMOJI_HITS=0`, `IDLE_CPU_2S_MS=0`, `WS_MB=48.48`, `PRIV_MB=19.34`, `HANDLES=548`, `NV_MODULES=0`.
  - Đã chụp ảnh trực tiếp kiểm chứng nút `Cài đặt` thu gọn cân đối trên `Expanded` và kiểm tra giữ nguyên `Compact` sau 6 giây rời chuột khi tắt `Gọn mép 4s`.

### Mục #027 — Lọc bỏ feature dư thừa, tối ưu Full LTO/abort, tăng cỡ chữ và thêm chấm `live` cạnh phiên bản (`v0.2.3`)
- **Thời gian thực hiện:** 2026-11-28T10:21:00+07:00
- **Phạm vi thay đổi:**
  - [Cargo.toml](Cargo.toml) & [Cargo.lock](Cargo.lock): Nâng phiên bản lên `0.2.3`; lọc và gỡ bỏ 2 feature của crate `windows` không sử dụng (`Foundation_Collections` và `Foundation_Numerics`), giữ đúng các feature thực dùng qua kiểm chứng compiler (`windows-core`, `Win32_Security`, `Win32_System_Variant`, `Win32_UI_Shell_PropertiesSystem` đều được sử dụng trực tiếp/gián tiếp); nâng `[profile.release]` lên `lto = true` (Full LTO) và `panic = "abort"`.
  - [src/renderer.rs](src/renderer.rs): Gỡ nhãn phiên bản khỏi thanh tiêu đề `Settings`, đưa `v{APP_VERSION}` vào ngay thẻ `Kiểm tra cập nhật` (`UiAction::CheckUpdate`, khi có bản mới hiện `"Có bản mới"` + tag màu `sys_accent`); tách helper `get_or_create_text_layout` và `measure_text_width` (qua `DWRITE_TEXT_METRICS`, dùng chung cache `IDWriteTextLayout`) để vẽ chấm trạng thái `live` hai lớp tĩnh (`power_ok` xanh lục, hoặc `sys_accent` khi có bản cập nhật; `0% CPU` chờ) sát cạnh chuỗi phiên bản; nâng cỡ chữ toàn hệ thống `TextFormats` (`title 14.2`, `subtitle/body_left 12.0`, `compact/button/button_center 12.2`, `small* 11.2`, `compact_sub 10.8`) và bổ sung kiểm tra điểm ảnh chấm `live` trong test Direct2D offscreen `smoked_glass_preserves_translucency_and_contrast_over_white`.
- **Kiểm chứng thực tế:**
  - `cargo clippy --all-targets -- -D warnings`: Đạt sạch (0 cảnh báo).
  - `cargo test --locked -- --nocapture`: **37/37** tests đạt trong `0.24s` (`WinRT Glass backdrop active: true | Blur effect active: true | GPU: AMD Radeon(TM) Graphics`, tương phản kính trên `#FFFFFF` đạt `5.58:1`, chấm `live` đo được `rgb=(0.36, 0.90, 0.56)` với `ver_w=29.7 DIP`).
  - `cargo build --release --locked`: Dung lượng [target\release\win-dynamic-island.exe](target/release/win-dynamic-island.exe) giảm từ `917,504 bytes` (`v0.2.2`) xuống còn **`714,240 bytes`** (`697.5 KB`, giảm **`203,264 bytes` / `198.5 KB` / `22.2%`**), đồng bộ sang `dist\win-dynamic-island.exe`, `dist\win-dynamic-island.exe.sha256` (`d4e7335b1f0e9750c6374a946c49950f4810ed4abf34a4ec7d28396415d0c232`) và `dist\WinDynamicIsland-win64.zip` (`345,120 bytes`).
  - Kiểm tra PE Imports & ký tự: `VCRUNTIME_DEP=False`, `WINHTTP_DEP=True`, `EMOJI_HITS=0`, không có DLL thừa.
  - Đo kiểm runtime bản `dist\win-dynamic-island.exe` (`PID 5944` chạy nền trên Desktop): sau khi ổn định qua nhịp co giãn khởi động, `SETTLED_IDLE_CPU_2S_MS=0`, `WS_MB=43.69`, `PRIV_MB=18.48`, `HANDLES=400`, `0` module NVIDIA (`USERENV.dll` trùng chuỗi con `nv`), `AMD_MODULES=4`; ảnh chụp thật `1920x1080` xác nhận chấm `live` xanh lục hiển thị sắc nét cạnh `v0.2.3` không cắt chữ.

### Mục #028 — Hoàn tất 3 chặng `v0.3.0`: Tối ưu tách module, Theo dõi Anti (10 trạng thái) & File Shelf cạnh Dynamic trên FHD
- **Thời gian thực hiện:** 2026-11-28T11:12:00+07:00
- **Đặc tả & Kế hoạch:** [specs/v03-shelf-anti-refactor/spec.md](specs/v03-shelf-anti-refactor/spec.md), [specs/v03-shelf-anti-refactor/plan.md](specs/v03-shelf-anti-refactor/plan.md), [specs/v03-shelf-anti-refactor/tasks.md](specs/v03-shelf-anti-refactor/tasks.md).
- **Chặng 1 — Tối ưu và tách nhỏ toàn bộ module:**
  - Tách các tệp đơn khối thành cấu trúc theo vai trò: [src/json_mini.rs](src/json_mini.rs) (trích xuất bộ parser JSON không phụ thuộc + `sha256_hex` khỏi [src/updater.rs](src/updater.rs)), `src/hud/{mod,policy_config,power,tests}.rs`, `src/state/{mod,animator,hitboxes,tests}.rs`, `src/renderer/{mod,compositor,theme,media_views,expanded_view,hud_views,settings_views,anti_view,shelf_view,tests}.rs`, và `src/window/{mod,modes,input,workers,tests}.rs`; gỡ 8 feature bắc cầu dư thừa trong [Cargo.toml](Cargo.toml).
- **Chặng 2 — Theo dõi Anti (`agy CLI`, `Anti IDE`, `Anti app`):**
  - [src/anti.rs](src/anti.rs) & [src/main.rs](src/main.rs): Định nghĩa đầy đủ 10 trạng thái (`Đang đọc dự án`, `Đang phân tích`, `Đang code`, `Đang test`, `Đang build`, `Đang chờ duyệt`, `Đang sửa lỗi`, `Bị lỗi`, `Đã hủy`, `Đã xong`) dùng chung 1 tệp JSON cục bộ `%LOCALAPPDATA%\WinDynamicIsland\anti-progress.json` ghi atomic; tích hợp cổng `--anti-hook <EVENT>`, bộ cài/gỡ an toàn vào `~/.gemini/config/hooks.json` (khóa `"win-dynamic-island-status"` + backup `.json.bak`, không dùng `PreToolUse` để không can thiệp quyền duyệt lệnh), suy luận tool đang chạy qua đuôi `transcript.jsonl`, và theo dõi sự kiện Win32 `ReadDirectoryChangesW` (0% CPU khi rảnh).
  - [src/watcher.rs](src/watcher.rs), [src/renderer/anti_view.rs](src/renderer/anti_view.rs) & `Settings`: Phát hiện foreground là `agy` / `Antigravity` để chỉ hiện tiến độ Anti khi người dùng chuyển sang app khác; ưu tiên Anti khi đang chạy song song media; vòng xoay 6 chấm mượt, tự giữ trạng thái kết thúc 4 giây; thêm nút bật/tắt `Theo dõi Anti` và `Cài/Gỡ Hook Anti` trong `Settings` (`424 x 398 DIP`).
- **Chặng 3 — File Shelf cạnh Dynamic trên màn hình FHD (`1920x1080`):**
  - [src/state/mod.rs](src/state/mod.rs), [src/state/hitboxes.rs](src/state/hitboxes.rs), [src/renderer/compositor.rs](src/renderer/compositor.rs) & [src/window/modes.rs](src/window/modes.rs): Mở rộng `MAX_SURFACE_WIDTH_DIP = 1040.0` giữ Dynamic căn giữa tâm FHD và đặt khay tệp ngay bên phải cách `10 DIP`; đồng bộ 2 lớp kính mờ `WinRtBackdropLayer` trên cùng `Compositor` + `CompositionDrawingSurface`, hợp vùng xuyên chuột qua GDI `CombineRgn(..., RGN_OR)`.
  - [src/shelf.rs](src/shelf.rs), [src/main.rs](src/main.rs), [src/renderer/shelf_view.rs](src/renderer/shelf_view.rs) & [src/window/input.rs](src/window/input.rs): Chuyển COM apartment sang `OleInitialize`/`OleUninitialize`, cài `IDropTarget` nhận kéo thả `CF_HDROP`, `IDropSource` + `IDataObject` (`DROPFILES` đa thư mục) + `DoDragDrop` kéo thả hai chiều ra ứng dụng khác, tickbox từng dòng + `"Chọn tất cả"`, ẩn hoàn toàn nút xóa khi `selected == 0` và chỉ hiện `Delete All` khi `selected >= 1` (dọn danh sách khay, giữ nguyên tuyệt đối tệp gốc trên đĩa).
- **Kiểm chứng thực tế `v0.3.0`:**
  - `cargo clippy --locked --all-targets -- -D warnings`: Đạt sạch (0 cảnh báo).
  - `cargo test -- --nocapture`: **44/44** tests đạt (`0.57s`), bao gồm GPU offscreen contrast (`5.34:1`), `AntiProgressSnapshot` & hook merge, `CF_HDROP` đa thư mục roundtrip, `ShelfState` tickbox/Delete All, và tương tác Win32 `HWND`.
  - `cargo build --release`: Tạo bản thực thi Portable `target\release\win-dynamic-island.exe` (**`841,728 bytes`**), đồng bộ sang `dist\win-dynamic-island.exe`, `dist\win-dynamic-island.exe.sha256` (`ffccb1d82a71111f4479575e09415ab6593154f991ff46cb84a06d1f92321646`) và `dist\WinDynamicIsland-win64.zip` (`401,766 bytes`).
  - Kiểm tra PE & runtime `dist\win-dynamic-island.exe` (`PID 2576`): `VCRUNTIME_DEP=False`, `WINHTTP_DEP=True`, `EMOJI_HITS=0`, `IDLE_CPU_2S_MS=0`, `WS_MB=50.34`, `PRIV_MB=20.64`, `HANDLES=549`, `NV_MODULES=0`, `AMD_MODULES=3`; ảnh chụp thực tế FHD xác nhận hiển thị sắc nét `Antigravity · Đang phân tích` kèm vòng chấm xoay và `Khay tệp` ở cạnh phải.

---

### Mục #029 — Sửa lỗi kéo thả tệp hai chiều `File Shelf` trên Windows 11 (Lỗi #015)
- **Thời gian thực hiện:** 2026-11-28T13:20:00+07:00
- **Phạm vi thay đổi:**
  - [src/shelf.rs](src/shelf.rs): Cải tiến `IslandDropTarget` chọn `DROPEFFECT` theo đúng bitmask đầu vào (`pick_drop_effect`), sửa `ShelfDataObject::EnumFormatEtc` chỉ trả format cho `DATADIR_GET` (chấp nhận `lindex == 0 | -1`), đổi `start_shelf_drag_out` sang chỉ dùng `DROPEFFECT_COPY` (tránh Explorer di chuyển mất tệp gốc) và bổ sung kiểm thử `IDropTarget::DragEnter` / `Drop` + `DATADIR_GET`/`SET`.
  - [src/window/mod.rs](src/window/mod.rs), [src/window/input.rs](src/window/input.rs), [src/window/workers.rs](src/window/workers.rs), [src/window/modes.rs](src/window/modes.rs): Bổ sung `WM_NCHITTEST` (`handle_nchittest` trả `HTCLIENT`/`HTTRANSPARENT`), mở bộ lọc thông điệp `ChangeWindowMessageFilterEx` (`WM_DROPFILES`, `WM_COPYGLOBALDATA`, `WM_COPYDATA`) kèm `DragAcceptFiles` + `WM_DROPFILES` fallback, tự bung `Khay tệp` khi kéo tệp tới (`shelf_auto_opened_by_drag`) và tách `ReleaseCapture()` + `start_shelf_drag_out()` ra ngoài borrow `&mut WindowController` trong `WM_MOUSEMOVE`; ghi nhận trạng thái fallback trên tooltip Tray nếu `RegisterDragDrop` lỗi.
  - [src/window/tests.rs](src/window/tests.rs): Bổ sung kịch bản bước #15 kiểm tra `WM_NCHITTEST`, tự mở/đóng `Khay tệp` khi `WM_APP_SHELF_HOVER`, và kiểm tra `handle_mouse_move` trả đường dẫn kéo ra ngoài borrow.
- **Kiểm chứng thực tế:**
  - `cargo clippy --locked --all-targets -- -D warnings`: Đạt sạch (0 cảnh báo).
  - `cargo test --locked`: **44/44** tests đạt (`0.34s`).
  - `cargo build --release --locked`: Tạo `target\release\win-dynamic-island.exe` (**`843,264 bytes`**, `SHA256=df17da6fa2f053060ead0348bdb8db7784d12da00641ea4ea01abc1aa77bc5df`, `VCRUNTIME_DEP=False`, `WINHTTP_DEP=True`, `EMOJI_HITS=0`, `NV_MODULES=0`, `WS_MB=54.11`, `PRIV_MB=24.45`, `HANDLES=563`).
  - Người dùng đã kiểm thử thực tế trên màn hình Desktop FHD với File Explorer (`PID 22488`): kéo thả thành công 2 tệp (`README.md`, `NHAT_KY_HOAT_DONG.md`) vào `Khay tệp (2)`.

---

### Mục #030 — Bấm thanh tiêu đề thu gọn nhanh `Khay tệp` & `Cài đặt` và đưa bản vá lên Git
- **Thời gian thực hiện:** 2026-11-28T13:27:30+07:00
- **Phạm vi thay đổi:**
  - [src/state/hitboxes.rs](src/state/hitboxes.rs) & [src/state/mod.rs](src/state/mod.rs): Tách `is_compact_click` dùng chung; thêm `should_collapse_shelf_on_header_click` (`y < rect.top + 34.0`) cho `Khay tệp` và `should_close_subview_on_header_click` (`y < pill.top + 38.0`) cho bảng `Cài đặt` (`Settings`) / `Thiết bị phát âm thanh` (`AudioDevices`), vẫn giữ nguyên nút `"Thu gọn"` và `"Quay lại"` cùng cơ chế chặn đóng nhầm khi bấm khe nút bên dưới.
  - [src/window/input.rs](src/window/input.rs) & [src/window/mod.rs](src/window/mod.rs): Thêm `hovered_header` và `is_pointer_over_collapsible_header` đổi con trỏ bàn tay (`IDC_HAND`) khi rê lên thanh tiêu đề; xử lý bấm gọn thanh tiêu đề trong `handle_left_button_up` để thu gọn `Khay tệp` hoặc quay lại ngay từ `Settings` / `AudioDevices`.
  - [src/window/tests.rs](src/window/tests.rs): Bổ sung kịch bản bước #16 kiểm chứng bấm thanh tiêu đề `Khay tệp` và `Cài đặt` thu gọn/quay lại nhanh.
  - [GEMINI.md](GEMINI.md): Bổ sung quy tắc kéo thả OLE trên cửa sổ `WS_EX_NOREDIRECTIONBITMAP` vào Mục 5 theo duyệt của người dùng.
- **Kiểm chứng thực tế:**
  - `cargo clippy --locked --all-targets -- -D warnings`: Đạt sạch (0 cảnh báo).
  - `cargo test --locked`: **44/44** tests đạt (`0.27s`).
  - `cargo build --release --locked`: Tạo `target\release\win-dynamic-island.exe` (**`845,312 bytes`**, `SHA256=1ce00d14bb285db71120dc6955cd19e078bfc6561728b33151913f26b78770f1`, `VCRUNTIME_DEP=False`, `WINHTTP_DEP=True`, `EMOJI_HITS=0`).

---

## Chốt bàn giao cuối phiên (Session Handoff)
- **Trạng thái hiện tại:** Đã hoàn tất 3 chặng, kiểm thử sạch, đóng gói và phát hành **v0.3.0** lên GitHub (`tag v0.3.0`, `841,728 bytes`), đồng bộ gói Portable `dist/`, dọn sạch `target/` và duy trì bản `dist\win-dynamic-island.exe` chạy nền ổn định.
- **Lưu ý cho phiên tiếp theo:**
  - Thư mục `target/` đã được dọn sạch để tiết kiệm dung lượng ổ cứng; lần chạy `cargo` tiếp theo sẽ biên dịch lại phụ thuộc từ đầu.
  - Nếu cần chạy thử bản mới biên dịch trong `target/`, cần thoát tiến trình đang chạy từ `dist\win-dynamic-island.exe` trước (do khóa `SingleInstanceGuard`).
  - Hook Anti (`~/.gemini/config/hooks.json`) chưa tự động ghi đè vào máy khi chưa kích hoạt; người dùng có thể bấm nút `"Hook Anti"` trong bảng `Settings` bất kỳ lúc nào để bật/tắt an toàn.
- **Hạng mục tiếp theo (khi có yêu cầu):** Mở rộng thêm Pomodoro mini, theo dõi tài nguyên gọn hoặc hỗ trợ thư mục kéo nhanh theo ngữ cảnh nếu người dùng đề xuất thêm.
