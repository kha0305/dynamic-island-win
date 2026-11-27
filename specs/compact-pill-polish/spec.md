# ĐẶC TẢ TÍNH NĂNG: NÂNG CẤP GIAO DIỆN THANH THU GỌN (`COMPACT-PILL-POLISH`)

- **Mã tính năng (Slug):** `compact-pill-polish`
- **Ngày lập:** 2026-11-27
- **Trạng thái:** Hoàn tất triển khai Phase 6 & kiểm thử thực tế Phase 7.

---

## 1. Bối cảnh và mục tiêu
- Trong thanh thu gọn `Compact` (`276 x 36 DIP`), khi phát nhạc có tiền tố tên dài hoặc khi đang Tạm dừng, bố cục 1 dòng hiện tại tạo khoảng trống thừa ở bên phải, sóng nhạc 3 cột thụt khá xa mép phải và ảnh bìa dùng bo góc `7 DIP` kèm viền đen cứng chưa đồng tâm với cung tròn `18 DIP` của viên thuốc.
- Tính năng `compact-pill-polish` nâng cấp bố cục `Compact` lên dạng **2 tầng gọn đẹp + thanh tiến độ mảnh + ảnh bìa tròn đồng tâm + sóng nhạc 5 vạch cân xứng hai đầu**, **giữ nguyên hoàn toàn hiệu ứng kính hòa màu theo hình nền/background hiện có**.

---

## 2. Quyết định đã chốt (Phase 3 — Clarify)
1. **Bố cục 2 tầng gọn đẹp (`316 x 40 DIP`, `corner_radius = 20.0`, `top = 2.0`):**
   - **Tầng trên:** Tên bài hát (`12.0pt SemiBold`, trắng sáng, đủ rộng hơn hạn chế cắt chữ).
   - **Tầng dưới:** Tên Nghệ sĩ / Nguồn phát (`10.0pt`, dịu nhẹ) kèm thanh tiến độ phát nhạc siêu mảnh (`2.0 DIP`) màu `accent` ngay sát bên dưới phần chữ.
   - Giữ `HUD Âm lượng` và `HUD Pin / Sạc` đồng bộ cùng kích thước `316 x 40 DIP` để chuyển đổi mượt không giật khung.
2. **Ảnh bìa bo tròn đồng tâm & cụm sóng nhạc cân hai đầu:**
   - Ảnh bìa `26 x 26 DIP` bo tròn hoàn toàn (`radius = 13.0`, đồng tâm với cung tròn `20.0` của viên thuốc khi cách mép đều `7.0 DIP`), viền highlight kính mảnh tinh tế thay cho viền đen cứng.
   - Đầu phải hiển thị **sóng nhạc 5 vạch mượt** đặt cân sát cung phải khi đang phát, và tự chuyển sang **icon Tạm dừng `||` nhỏ dịu** khi tạm dừng để viên thuốc luôn cân đối hai đầu.

---

## 3. Tiêu chí nghiệm thu
- Thanh `Compact` giữ nguyên vị trí `top = 2.0 DIP` (`compact_rgn.top == 0` không che thanh tab), kích thước `316 x 40 DIP`, ảnh bìa tròn đồng tâm không cấn mép, không trống một bên khi phát lẫn khi tạm dừng.
- Thanh tiến độ mảnh dưới `Compact` cập nhật đều ngay cả khi tắt hiệu ứng sóng nhạc động (khi đang phát), và dừng tick hoàn toàn (`0% CPU`) khi tạm dừng hoặc không có session.
- `cargo clippy --all-targets -- -D warnings`, toàn bộ unit tests (bao gồm test pixel kính khói trên `#FFFFFF` và hitbox HUD âm lượng) và đo kiểm `--release` đạt sạch.
