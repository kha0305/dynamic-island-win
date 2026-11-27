# ĐẶC TẢ TÍNH NĂNG: HỆ THỐNG THÔNG BÁO DYNAMIC ISLAND (`ISLAND-NOTIFICATIONS`)

- **Mã tính năng (Slug):** `island-notifications`
- **Ngày lập:** 2026-11-27
- **Trạng thái:** Hoàn tất triển khai Phase 6 & kiểm thử thực tế Phase 7.

---

## 1. Bối cảnh và mục tiêu
- Trong `src/window.rs` (`show_deferred_update_modal`), mọi hộp thoại kết quả kiểm tra cập nhật (`Bạn đang sử dụng phiên bản mới nhất`), xác nhận cài đặt (`MB_YESNO`), và cảnh báo lỗi (`MB_ICONERROR`) vẫn dùng `MessageBoxW` mặc định của Windows ở giữa màn hình, không đồng bộ với giao diện `Liquid Glass` và chiếm tiêu điểm cửa sổ.
- Tính năng `island-notifications` **loại bỏ hoàn toàn `MessageBoxW` khỏi toàn bộ mã nguồn ứng dụng**, chuyển toàn bộ thông báo trạng thái cập nhật và lỗi sang thẻ `Liquid Glass` trực tiếp trên Dynamic Island (`IslandMode::Notice`).

---

## 2. Quyết định đã chốt (Phase 3 — Clarify)
1. **Hành vi khi phát hiện bản cập nhật lúc khởi động (~10s sau khi mở):**
   - Nếu đang ở desktop bình thường (không đang mở `Settings`/`AudioDevices` và không ở chế độ toàn màn hình), tự động bung thẻ `"Có bản cập nhật mới"` ngay trên Dynamic Island để người dùng có thể bấm `"Cập nhật ngay"` hoặc `"Để sau"`. Nếu đang fullscreen (game/video), giữ im ở Tray để không làm gián đoạn.
2. **Kích thước và bố cục thẻ thông báo đồng nhất (`416 x 136 DIP`):**
   - Tất cả thông báo cập nhật/xác nhận/lỗi đều dùng chung bố cục thẻ `Liquid Glass` `416 x 136 DIP` (`IslandMode::Notice`, `corner_radius = 26.0`, `TOP_MARGIN_DIP = 6.0`):
     - Huy hiệu tròn bên trái (`40 x 40 DIP`) kèm icon vector Direct2D và màu phân biệt theo loại thông báo (`power_ok` xanh ngọc cho Đã mới nhất, `accent` xanh dương cho Đang kiểm tra / Có bản mới / Đang tải, `danger_border` đỏ cho Lỗi).
     - Tiêu đề + nội dung mô tả tự ngắt dòng (`DWRITE_WORD_WRAPPING_WRAP`, 2–3 dòng không bị cắt chữ).
     - Một hoặc hai nút hành động bo tròn phía dưới (`Đã hiểu`, `Thu gọn`, `Cập nhật ngay` / `Để sau`, `Thử lại` / `Đóng`).
     - Thông báo thông tin thường (`UpToDate`) tự động ẩn sau `4.0 giây` nếu chuột không giữ trên thẻ; thông báo xác nhận và lỗi giữ nguyên cho đến khi người dùng bấm chọn nút.

---

## 3. Tiêu chí nghiệm thu
- Không còn bất kỳ lời gọi `MessageBoxW` nào trong mã nguồn (`src/*.rs`).
- Bấm `"Kiểm tra cập nhật"` từ Tray hiện ngay thẻ `"Đang kiểm tra cập nhật..."` rồi chuyển mượt sang `"Đã là bản mới nhất"`, `"Có bản cập nhật mới"`, hoặc `"Không thể cập nhật"` kèm nút xử lý tương ứng.
- Thông báo lỗi dài hiển thị trọn vẹn nhiều dòng, không tràn khung và không trùng đè hitbox.
- Đáp ứng sạch `cargo clippy --all-targets -- -D warnings`, unit tests và đo kiểm `--release` (`0% CPU` lúc rảnh, không đánh thức GPU NVIDIA, `EMOJI_HITS=0`).
