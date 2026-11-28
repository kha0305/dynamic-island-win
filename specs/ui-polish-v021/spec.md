# ĐẶC TẢ BẢN VÁ UI v0.2.1 (`UI-POLISH-V021`)

- **Mã tính năng (Slug):** `ui-polish-v021`
- **Ngày lập:** 2026-11-28
- **Phiên bản mục tiêu:** `0.2.1`
- **Trạng thái:** Đã hoàn tất triển khai Phase 6, chụp ảnh kiểm chứng trực tiếp và nghiệm thu Phase 7.

---

## 1. Bối cảnh và phát hiện từ khảo sát thực tế `v0.2.0`
Qua đọc mã nguồn và chụp kiểm tra trực tiếp 4 trạng thái thực tế của `dist\win-dynamic-island.exe` (`Compact`, `Expanded`, `AudioDevices`, `Settings`) trên màn hình `1920x1080`:
1. **Màn hình `Expanded` (`416 x 160 DIP`):** Nút `Cài đặt` đặt ở góc trên phải làm tiêu đề bài hát bị cắt sớm (`240 DIP`), trong khi hàng đáy bị lệch trái (`OpenAudioDevices` chỉ rộng `116 DIP`, vùng chữ `80 DIP` cắt tên loa thành `Speakers (Real...`, còn góc dưới phải rộng `116 DIP` bỏ trống).
2. **Màn hình `AudioDevices` (`424 x 284 DIP`):** Khi máy chỉ có 3 thiết bị phát, vùng bên dưới để trống một mảng đen khổng lồ (~`91 DIP`); đồng thời nút `"Quay lại"` dùng màu `accent` trích từ ảnh bìa bài hát (như màu đỏ san hô) gây lệch tông hệ thống và luôn quay về `Expanded` bất kể trước đó mở từ đâu.
3. **Màn hình `Settings` (`424 x 298 DIP`):** Các công tắc bật, chữ `"Quay lại"` và `"Đổi"` đều đổi màu theo ảnh bìa bài hát (khi phát nhạc tông đỏ làm cả bảng Cài đặt nhuốm đỏ giống nút nguy hiểm `"Thoát ứng dụng"`); 12 thẻ xếp dày chưa có tiêu đề phân nhóm, chưa hiển thị phiên bản và chưa có nút `"Kiểm tra cập nhật"` trực tiếp.
4. **Chế độ `Slim` (`152 x 16 DIP`):** Khi không có nhạc, thanh vẫn vẽ vòng tròn placeholder và rãnh tiến độ dài như thanh seek rỗng; đồng thời hàm vẽ lấy trực tiếp `input.pill` thay vì neo qua `virtual_pill_anchored`.
5. **HUD Âm lượng khi vừa đổi thiết bị (`show_device_name == true`):** Nhánh hiển thị tên thiết bị chưa vẽ knob và halo phản hồi khi rê chuột vào vùng `SeekVolume`.

---

## 2. Yêu cầu đã chốt (Phase 3 — Clarify)
1. **Cân đối hàng đáy `Expanded` & nới vùng chữ:**
   - Đưa nút `Cài đặt` (kèm icon bánh răng vector) xuống góc dưới phải đối xứng với nút `Thiết bị phát âm thanh` ở góc dưới trái; 3 nút điều khiển phát nhạc đặt chính giữa.
   - Giải phóng toàn bộ phần trên bên phải cho tiêu đề bài hát, nghệ sĩ và nguồn phát.
   - Nới rộng không gian hiển thị tên thiết bị phát ở góc dưới trái.
2. **Nâng cấp `Settings` & tách màu hệ thống:**
   - Phân nhóm rõ ràng 3 khu vực (`Hiển thị`, `HUD & Âm thanh`, `Hệ thống`), thêm thẻ mở nhanh `Thiết bị phát` và thẻ `Kiểm tra cập nhật` kèm số phiên bản (`v0.2.1`).
   - Tách màu hệ thống cố định (`sys_accent` / trung tính) cho bảng `Settings`, `AudioDevices` và nút `"Quay lại"` để không bị ảnh bìa bài hát nhuốm đỏ toàn bộ điều khiển hệ thống.
3. **Co giãn chiều cao `AudioDevices` & nhớ ngữ cảnh quay lại:**
   - Chiều cao `AudioDevices` tính linh hoạt theo số thiết bị hiển thị (`1..=5`), loại bỏ khoảng trống thừa khi ít thiết bị.
   - Nút `"Quay lại"` ở `Settings` và `AudioDevices` quay về đúng màn hình nguồn hợp lệ trước đó.
4. **Tối giản `Slim` khi rảnh & đồng bộ knob `Volume HUD`:**
   - `Slim` khi không có nhạc hiển thị viên thuốc Notch tối giản (bỏ thanh tiến độ rỗng) và neo layout qua `virtual_pill_anchored`.
   - `Volume HUD` khi hiện tên thiết bị vẫn hiển thị phản hồi knob/halo khi rê chuột qua thanh âm lượng.
5. **Phát hành `v0.2.1`:**
   - Sau khi đạt `cargo clippy --all-targets -- -D warnings`, toàn bộ `cargo test`, build `--release` và kiểm thử giao diện thực tế: nâng phiên bản lên `0.2.1`, đồng bộ `dist/` và commit + đẩy tag `v0.2.1` lên GitHub.

---

## 3. Tiêu chí nghiệm thu
- Không còn hiện tượng cắt sớm tiêu đề hoặc khoảng trống lệch bên ở hàng đáy `Expanded`; tên thiết bị hiển thị rộng hơn rõ rệt.
- `Settings` phân 3 nhóm rành mạch, kiểm tra cập nhật hoạt động trực tiếp bằng `UiAction::CheckUpdate`, màu công tắc và nút `"Quay lại"` giữ nhất quán không bị đổi sang màu đỏ theo ảnh bìa bài hát.
- `AudioDevices` co vừa số thiết bị thực tế trên máy (không chừa mảng trống lớn ở đáy).
- `Slim` lúc rảnh gọn sạch, `Volume HUD` có phản hồi hover knob kể cả khi hiện tên thiết bị.
- `cargo clippy --all-targets -- -D warnings` sạch, toàn bộ unit/integration tests đạt, bản release giữ `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `NV_MODULES=0`, `CPU_2S_MS=0`, chụp ảnh kiểm chứng trực tiếp các chế độ sau sửa trước khi đẩy `v0.2.1`.
