# WIN DYNAMIC ISLAND (WINDOWS 11 + RUST)

Thanh viên thuốc thông minh (Dynamic Island) gọn nhẹ viết thuần bằng **Rust + Win32 / Direct2D / WinRT Composition** cho Windows 11.

## Tính năng chính
- **Hiệu ứng kính Liquid Glass thời gian thực:** Sử dụng `Windows::UI::Composition` (`HostBackdropBrush` qua chuỗi `CLSID_D2D1Saturation` + `CLSID_D2D1GaussianBlur`) kết hợp `CompositionDrawingSurface` đồng pha, lớp kính khói chống lóa và tự đồng bộ sắc màu + điểm nhấn theo ảnh bìa video/bài hát đang phát.
- **Điều khiển nhạc hệ thống (GSMTC) & Icon thiết bị âm thanh:** Tự động bắt phiên phát nhạc từ trình duyệt (Chrome, Edge...) và các trình phát Windows; chế độ `Compact` (`338 x 40 DIP`) hiển thị ảnh bìa tròn, thanh tiến độ, icon thiết bị âm thanh (`Loa` / `Tai nghe` / `Bluetooth`, bấm mở nhanh danh sách chọn) và sóng nhạc; chế độ `Expanded` rộng rãi với nút `Cài đặt` và chọn thiết bị phát đối xứng ở hàng đáy cùng thanh kéo trượt Timeline mượt (`SetCapture`).
- **Tab-Safe Notch & Chuyển động lò xo 144Hz:** Thu gọn về dải Notch mảnh `152 x 16 DIP` tối giản bám mép `y = 0` không chắn tab trình duyệt, đồng bộ nhịp `DwmFlush` và ngủ `0%` CPU khi đứng yên.
- **HUD Âm lượng, Pin / Sạc, Bluetooth (`% pin`) & Thiết bị ngoại vi:** Hiển thị HUD tức thời khi đổi âm lượng, cắm/rút sạc, kết nối/ngắt thiết bị Bluetooth (kèm `% pin` và viên pin mini), cắm/rút tai nghe hoặc USB/ngoại vi (gộp theo `ContainerId`); hỗ trợ cuộn chuột chỉnh loa trực tiếp trên Island, chuyển nhanh thiết bị phát mặc định qua `IPolicyConfig` và bảng `Settings` (`424 x 436 DIP`).
- **File Shelf cạnh Dynamic (FHD):** Khay tệp dính ngay cạnh phải Dynamic Island (`OleInitialize` + `IDropTarget` + `IDropSource`/`IDataObject`), hỗ trợ kéo thả hai chiều với Explorer/trình duyệt/app nhắn tin, tickbox chọn tệp và chỉ hiện `Delete All` khi có mục được tick (dọn khay tạm, giữ nguyên tệp gốc trên đĩa).
- **Theo dõi Anti 10 trạng thái (`agy CLI`, `Anti IDE`, `Anti App`):** Nhận diện nguồn độc lập đường dẫn cài đặt, hỗ trợ theo dõi đồng thời cả Hook `--anti-hook` và nhật ký `transcript.jsonl` (`Anti IDE`), hiển thị đa phiên (viên chính ở giữa, viên phụ kính mờ `232 x 40 DIP` bên trái), tóm tắt thao tác gọn (`toolSummary`), thanh ánh sáng đáy, màu phân biệt nguồn, thẻ chi tiết `AntiExpanded` (`Mở cửa sổ`, `Ẩn nhanh`, `Cài đặt`) và viền nhấn vàng/đỏ khi `Đang chờ duyệt` hoặc `Bị lỗi` (`0% CPU` lúc rảnh qua `ReadDirectoryChangesW`).
- **Tối ưu Laptop Hybrid GPU & gọn nhẹ:** Tự ưu tiên GPU tích hợp (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`), không đánh thức card rời NVIDIA khi rảnh; tự ẩn khi phát hiện ứng dụng toàn màn hình.
- **Hai gói phân phối (Setup + Portable) & Tự cập nhật Liquid Glass:** Hỗ trợ cả trình cài đặt chuẩn `WinDynamicIsland-Setup.exe` (tạo lối tắt Start Menu/Desktop, gỡ cài đặt trong Windows Settings, giữ quyền `Medium IL`) lẫn bản chạy ngay Portable (`WinDynamicIsland-win64.zip` liên kết tĩnh CRT `+crt-static` không cần Visual C++ Redistributable), đi kèm bộ Icon chính thức, chặn mở trùng (`CreateMutexW`), phục hồi Startup/TaskbarCreated và tự cập nhật từ GitHub Releases (`WinHTTP` + `SHA-256`).

## Cấu trúc thư mục
- `assets/app.ico` & `build.rs` — Biểu tượng đa kích thước Liquid Glass và trình nhúng tài nguyên PE thuần Rust (Icon, Version, Manifest).
- `src/main.rs` — Điểm khởi chạy, cổng `--anti-hook`, `--uninstall`, retry khởi tạo lúc logon, khóa đơn tiến trình và `OleInitialize`.
- `src/install.rs` & `src/bin/setup.rs` — Trình cài đặt `WinDynamicIsland-Setup.exe`, tạo Shortcut COM và gỡ cài đặt sạch sẽ.
- `src/anti.rs` — Miền dữ liệu 10 trạng thái Anti, quản lý Global Hook an toàn và luồng `ReadDirectoryChangesW`.
- `src/shelf.rs` — Trạng thái File Shelf và bộ xử lý OLE Drag-Drop hai chiều (`CF_HDROP`).
- `src/json_mini.rs` — Bộ phân tích JSON gọn không phụ thuộc và `sha256_hex`.
- `src/window/` — Cửa sổ không viền `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP`, `CombineRgn` và vòng lặp sự kiện (`mod`, `modes`, `input`, `workers`, `tests`).
- `src/state/` — Trạng thái Island, mô hình lò xo, hình học Dynamic + Shelf và hitbox (`mod`, `animator`, `hitboxes`, `tests`).
- `src/renderer/` — Tầng vẽ Direct2D / DirectWrite / WinRT `CompositionDrawingSurface` 3 lớp kính mờ (`mod`, `compositor`, `theme`, `media_views`, `expanded_view`, `hud_views`, `settings_views`, `anti_view`, `shelf_view`, `tests`).
- `src/hud/` — Giám sát âm lượng WASAPI, trạng thái nguồn điện/pin, thiết bị Bluetooth (`% pin`) / USB / ngoại vi (`device.rs`) và chuyển thiết bị phát qua `IPolicyConfig` (`mod`, `device`, `policy_config`, `power`, `tests`).
- `src/media.rs` — Lắng nghe GSMTC bất đồng bộ, giải mã ảnh bìa WIC và tua nhạc.
- `src/updater.rs` — Kiểm tra và tự cập nhật qua HTTPS `WinHTTP` + kiểm tra SHA-256.
- `src/vsync.rs` — Bộ phát nhịp `DwmFlush` tiết kiệm CPU.
- `src/watcher.rs` — Phát hiện ứng dụng toàn màn hình, cửa sổ Anti foreground và đa màn hình DPI v2.
- `src/tray.rs` & `src/config.rs` — Khay hệ thống, lưu cấu hình (`%LOCALAPPDATA%\WinDynamicIsland\config.ini`) và Autostart.
- `.github/workflows/release.yml` — Quy trình GitHub Actions tự động biên dịch bản phát hành khi đẩy tag `v*`.

## Tự biên dịch từ mã nguồn
Yêu cầu công cụ Rust MSVC (`x86_64-pc-windows-msvc`):

```powershell
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked --bin win-dynamic-island
$env:WDI_SETUP_PAYLOAD = (Resolve-Path target\release\win-dynamic-island.exe).Path
cargo build --release --locked --bin win-dynamic-island-setup
```

Thành phẩm bản phát hành nằm tại `target\release\win-dynamic-island.exe` và `target\release\win-dynamic-island-setup.exe`.
