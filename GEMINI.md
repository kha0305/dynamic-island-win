# LUẬT RIÊNG DỰ ÁN WIN DYNAMIC ISLAND (WINDOWS 11 + RUST)

> Tệp luật này áp dụng riêng cho dự án Dynamic Island trên Windows 11 tại thư mục `C:\Users\ASUS\Desktop\New folder\win-dynamic-island`.
> Kế thừa đầy đủ bộ quy chuẩn toàn cục tại `C:\Users\ASUS\.gemini\GEMINI.md` và `C:\Users\ASUS\.gemini\config\rules\global_rules.md`.
> Sau khi cập nhật tệp luật này, mở phiên trò chuyện mới tại thư mục gốc dự án để hệ thống nạp trọn vẹn quy tắc và ngữ cảnh.

---

## 1. Thông tin môi trường đã kiểm chứng
- **Hệ điều hành:** Microsoft Windows 11 Pro (Version `10.0.26300.0`, Build `26300`)
- **Phần cứng đồ họa:** Laptop hai GPU — AMD Radeon(TM) Graphics + NVIDIA GeForce RTX 3050 Laptop GPU
- **Toolchain Rust có sẵn:** `rustc 1.100.0 (0ef1bc386 2026-11-09)`, `cargo 1.100.0 (3acc79c0c 2026-11-05)`
- **Trạng thái công nghệ giao diện:** Chưa chốt giữa Hướng 1 (Thuần Rust: Win32 + `egui` / `iced` / DirectComposition) và Hướng 2 (Tauri v2). Bắt buộc xác nhận với người dùng tại Phase 3/4 trước khi tạo `Cargo.toml` và viết mã nguồn.

---

## 2. Nghi thức bắt buộc khi bắt đầu phiên làm việc
1. **Đọc đủ ngữ cảnh trước khi hành động:** Phải đọc `GEMINI.md`, `NHAT_KY_HOAT_DONG.md`, `SO_TAY_KIEM_TRA_BUG.md`, `TASK_MEMORY.md` và `specs/dynamic-island/spec.md` trước khi đề xuất phương án hay chỉnh sửa bất kỳ tệp nào.
2. **Dòng xác nhận đầu phiên:** Câu trả lời đầu tiên trong phiên mới phải có dòng:
   `Xác nhận: Đã đọc và tuân thủ các quy tắc dự án tại C:\Users\ASUS\Desktop\New folder\win-dynamic-island\GEMINI.md`
3. **Không hỏi lại thông tin đã ghi:** Kế thừa trực tiếp tiến độ và quyết định từ `TASK_MEMORY.md` và `NHAT_KY_HOAT_DONG.md`.

---

## 3. Ngôn ngữ, trình bày và kỷ luật thực thi
- **Tuyệt đối không sử dụng Emoji:** Cấm hoàn toàn emoji trong mã nguồn, chuỗi UI, nút bấm, nhật ký, tài liệu và phản hồi trao đổi.
- **Tiếng Việt có dấu chuẩn xác:** Mọi văn bản và chuỗi giao diện tiếng Việt phải có dấu đầy đủ, đúng ngữ pháp và thuật ngữ kỹ thuật chuẩn xác.
- **Ghi chú trước và sau mỗi thao tác:**
  - Trước khi tạo hoặc sửa mỗi tệp: ghi rõ `Mục tiêu kỹ thuật` và `Kế hoạch hành động`.
  - Làm cuốn chiếu từng hạng mục, kiểm thử đạt mới sang hạng mục tiếp theo.
- **Giới hạn không gian làm việc:** Chỉ thao tác bên trong `C:\Users\ASUS\Desktop\New folder\win-dynamic-island`. Tuyệt đối không can thiệp tệp hoặc Registry ngoài dự án trừ khi được yêu cầu rõ ràng.

---

## 4. Quy trình phát triển tính năng (Spec-First & Ponytail)
Áp dụng đủ 7 bước cho mọi tính năng mới hoặc thay đổi kiến trúc:
1. **Phase 1 — Constitution:** Đọc lại `GEMINI.md`.
2. **Phase 2 — Specify:** Cập nhật đặc tả vào `specs/<slug>/spec.md`.
3. **Phase 3 — Clarify:** Hỏi tối đa 5 câu làm rõ các điểm mơ hồ (hiện tại cần chốt hướng UI: Thuần Rust hay Tauri v2, và phạm vi MVP).
4. **Phase 4 — Plan:** Lập kế hoạch kiến trúc, danh sách tệp sẽ tạo/sửa và **DỪNG LẠI CHỜ DUYỆT**. Không viết code trước khi người dùng duyệt.
5. **Phase 5 — Tasks:** Chia nhỏ đầu việc độc lập.
6. **Phase 6 — Implement:** Viết mã gọn tối thiểu theo thang Ponytail, ưu tiên API gốc của Windows (`windows-rs`) và stdlib Rust trước khi thêm crate ngoài.
7. **Phase 7 — Converge:** Biên dịch kiểm tra (`cargo check` / `cargo test`), chạy thử giao diện thực tế, đối chiếu `SO_TAY_KIEM_TRA_BUG.md`, cập nhật `NHAT_KY_HOAT_DONG.md`.

---

## 5. Quy chuẩn kỹ thuật đặc thù cho Dynamic Island trên Windows 11
- **Không làm lag GPU rời trên laptop Hybrid (AMD + NVIDIA):** Tuyệt đối không khởi tạo ngữ cảnh đồ họa nặng gây đánh thức card rời NVIDIA RTX 3050 liên tục khi ứng dụng chỉ đang ở chế độ chờ (idle); ưu tiên dùng GPU tích hợp hoặc cấu hình nhẹ tiết kiệm điện.
- **Không chiếm quyền giao diện / Không che khuất thao tác:**
  - Cửa sổ phải dùng cờ `WS_EX_TOOLWINDOW` và `WS_EX_TOPMOST` để không hiện trên Taskbar và danh sách `Alt + Tab`.
  - Vùng trống xung quanh viên thuốc phải cho phép xuyên chuột (click-through) hoặc vừa khít kích thước bo tròn của viên thuốc để không chắn thanh tab trình duyệt.
  - Phải tự động ẩn khi phát hiện ứng dụng foreground đang chạy toàn màn hình (game, video fullscreen).
- **Quản lý tài nguyên và độ tin cậy:**
  - Luôn đóng/giải phóng Win32 Handle, COM/WinRT session, Timer và Hook khi thoát hoặc dựng lại cửa sổ.
  - Không đọc/poll nặng trên luồng UI chính (UI thread); mọi tác vụ lắng nghe GSMTC Media, theo dõi phần cứng hoặc kéo thả tệp phải chạy bất đồng bộ qua kênh truyền tin an toàn (mpsc / channel) và chỉ đánh thức vẽ lại khi trạng thái thật sự thay đổi.
  - Không dùng `unwrap()` / `expect()` bừa bãi trên luồng chạy chính với dữ liệu từ hệ điều hành (tên bài hát, ảnh bìa rỗng, thiết bị ngắt kết nối đột ngột).
- **Kiểm soát dung lượng build:** Tuân thủ `.gitignore`, không giữ file tạm; nếu tạm dừng phát triển dài ngày nên dọn `target/` (giữ lại tệp `.exe` bản release nếu cần) để tránh phình ổ cứng.

---

## 6. Trung thực và kiểm chứng
- Không bịa đặt tên crate, phiên bản crate, API Win32/WinRT hay kết quả chạy thử.
- Không tuyên bố số liệu hiệu năng (RAM, CPU, FPS) nếu chưa đo kiểm thực tế trên bản build thật.
- Trước khi báo hoàn thành bất kỳ hạng mục nào, bắt buộc biên dịch sạch, chạy kiểm thử và đối chiếu toàn bộ mục trong `SO_TAY_KIEM_TRA_BUG.md`.
