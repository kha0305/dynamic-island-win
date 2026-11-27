# WIN DYNAMIC ISLAND (WINDOWS 11 + RUST)

Thanh viên thuốc thông minh (Dynamic Island) gọn nhẹ viết thuần bằng **Rust + Win32 / Direct2D / WinRT Composition** cho Windows 11.

## Tính năng chính
- **Hiệu ứng kính Liquid Glass thời gian thực:** Sử dụng `Windows::UI::Composition` (`HostBackdropBrush` qua chuỗi `CLSID_D2D1Saturation` + `CLSID_D2D1GaussianBlur`) kết hợp lớp bóng specular hai chiều và tự đồng bộ sắc màu + điểm nhấn theo ảnh bìa video/bài hát đang phát.
- **Điều khiển nhạc hệ thống (GSMTC):** Tự động bắt phiên phát nhạc từ trình duyệt (Chrome, Edge...) và các trình phát Windows, hiển thị ảnh bìa cắt vuông, thanh timeline tua trực tiếp và các nút vector bo tròn mềm mại.
- **Chuyển động lò xo mượt theo tần số quét:** Tự khóa nhịp `WaitForVBlank` theo từng màn hình (60Hz–240Hz), tự thu gọn sau 5 giây khi rời chuột và ngủ `0%` CPU khi đứng yên.
- **Tối ưu Laptop Hybrid GPU & gọn nhẹ:** Tự ưu tiên GPU tích hợp (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`), không đánh thức card rời NVIDIA khi rảnh; tự ẩn khi phát hiện ứng dụng toàn màn hình.
- **Bản Portable độc lập & Tự cập nhật:** Liên kết tĩnh CRT (`+crt-static`) không cần Visual C++ Redistributable, chặn mở trùng tiến trình (`CreateMutexW`) và hỗ trợ tự kiểm tra/tải bản cập nhật trực tiếp từ GitHub Releases kèm xác minh **SHA-256**.

## Cấu trúc thư mục
- `src/main.rs` — Điểm khởi chạy, khóa đơn tiến trình và dọn tệp sao lưu sau cập nhật.
- `src/window.rs` — Cửa sổ không viền `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP`, tương tác chuột và vòng lặp sự kiện.
- `src/renderer.rs` — Tầng vẽ Direct2D / DirectWrite / WinRT `HostBackdropBrush` Liquid Glass và đồng bộ bảng màu video.
- `src/media.rs` — Lắng nghe GSMTC bất đồng bộ, giải mã ảnh bìa WIC và tua nhạc.
- `src/updater.rs` — Kiểm tra và tự cập nhật qua HTTPS `WinHTTP` + kiểm tra SHA-256.
- `src/vsync.rs` — Bộ phát nhịp `WaitForVBlank` tiết kiệm CPU.
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
