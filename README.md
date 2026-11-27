# WIN DYNAMIC ISLAND (WINDOWS 11 + RUST)

Thanh viên thuốc thông minh (Dynamic Island) gọn nhẹ viết thuần bằng **Rust + Win32 / Direct2D / WinRT Composition** cho Windows 11.

## Tính năng chính
- **Hiệu ứng kính Liquid Glass thời gian thực:** Sử dụng `Windows::UI::Composition` (`HostBackdropBrush` qua chuỗi `CLSID_D2D1Saturation` + `CLSID_D2D1GaussianBlur`) kết hợp `CompositionDrawingSurface` đồng pha, lớp kính khói chống lóa và tự đồng bộ sắc màu + điểm nhấn theo ảnh bìa video/bài hát đang phát.
- **Điều khiển nhạc hệ thống (GSMTC) & Bố cục Compact 2 tầng:** Tự động bắt phiên phát nhạc từ trình duyệt (Chrome, Edge...) và các trình phát Windows, hiển thị ảnh bìa tròn, tiêu đề + nghệ sĩ, thanh tiến độ, nút vector bo tròn và kéo trượt Timeline mượt (`SetCapture`).
- **Tab-Safe Notch & Chuyển động lò xo 144Hz:** Thu gọn về dải Notch mảnh `152 x 16 DIP` bám mép `y = 0` không chắn tab trình duyệt, đồng bộ nhịp `DwmFlush` và ngủ `0%` CPU khi đứng yên.
- **HUD Âm lượng, Pin / Sạc & Chuyển thiết bị âm thanh:** Hiển thị HUD tức thời khi đổi âm lượng hoặc cắm/rút sạc, hỗ trợ cuộn chuột chỉnh loa trực tiếp trên Island và chuyển nhanh thiết bị phát mặc định qua `IPolicyConfig`.
- **Tối ưu Laptop Hybrid GPU & gọn nhẹ:** Tự ưu tiên GPU tích hợp (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`), không đánh thức card rời NVIDIA khi rảnh; tự ẩn khi phát hiện ứng dụng toàn màn hình.
- **Bản Portable độc lập & Tự cập nhật Liquid Glass:** Liên kết tĩnh CRT (`+crt-static`) không cần Visual C++ Redistributable, chặn mở trùng tiến trình (`CreateMutexW`) và tự kiểm tra/tải bản cập nhật từ GitHub Releases (`WinHTTP` + `SHA-256`) kèm thẻ thông báo nội bộ.

## Cấu trúc thư mục
- `src/main.rs` — Điểm khởi chạy, khóa đơn tiến trình và dọn tệp sao lưu sau cập nhật.
- `src/window.rs` — Cửa sổ không viền `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP`, tương tác chuột và vòng lặp sự kiện.
- `src/state.rs` — Trạng thái Island, mô hình lò xo và tính toán hitbox tương tác.
- `src/renderer.rs` — Tầng vẽ Direct2D / DirectWrite / WinRT `CompositionDrawingSurface` Liquid Glass và đồng bộ bảng màu video.
- `src/media.rs` — Lắng nghe GSMTC bất đồng bộ, giải mã ảnh bìa WIC và tua nhạc.
- `src/hud.rs` — Giám sát âm lượng WASAPI, trạng thái nguồn điện/pin và chuyển thiết bị phát qua `IPolicyConfig`.
- `src/updater.rs` — Kiểm tra và tự cập nhật qua HTTPS `WinHTTP` + kiểm tra SHA-256.
- `src/vsync.rs` — Bộ phát nhịp `DwmFlush` tiết kiệm CPU.
- `src/watcher.rs` — Phát hiện ứng dụng toàn màn hình và đa màn hình DPI v2.
- `src/tray.rs` & `src/config.rs` — Khay hệ thống, lưu cấu hình (`%LOCALAPPDATA%\WinDynamicIsland\config.ini`) và Autostart.
- `.github/workflows/release.yml` — Quy trình GitHub Actions tự động biên dịch bản phát hành khi đẩy tag `v*`.

## Tự biên dịch từ mã nguồn
Yêu cầu công cụ Rust MSVC (`x86_64-pc-windows-msvc`):

```powershell
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Thành phẩm bản phát hành nằm tại `target\release\win-dynamic-island.exe`.
