# KẾ HOẠCH KIẾN TRÚC VÀ TRIỂN KHAI: WIN DYNAMIC ISLAND (PHASE 4 - PLAN)

- **Mã tính năng (Slug):** `dynamic-island`
- **Trạng thái:** Đã duyệt (2026-11-27)
- **Mục tiêu:** Xây dựng MVP v0.1 bằng Rust thuần Native Windows 11, đạt độ mượt khung hình cao theo tần số quét gốc màn hình nhưng không đánh thức GPU rời NVIDIA RTX 3050 và tiêu tốn gần 0% tài nguyên khi đứng yên.

---

## 1. Kiến trúc hệ thống (3 luồng tách biệt)

1. **Luồng UI (`src/main.rs`, `src/window.rs`, `src/renderer.rs`, `src/tray.rs`):**
   - Sở hữu `HWND` với cờ `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP` (ẩn khỏi Taskbar và `Alt + Tab`, không lưu bitmap GDI dư thừa).
   - Khởi tạo pipeline `DirectComposition` + `DXGI Flip-Model SwapChain` kích thước tối đa cố định + `Direct2D` + `DirectWrite` trên iGPU (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`).
   - Cập nhật `SetWindowRgn` bám theo hình viên thuốc (cộng dư 1–2 px giữ antialiasing) để vùng trống hai bên xuyên chuột hoàn toàn xuống ứng dụng bên dưới.
   - Chạy vòng đồng bộ khung hình chỉ trong thời gian có animation lò xo hoặc sóng nhạc; tự động ngắt hoàn toàn khi lò xo đạt trạng thái nghỉ.
   - Nhận sự kiện từ các luồng nền qua `std::sync::mpsc` kết hợp `PostMessageW` (custom `WM_APP`) để đánh thức ngay lập tức mà không cần polling.

2. **Luồng Media (`src/media.rs`):**
   - Kết nối Windows GSMTC (`GlobalSystemMediaTransportControlsSessionManager`) trên luồng nền riêng.
   - Theo dõi đổi phiên phát nhạc, tiêu đề bài hát, nghệ sĩ, trạng thái Play/Pause.
   - Giải mã và thu nhỏ ảnh bìa về kích thước nhỏ (tối đa 128x128 px) ngay trên luồng nền trước khi gửi sang UI, tránh giữ ảnh lớn hoặc panic khi thumbnail null.
   - Nhận lệnh điều khiển (Play/Pause/Next/Previous) từ UI qua channel.

3. **Luồng Giám sát hệ thống (`src/watcher.rs`):**
   - Phát hiện ứng dụng foreground đang bật toàn màn hình (game, video fullscreen) để tự ẩn Island, và khôi phục ngay khi thoát fullscreen.
   - Theo dõi thay đổi màn hình / độ phân giải / DPI để tính lại vị trí căn giữa.

---

## 2. Cấu trúc tệp dự kiến

- `Cargo.toml` — Khai báo package và duy nhất crate `windows` với đúng các feature cần dùng.
- `build.rs` & `app.manifest` — Nhúng manifest Per-Monitor DPI Awareness v2 vào tệp `.exe`.
- `src/main.rs` — Điểm vào chương trình (`#![windows_subsystem = "windows"]`), khởi tạo luồng và message loop.
- `src/state.rs` — Trạng thái hiển thị (`Hidden`, `Slim`, `Compact`, `Expanded`), cấu trúc `MediaInfo`, vật lý lò xo bảo toàn vận tốc (`SpringState`), cấu hình ứng dụng.
- `src/window.rs` — Vòng đời `HWND`, tính tọa độ đa màn hình/DPI, xử lý hover/click và `SetWindowRgn`.
- `src/renderer.rs` — Khởi tạo DXGI iGPU / DirectComposition / Direct2D / DirectWrite / WIC và vẽ viên thuốc bo tròn, ảnh bìa, văn bản tiếng Việt/CJK.
- `src/media.rs` — Tích hợp WinRT GSMTC và giải mã thumbnail an toàn.
- `src/watcher.rs` — Phát hiện Fullscreen và thay đổi màn hình.
- `src/tray.rs` — System Tray (`Shell_NotifyIconW`) với menu tiếng Việt chuẩn và tùy chọn tự khởi động (`HKCU`).

---

## 3. Lộ trình triển khai cuốn chiếu (4 bước)

1. **Bước 1 — Khung dự án, Cửa sổ Overlay & System Tray:**
   - Khởi tạo `Cargo.toml`, `app.manifest`, `build.rs`, khung `window.rs` (DirectComposition/iGPU cơ bản hoặc nền đen bo tròn khởi đầu) và `tray.rs`.
   - *Kiểm chứng:* Biên dịch sạch, không hiện trên Taskbar/`Alt+Tab`, bấm xuyên vùng rỗng, thoát sạch qua System Tray.
2. **Bước 2 — Vẽ Direct2D/DirectWrite & Animation lò xo mượt VSync:**
   - Hoàn thiện `state.rs` (vật lý lò xo ngắt quãng mượt) và `renderer.rs`, ngắt vòng VSync khi đứng yên.
   - *Kiểm chứng:* Co giãn mượt theo tần số quét màn hình; CPU ~0% khi idle; kiểm tra GPU rời NVIDIA RTX 3050 không bị đánh thức.
3. **Bước 3 — Tích hợp nhạc Windows GSMTC:**
   - Triển khai `media.rs`, hiển thị ảnh bìa bo góc, tiêu đề/nghệ sĩ tiếng Việt & CJK, nút Play/Pause/Next/Previous.
   - *Kiểm chứng:* Test thực tế với trình duyệt/Spotify, test trường hợp không có ảnh bìa và đóng trình phát đột ngột.
4. **Bước 4 — Tự ẩn Fullscreen, Đa màn hình & Hoàn thiện v0.1:**
   - Triển khai `watcher.rs`, menu chọn màn hình, tùy chọn tự ẩn khi rảnh và tự khởi động.
   - *Kiểm chứng:* Test Fullscreen, đổi DPI 100%/125%/150%, build release và đối chiếu toàn bộ `SO_TAY_KIEM_TRA_BUG.md`.

---

## 4. Phân tích rủi ro & Phòng ngừa

- **Đánh thức GPU rời (NVIDIA RTX 3050):** Bắt buộc dùng `IDXGIFactory6::EnumAdapterByGpuPreference` với `DXGI_GPU_PREFERENCE_MINIMUM_POWER` và ngắt render loop khi lò xo dừng.
- **Rò rỉ tài nguyên Win32/COM/GDI:** Bọc `HWND`, `HRGN`, COM/WinRT handles với `Drop` rõ ràng; giải phóng toàn bộ trước khi thoát.
- **Panic do dữ liệu hệ điều hành:** Không dùng `unwrap()`/`expect()` khi đọc dữ liệu GSMTC hoặc cửa sổ foreground; fallback an toàn khi thiếu dữ liệu.
- **Bẫy `WM_NCHITTEST`:** Không dựa vào `HTTRANSPARENT` vì chỉ hoạt động nội bộ thread; dùng `SetWindowRgn` cắt vùng rỗng bên ngoài viên thuốc để xuyên chuột sang mọi ứng dụng khác.
