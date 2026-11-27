# ĐẶC TẢ TÍNH NĂNG: HUD ÂM LƯỢNG VÀ PIN / NGUỒN SẠC (`SYSTEM-HUD`)

- **Mã tính năng (Slug):** `system-hud`
- **Ngày lập:** 2026-11-27
- **Trạng thái:** Hoàn tất triển khai Phase 6 & kiểm thử thực tế Phase 7 (sẵn sàng phát hành `v0.2.0`).

---

## 1. Bối cảnh và mục tiêu
- Bổ sung dải HUD nổi tức thời (~`2.0 giây`) ngay trên Dynamic Island khi người dùng thay đổi âm lượng hệ thống hoặc thay đổi trạng thái nguồn điện/pin laptop, giữ ngôn ngữ `Liquid Glass` kính khói đồng nhất và tự lướt mượt về trạng thái trước đó.
- Đáp ứng nguyên tắc hiệu năng `GEMINI.md`: dùng sự kiện gốc Windows (`WASAPI` callback + `WM_POWERBROADCAST`), tuyệt đối `0% CPU` khi đứng yên, không dùng keyboard/mouse hook toàn cục (`WH_KEYBOARD_LL` / `WH_MOUSE_LL`) và không đánh thức GPU rời NVIDIA.

---

## 2. Yêu cầu chức năng đã chốt tại Phase 3
1. **Phạm vi sự kiện hỗ trợ:**
   - **HUD Âm lượng:** Theo dõi mức âm lượng tổng (`0..100%`) và trạng thái tắt tiếng (`Mute`) qua Windows Audio (`WASAPI` `IAudioEndpointVolumeCallback` + `IMMNotificationClient` để tự theo kịp khi đổi loa/tai nghe mặc định).
   - **HUD Pin / Sạc:** Theo dõi qua `WM_POWERBROADCAST` + `GetSystemPowerStatus`: bật HUD khi **cắm sạc**, **rút sạc**, hoặc khi pin xả xuống **ngưỡng yếu (`<= 15%`, và nhắc lại một lần ở `<= 7%`)**; không hiện lặp lại mỗi khi giảm 1% bình thường.
   - **Loại trừ rõ ràng (YAGI):** Không làm HUD `Caps Lock` / `Num Lock`, không làm HUD độ sáng WMI, không chèn hook ẩn cửa sổ hệ thống Windows 11.
2. **Quy tắc hiển thị & ưu tiên trạng thái:**
   - Khi có sự kiện HUD mới, nếu Island đang ở `Slim` (`152 x 16 DIP`), `Compact` (`276 x 36 DIP`), hoặc đang ngủ tạm do bật `Tự ẩn khi rảnh` (`auto_hide_when_idle`), Island bung dải `Compact` (`276 x 36 DIP`, `top = 2.0 DIP`) để hiển thị HUD trong **`2.0 giây`**, sau đó tự trở về đúng chế độ cũ (`Slim` / `Hidden` / `Compact` nhạc).
   - Nếu đang mở `Expanded` hoặc `Bảng điều khiển nhanh` (`Settings`), **không** nhảy đè HUD để tránh gián đoạn thao tác kéo timeline hoặc cấu hình.
   - Nếu `auto_hide_fullscreen` đang ẩn do toàn màn hình, hoặc người dùng đã chủ động tắt hiển thị (`Ẩn tạm`), giữ nguyên trạng thái ẩn.
3. **Tương tác trực tiếp trên viên thuốc:**
   - **Kéo thanh âm lượng & Bấm tắt tiếng:** Khi `HUD Âm lượng` đang mở, người dùng có thể bấm nút loa bên trái để Bật/Tắt tiếng (`ToggleMute`), hoặc bấm giữ kéo mượt trên thanh tiến độ (`SetCapture`) để chỉnh từ `0%` tới `100%`.
   - **Cuộn con lăn chuột (`WM_MOUSEWHEEL` + bộ tích lũy `WHEEL_DELTA`):** Khi bật `hud_wheel_enabled` và `hud_volume_enabled`, nếu rê chuột lên viên thuốc (`Slim` / `Compact` / `HUD`), cuộn con lăn (cả chuột nấc `120` lẫn touchpad mượt phân đoạn nhỏ) sẽ tích lũy chính xác theo `WHEEL_DELTA`, tăng/giảm âm lượng (`2%` mỗi `120` đơn vị), tự bật hoặc gia hạn `HUD Âm lượng` thêm `2.0 giây`, và chặn trôi trang bên dưới khi `mode != Expanded && mode != Settings`.
   - **Giữ HUD khi đang tương tác:** Trong lúc chuột đang nằm trên `HUD` (`mouse_tracked`) hoặc đang kéo thanh âm lượng, không tự đóng; sau khi chuột rời đi, đếm lại `2.0 giây` rồi mới thu gọn.
4. **Tùy chọn cấu hình:**
   - Ba công tắc độc lập trong `Bảng điều khiển nhanh` (`Settings`), menu System Tray và lưu trong `config.ini`: `HUD Âm lượng` (`hud_volume_enabled`, mặc định `true`), `Cuộn chỉnh loa` (`hud_wheel_enabled`, mặc định `true`), và `HUD Pin / Sạc` (`hud_power_enabled`, mặc định `true`).

---

## 3. Tiêu chí nghiệm thu
- Tăng/giảm âm lượng bằng phím laptop hoặc cuộn chuột trên viên thuốc làm HUD nổi mượt ở `276 x 36 DIP`, hiển thị đúng % và tự thu sau `2.0 giây`.
- Kéo thanh âm lượng trên HUD mượt liên tục kể cả khi con trỏ trượt ngang qua mép thanh, không thu nhỏ nhầm, giải phóng capture an toàn.
- Cắm/rút sạc bật HUD Pin đúng trạng thái sạc và %; rút lại khi cắm ổn định không spam theo từng %.
- Đang ở `Expanded` hoặc `Settings` không bị HUD nhảy đè.
- Đáp ứng sạch `cargo clippy --all-targets -- -D warnings`, unit tests và `0% CPU` lúc chờ.
