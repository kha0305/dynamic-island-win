# WIN DYNAMIC ISLAND (WINDOWS 11 + RUST)

Thanh viên thuốc thông minh (Dynamic Island) gọn nhẹ viết thuần bằng **Rust + Win32 / Direct2D / WinRT Composition** cho Windows 11.

## Tính năng chính
- **Hiệu ứng kính Liquid Glass thời gian thực:** Sử dụng `Windows::UI::Composition` (`HostBackdropBrush` qua chuỗi `CLSID_D2D1Saturation` + `CLSID_D2D1GaussianBlur`) kết hợp `CompositionDrawingSurface` đồng pha, lớp kính khói chống lóa và tự đồng bộ sắc màu + điểm nhấn theo ảnh bìa video/bài hát đang phát.
- **Điều khiển nhạc hệ thống (GSMTC) & Bố cục Expanded đối xứng:** Tự động bắt phiên phát nhạc từ trình duyệt (Chrome, Edge...) và các trình phát Windows; chế độ `Compact` 2 tầng có ảnh bìa tròn, thanh tiến độ và sóng nhạc; chế độ `Expanded` rộng rãi với nút `Cài đặt` và chọn thiết bị phát đối xứng ở hàng đáy cùng thanh kéo trượt Timeline mượt (`SetCapture`).
- **Tab-Safe Notch & Chuyển động lò xo 144Hz:** Thu gọn về dải Notch mảnh `152 x 16 DIP` tối giản bám mép `y = 0` không chắn tab trình duyệt, đồng bộ nhịp `DwmFlush` và ngủ `0%` CPU khi đứng yên.
- **HUD Âm lượng, Pin / Sạc, Thiết bị phát & Bảng điều khiển nhanh:** Hiển thị HUD tức thời khi đổi âm lượng hoặc cắm/rút sạc, hỗ trợ cuộn chuột chỉnh loa trực tiếp trên Island, chuyển nhanh thiết bị phát mặc định qua `IPolicyConfig` với danh sách tự co chiều cao, cùng bảng `Settings` phân 3 nhóm tách màu hệ thống và kiểm tra cập nhật trực tiếp.
- **File Shelf cạnh Dynamic (FHD):** Khay tệp dính ngay cạnh phải Dynamic Island (`OleInitialize` + `IDropTarget` + `IDropSource`/`IDataObject`), hỗ trợ kéo thả hai chiều với Explorer/trình duyệt/app nhắn tin, tickbox chọn tệp và chỉ hiện `Delete All` khi có mục được tick (dọn khay tạm, giữ nguyên tệp gốc trên đĩa).
- **Theo dõi Anti 10 trạng thái (`agy CLI`, `Anti IDE`, `Anti app`):** Dùng chung 1 tệp tiến độ cục bộ (`%LOCALAPPDATA%\WinDynamicIsland\anti-progress.json`) qua cổng hook `--anti-hook <EVENT>` + `ReadDirectoryChangesW` (`0% CPU` lúc rảnh), chỉ hiển thị kèm animation xoay khi chuyển sang ứng dụng khác.
- **Tối ưu Laptop Hybrid GPU & gọn nhẹ:** Tự ưu tiên GPU tích hợp (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`), không đánh thức card rời NVIDIA khi rảnh; tự ẩn khi phát hiện ứng dụng toàn màn hình.
- **Bản Portable độc lập & Tự cập nhật Liquid Glass:** Liên kết tĩnh CRT (`+crt-static`) không cần Visual C++ Redistributable, chặn mở trùng tiến trình (`CreateMutexW`) và tự kiểm tra/tải bản cập nhật từ GitHub Releases (`WinHTTP` + `SHA-256`) kèm thẻ thông báo nội bộ.

## Cấu trúc thư mục
- `src/main.rs` — Điểm khởi chạy, cổng `--anti-hook`, khóa đơn tiến trình và `OleInitialize`.
- `src/anti.rs` — Miền dữ liệu 10 trạng thái Anti, quản lý Global Hook an toàn và luồng `ReadDirectoryChangesW`.
- `src/shelf.rs` — Trạng thái File Shelf và bộ xử lý OLE Drag-Drop hai chiều (`CF_HDROP`).
- `src/json_mini.rs` — Bộ phân tích JSON gọn không phụ thuộc và `sha256_hex`.
- `src/window/` — Cửa sổ không viền `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP`, `CombineRgn` và vòng lặp sự kiện (`mod`, `modes`, `input`, `workers`, `tests`).
- `src/state/` — Trạng thái Island, mô hình lò xo, hình học Dynamic + Shelf và hitbox (`mod`, `animator`, `hitboxes`, `tests`).
- `src/renderer/` — Tầng vẽ Direct2D / DirectWrite / WinRT `CompositionDrawingSurface` 2 lớp kính mờ (`mod`, `compositor`, `theme`, `media_views`, `expanded_view`, `hud_views`, `settings_views`, `anti_view`, `shelf_view`, `tests`).
- `src/hud/` — Giám sát âm lượng WASAPI, trạng thái nguồn điện/pin và chuyển thiết bị phát qua `IPolicyConfig` (`mod`, `policy_config`, `power`, `tests`).
- `src/media.rs` — Lắng nghe GSMTC bất đồng bộ, giải mã ảnh bìa WIC và tua nhạc.
- `src/updater.rs` — Kiểm tra và tự cập nhật qua HTTPS `WinHTTP` + kiểm tra SHA-256.
- `src/vsync.rs` — Bộ phát nhịp `DwmFlush` tiết kiệm CPU.
- `src/watcher.rs` — Phát hiện ứng dụng toàn màn hình, cửa sổ Anti foreground và đa màn hình DPI v2.
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
