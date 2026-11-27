# ĐẶC TẢ DỰ ÁN: WIN DYNAMIC ISLAND (PHASE 2 - SPECIFY)

- **Mã tính năng (Slug):** `dynamic-island`
- **Ngày lập:** 2026-11-27
- **Trạng thái:** Đã chốt Phase 3 & duyệt Plan Phase 4 — bắt đầu triển khai MVP v0.1.

---

## 0. Quyết định đã chốt (Phase 3 — Clarify)
1. **Kiến trúc được chọn:** Hướng 1 — Thuần Rust Native (`Win32 API` + `DirectComposition` + `DXGI Flip-Model SwapChain` + `Direct2D` + `DirectWrite` + `WIC`) thông qua duy nhất crate `windows` và thư viện chuẩn Rust (`std`). Không dùng Tauri v2, WebView2 hay `egui`/`wgpu`.
2. **Tối ưu phần cứng Hybrid GPU:** Ép chọn bộ xử lý đồ họa tiết kiệm điện (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`) trên iGPU AMD Radeon(TM) Graphics, không đánh thức card rời NVIDIA RTX 3050; đồng bộ animation theo VSync/Compositor Clock khi chuyển động và ngắt vòng lặp render về 0% khi đứng yên.
3. **Phạm vi MVP v0.1:** Thực hiện đầy đủ Giai đoạn 1 (Viên thuốc nổi cạnh trên với vật lý lò xo bảo toàn vận tốc, điều khiển nhạc Windows GSMTC, tự ẩn khi Fullscreen, đa màn hình & DPI Scaling, System Tray tiếng Việt).
4. **Trạng thái mặc định khi rảnh:** Thanh viên thuốc siêu mảnh sát mép trên (Slim) để không che thanh tab trình duyệt, kèm tùy chọn bật/tắt chế độ tự ẩn hoàn toàn khi không phát nhạc trong menu System Tray.

---

## 1. Bài toán và mục tiêu
Xây dựng một ứng dụng thanh viên thuốc nổi (Dynamic Island) đặt tại cạnh trên màn hình cho Windows 11, viết bằng **Rust**, mô phỏng trải nghiệm mượt mà như Dynamic Island / NotchNook trên macOS nhưng tối ưu cho màn hình phẳng của Windows:
- Gọn nhẹ, chạy ngầm ít tốn RAM/CPU, không đánh thức GPU rời khi chờ.
- Hiển thị thông tin tức thời (nhạc đang phát, trạng thái phần cứng, khay kéo thả tệp tạm, hẹn giờ) mà không chắn tầm nhìn hay cản trở thanh tab trình duyệt.

---

## 2. Yêu cầu chức năng (chia 3 giai đoạn)

### Giai đoạn 1 — Bản lõi MVP (v0.1)
1. **Cửa sổ viên thuốc nổi trên cùng (Floating Pill Overlay):**
   - Nằm căn giữa mép trên màn hình chính (hoặc màn hình có con trỏ).
   - Bo tròn hai đầu (capsule/pill), có animation co giãn mượt theo mô hình lò xo (spring physics).
   - Hai chế độ chính: **Thu gọn (Compact)** và **Mở rộng (Expanded)** khi rê chuột hoặc bấm vào.
   - Hỗ trợ **Chế độ ngủ mảnh (Slim/Auto-hide)** sát cạnh trên để không che tab trình duyệt khi không dùng.
2. **Trình điều khiển nhạc đang phát (Live Media Pill):**
   - Tích hợp Windows GSMTC (`GlobalSystemMediaTransportControls`) để tự động bắt thông tin phát nhạc từ Spotify, Chrome, Edge, VLC...
   - Hiển thị tên bài hát, nghệ sĩ, ảnh bìa mini và nút Play / Pause / Next / Previous.
3. **Biểu tượng khay hệ thống (System Tray):**
   - Có menu chuột phải dưới khay hệ thống: Ẩn/Hiện nhanh, bật/tắt tự khởi động cùng Windows, chọn màn hình hiển thị, Thoát an toàn.

### Giai đoạn 2 — Tiện ích làm việc (v0.2)
4. **HUD thay thế thông báo Windows:**
   - Bung thông báo gọn khi thay đổi âm lượng, tắt/bật micro, chuyển trạng thái `CapsLock`, cắm/rút sạc hoặc kết nối thiết bị Bluetooth.
5. **Khay thả tệp tạm (File Shelf / Notch Drop):**
   - Hỗ trợ kéo tệp hoặc ảnh lên mép trên màn hình để giữ tạm trong Island, sau đó kéo từ Island thả vào ứng dụng đích (Discord, Zalo, trình duyệt, Explorer).
6. **Đồng hồ đếm ngược / Pomodoro mini:**
   - Cuộn chuột hoặc chọn nhanh khoảng thời gian 5–25 phút, hiện số đếm ngược nhỏ gọn ở trạng thái thu gọn.

### Giai đoạn 3 — Mở rộng cá nhân hóa (v0.3)
7. **Theo dõi phần cứng tối giản:** Hiển thị nhanh % CPU, RAM, tốc độ mạng khi mở rộng.
8. **Bảng chạy nhanh (Quick Actions):** Cho phép cấu hình phím tắt / nút mở nhanh các công cụ cá nhân hoặc ghi chú nhanh.

---

## 3. Yêu cầu phi chức năng & Ràng buộc kỹ thuật Windows 11
- **Cửa sổ hệ thống chuẩn:** Sử dụng cờ `WS_EX_TOOLWINDOW` + `WS_EX_TOPMOST` để ẩn khỏi Taskbar và `Alt + Tab`.
- **Không chắn chuột vùng rỗng:** Khung cửa sổ khớp sát kích thước viên thuốc (hoặc bật click-through cho vùng nền trong suốt).
- **Tự ẩn khi Fullscreen:** Tự động phát hiện ứng dụng toàn màn hình (game, video fullscreen) để ẩn Island.
- **Đa luồng an toàn:** Toàn bộ việc lắng nghe GSMTC, sự kiện âm thanh/nguồn điện và giám sát fullscreen chạy trên luồng nền; luồng giao diện chỉ nhận sự kiện cập nhật để vẽ lại khi cần.
- **Không emoji, tiếng Việt chuẩn:** Toàn bộ giao diện và log tuân thủ tuyệt đối quy chuẩn `GEMINI.md`.

---

## 4. Hai phương án kiến trúc chờ duyệt (Phase 3 -> Phase 4)

### Phương án A — Thuần Rust (Win32 API + `egui` / `iced` hoặc Direct2D/DirectComposition)
- **Ưu điểm:** Không cần WebView2 hay `node_modules/`, tệp `.exe` gọn, kiểm soát trực tiếp cửa sổ Win32, DPI và tối ưu năng lượng cho laptop Hybrid GPU.
- **Nhược điểm:** Cần tự tinh chỉnh thẩm mỹ bo tròn, hiệu ứng mờ nền (Acrylic/Mica) và animation bằng code Rust.

### Phương án B — Rust Backend + Web UI (Tauri v2)
- **Ưu điểm:** Thiết kế giao diện bo tròn, blur, hiệu ứng lò xo cực nhanh bằng CSS/HTML/JS.
- **Nhược điểm:** Tiêu tốn RAM cao hơn (do WebView2), phát sinh `node_modules/` và thư mục `target/` lớn khi phát triển.

---

## 5. Tiêu chí nghiệm thu cho MVP (v0.1)
- [x] Khởi chạy không hiện trên Taskbar và `Alt + Tab`.
- [x] Căn giữa đúng mép trên màn hình với các mức DPI khác nhau.
- [x] Tự động nhận diện và điều khiển được bài hát đang phát từ trình duyệt hoặc Spotify qua GSMTC.
- [x] Co giãn mượt khi hover/click và tự ẩn khi có cửa sổ toàn màn hình.
- [x] Thoát ứng dụng sạch sẽ qua System Tray không để lại tiến trình treo.
