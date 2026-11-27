# ĐẶC TẢ TÍNH NĂNG: TỰ CẬP NHẬT QUA GITHUB RELEASES (AUTO-UPDATE)

## 1. Mục tiêu
Cho phép người dùng cuối (bao gồm bạn bè dùng bản Portable `win-dynamic-island.exe`) tự kiểm tra, tải về và cập nhật lên phiên bản mới từ `https://github.com/kha0305/dynamic-island-win` mà không cần gỡ cài đặt hay thao tác thủ công.

## 2. Nguyên tắc thiết kế & An toàn
- **Không tốn RAM/CPU nền:** Dùng API `WinHTTP` gốc của Windows (`winhttp.dll`), không thêm thư viện mạng/async nặng (`tokio`/`reqwest`). Chỉ tạo luồng kiểm tra ngắn hạn khi khởi động (~10 giây sau khi mở, chạy 1 lần rồi tự thoát) hoặc khi người dùng bấm *"Kiểm tra cập nhật"*.
- **Bảo mật kết nối & toàn vẹn tệp:**
  - Chỉ cho phép giao thức `HTTPS` và domain hợp lệ của GitHub (`api.github.com`, `github.com`, `*.githubusercontent.com`), chặn hạ cấp HTTPS xuống HTTP (`WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP`).
  - Đối chiếu hàm băm **SHA-256** nếu release đính kèm tệp `win-dynamic-island.exe.sha256`, đồng thời kiểm tra header PE `"MZ"` trước khi thay thế tệp thực thi.
- **Thay thế tệp đang chạy an toàn trên Windows:**
  - Tải về `win-dynamic-island.exe.new` cùng thư mục hiện tại.
  - Đổi tên tệp đang chạy thành `win-dynamic-island.exe.old`, đổi `.new` thành `win-dynamic-island.exe`.
  - Khởi chạy tiến trình mới với cờ `--post-update` (chờ khóa `SingleInstanceGuard` được tiến trình cũ nhả ra) rồi thoát tiến trình cũ.
  - Tiến trình mới tự dọn tệp `.old` trên nền mà không làm trễ khởi động.
