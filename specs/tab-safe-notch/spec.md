# ĐẶC TẢ TÍNH NĂNG: CHẾ ĐỘ BÁM MÉP KHÔNG VƯỚNG TAB (TAB-SAFE NOTCH)

- **Mã tính năng (Slug):** `tab-safe-notch`
- **Ngày lập:** 2026-11-27
- **Trạng thái:** Hoàn tất triển khai Phase 6 & kiểm thử lõi (Phase 7) — phát hành `v0.1.2`.

---

## 1. Bối cảnh và vấn đề
- Trên Windows 11, các trình duyệt như Google Chrome / Edge và nhiều ứng dụng đặt thanh tab ngay tại dải cạnh trên cùng (`y = 0..40 px`).
- Chế độ `Compact` hiện tại (`276x36 DIP` tại `top = 6 DIP`) chiếm trọn khoảng `y = 6..42 px`, dễ đè lên tiêu đề và nút đóng tab ở giữa màn hình khi mở nhiều tab.

## 2. Yêu cầu đã chốt tại Phase 3
1. **Hình dáng thanh Notch bám mép trên (`IslandMode::Slim`):**
   - Kích thước **`152 x 16 DIP`** bám sát cạnh trên **`top = 0.0 DIP`**, bo góc `8.0 DIP`.
   - Khi có nhạc: hiển thị chấm ảnh bìa mini (`10 x 10 DIP`) bên trái, vạch tiến độ phát nhạc mảnh (`2.0 DIP`) ở giữa, và 3 vạch sóng nhạc thu gọn bên phải (khi đang phát và bật `visualizer_enabled`).
   - Khi không có nhạc (và chưa bật `auto_hide_when_idle`): hiển thị chấm trạng thái mảnh cùng thanh chờ dịu, chạm/rê chuột vào là bung lên `Compact` (`276x36 DIP`).
2. **Quy tắc tự thu gọn sau 4 giây (`notch_collapse_after_4s`):**
   - Bật mặc định (`true`), có thể bật/tắt trong Bảng điều khiển nhanh (`Settings`), menu System Tray và lưu vào `config.ini`.
   - Khi vừa bắt đầu có phiên phát nhạc hoặc chuyển sang bài hát mới (`title` / `artist` đổi), nếu đang ở `Slim` hoặc `Compact`, Island hiển thị `Compact` trong **4 giây** để người dùng xem tên bài, sau đó tự thu về dải Notch `152x16 DIP` tại `y = 0`.
   - Trong lúc con trỏ chuột đang nằm trên Island (`mouse_tracked == true`) hoặc đang kéo timeline, không tự thu gọn; khi chuột rời đi, đếm lại 4 giây rồi rút về dải Notch (hoặc thu về ngay nếu không có nhạc).
3. **Phạm vi loại trừ (YAGI):**
   - Không thêm cơ chế giữ phím `Alt` xuyên chuột hoặc keyboard hook để giữ trải nghiệm đơn giản, gọn mã nguồn và an toàn tuyệt đối trước Windows Defender / SmartScreen.
