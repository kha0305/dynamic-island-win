# ĐẶC TẢ TÍNH NĂNG: BỘ ICON CHÍNH THỨC, SỬA LỖI STARTUP & TRÌNH CÀI ĐẶT SETUP (`v0.3.4`)

- **Mã tính năng (Slug):** `setup-startup-icon`
- **Ngày lập:** 2026-11-28
- **Phiên bản mục tiêu:** `0.3.4`
- **Trạng thái:** Đang triển khai theo phê duyệt của người dùng.

---

## 1. Mục tiêu và bài toán thực tế
1. **Biểu tượng ứng dụng chuyên nghiệp:** Tệp `win-dynamic-island.exe` hiện chưa có resource icon nhúng vào PE executable và `WNDCLASSEXW`, còn khay hệ thống dùng bitmap 16x16 vẽ tay đơn giản. Cần tạo bộ icon đa kích thước sắc nét (`16x16` đến `256x256`) và nhúng đồng bộ cho `.exe`, cửa sổ, shortcut và System Tray.
2. **Startup lúc chạy lúc không khi mở máy:** Khắc phục triệt để các nguyên nhân thất bại lúc logon Windows 11 (thoát im lặng khi DWM/đồ họa chưa sẵn sàng, mất khay hệ thống khi Explorer Taskbar khởi tạo chậm, kiểm tra thiếu `Explorer\StartupApproved\Run` và đường dẫn cũ trong Registry) mà vẫn giữ nguyên mức quyền chuẩn `Medium IL` để kéo thả `File Shelf` hoạt động bình thường.
3. **Song song 2 bản phân phối (Setup + Portable):** Cung cấp cả trình cài đặt **`WinDynamicIsland-Setup.exe`** (cài đặt chuẩn vào `%LOCALAPPDATA%\Programs\WinDynamicIsland`, tạo Shortcut Start Menu/Desktop, đăng ký Installed Apps kèm gỡ cài đặt sạch `--uninstall`) lẫn bản chạy ngay Portable (`WinDynamicIsland-win64.zip`).

---

## 2. Yêu cầu chức năng
- **FR-1 (Icon chính thức):**
  - Tạo `assets/app.ico` gồm các kích thước `16, 20, 24, 32, 48, 256` px theo phong cách nền vuông bo tròn tối kiểu Liquid Glass và viên thuốc Dynamic Island nổi bật.
  - Nhúng `RT_ICON` / `RT_GROUP_ICON`, `RT_VERSION` và `RT_MANIFEST` (`asInvoker`, `Common-Controls v6`, `PerMonitorV2`) qua `build.rs` thuần Rust.
  - Gắn `hIcon`/`hIconSm` cho `WNDCLASSEXW` và tải icon theo DPI (`SM_CXSMICON` / `SM_CYSMICON`) cho `TrayIcon`.
- **FR-2 (Startup tin cậy ở `Medium IL`):**
  - `src/main.rs`: Thử lại `IslandWindow::create()` tối đa 8 nhịp khi vừa logon nếu hệ thống đồ họa chưa sẵn sàng; ghi lỗi chi tiết ra `%LOCALAPPDATA%\WinDynamicIsland\startup.log` nếu thất bại hoàn toàn.
  - `src/window/mod.rs` & `src/tray.rs`: Lắng nghe `RegisterWindowMessageW("TaskbarCreated")` và thêm timer thử lại gắn `TrayIcon` nếu Explorer Taskbar khởi động sau ứng dụng.
  - `src/config.rs`: Xác thực đường dẫn `.exe` trong `HKCU\...\Run` còn tồn tại trên đĩa, kiểm tra trạng thái vô hiệu hóa tại `HKCU\...\Explorer\StartupApproved\Run`, xóa cờ chặn khi bật lại Autostart, và tự đồng bộ lại đường dẫn khi khởi chạy nếu đường dẫn cũ không còn tồn tại.
- **FR-3 (Trình cài đặt `WinDynamicIsland-Setup.exe` & Gỡ cài đặt `--uninstall`):**
  - Giao diện Win32 tiếng Việt có dấu chuẩn xác (tùy chọn Startup, Desktop Shortcut, chạy ngay sau cài đặt; hỗ trợ cờ `--silent` / `/S`).
  - Tự đóng êm tiến trình đang chạy trước khi chép tệp vào `%LOCALAPPDATA%\Programs\WinDynamicIsland\win-dynamic-island.exe`.
  - Tạo Shortcut Start Menu + Desktop có Icon chính thức; ghi khóa `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\WinDynamicIsland` để hiển thị và gỡ sạch từ Windows Settings > Apps.
  - Cập nhật `.github/workflows/release.yml` phát hành cả `WinDynamicIsland-Setup.exe` lẫn `WinDynamicIsland-win64.zip` và `win-dynamic-island.exe`.
