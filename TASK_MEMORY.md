# BỘ NHỚ TRẠNG THÁI DỰ ÁN & NGỮ CẢNH (TASK MEMORY)

> Tài liệu này lưu giữ toàn bộ bối cảnh hình thành dự án, thông số môi trường thật trên máy và điểm dừng hiện tại theo Quy tắc 22 (Session Continuity) của `global_rules.md`.

---

## 1. Bối cảnh hình thành dự án (Conversation Context)
- **Nhu cầu gốc của người dùng:** Muốn tự làm một công cụ bằng **Rust** trên máy Windows cá nhân; sau khi trao đổi các hướng ý tưởng, người dùng đã chốt ý tưởng làm **Dynamic Island cho Windows 11** (mô phỏng trải nghiệm viên thuốc / Notch thông minh trên iOS và macOS như NotchNook / Alcove / BoringNotch).
- **Lý do kỹ thuật được lưu ý từ môi trường máy:**
  - Người dùng ưu tiên phần mềm gọn nhẹ, hạn chế phát sinh hàng chục GB thư mục biên dịch rác (`target/`, `node_modules/`).
  - Trên máy đang chạy song song hai card đồ họa (AMD Radeon tích hợp + NVIDIA RTX 3050 Laptop GPU), nên ứng dụng chạy nền cần thiết kế tối ưu để không đánh thức card rời NVIDIA khi đang chờ.
  - Người dùng cũng hay sử dụng các công cụ cục bộ trên máy (như app dịch màn hình `D:\tool\dich-AI`, công cụ `GoodbyeDPI` trong `C:\Users\ASUS\Desktop\New folder`, các launcher/script tiện ích), có thể tích hợp ở Giai đoạn 3 nếu cần.

---

## 2. Thông số hệ thống đã kiểm chứng (Verified Environment)
- **Thư mục gốc dự án:** `C:\Users\ASUS\Desktop\New folder\win-dynamic-island`
- **Hệ điều hành:** Microsoft Windows 11 Pro (Version `10.0.26300.0`, Build `26300`)
- **Trình biên dịch Rust:**
  - `rustc 1.100.0 (0ef1bc386 2026-11-09)`
  - `cargo 1.100.0 (3acc79c0c 2026-11-05)`
- **Đồ họa:** AMD Radeon(TM) Graphics + NVIDIA GeForce RTX 3050 Laptop GPU

---

## 3. Trạng thái các tệp và pha phát triển
| Thành phần | Đường dẫn | Trạng thái | Ghi chú |
| :--- | :--- | :--- | :--- |
| Luật riêng dự án | `GEMINI.md` | HOÀN TẤT | Kế thừa `global_rules.md` + ràng buộc Win32/Rust |
| Nhật ký hoạt động | `NHAT_KY_HOAT_DONG.md` | HOÀN TẤT | Đã ghi Mục #001–#012 gồm bản Portable và Tự cập nhật GitHub Releases |
| Sổ tay kiểm tra bug | `SO_TAY_KIEM_TRA_BUG.md` | HOÀN TẤT | Đã đối chiếu toàn bộ checklist và ghi nhận 7 lỗi đã xử lý |
| Đặc tả tính năng | `specs/dynamic-island/spec.md`, `specs/auto-update/spec.md` | ĐÃ CHỐT | Đã chốt Hướng 1 thuần Rust, MVP v0.1 và cơ chế Tự cập nhật GitHub Releases |
| Kế hoạch & Công việc | `specs/dynamic-island/plan.md`, `tasks.md` | HOÀN TẤT v0.1.0 | Toàn bộ task MVP + Liquid Glass + Portable + Auto-Update đã kiểm chứng |
| Mã nguồn & CI/CD | `Cargo.toml`, `.cargo/config.toml`, `src/*.rs`, `.github/workflows/release.yml` | HOÀN TẤT v0.1.0 | 25/25 tests đạt, clippy sạch, `.exe` `724.5 KB` (`+crt-static`, `VCRUNTIME_DEP=False`), sẵn sàng đẩy GitHub |

---

## 4. Các quyết định đã chốt và bước tiếp theo (v0.2)
1. **Kiến trúc đã chốt (v0.1.0):** Thuần Rust Native (`Win32` + `DWMWA_USE_HOSTBACKDROPBRUSH` + `Windows::UI::Composition HostBackdropBrush` qua `CLSID_D2D1GaussianBlur` / fallback `DirectComposition` + `DXGI iGPU` + `Direct2D` + `DirectWrite` + `WIC` + `GSMTC` + `WinHTTP`) chỉ dùng crate `windows = "0.62.2"`, `windows-core = "0.62.2"` và `windows-numerics = "0.3.1"`.
2. **Phạm vi đã hoàn thành (v0.1.0):** Viên thuốc nổi cạnh trên co giãn lò xo VSync, điều khiển nhạc GSMTC kèm ảnh bìa, Liquid Glass đồng màu video, tự thu gọn 5 giây, bảng điều khiển trực quan ngay trên Island, menu System Tray phân nhóm kèm submenu đa màn hình, tự ẩn Fullscreen, lưu cấu hình, Autostart, Single-Instance, đóng gói Portable không phụ thuộc VCRUNTIME và Tự cập nhật qua GitHub Releases (`WinHTTP` + `SHA-256`) kèm GitHub Actions CI.
3. **Hướng mở rộng v0.2:** Khi người dùng yêu cầu, sẽ mở rộng theo spec cho HUD âm lượng/pin/CapsLock, File Shelf kéo thả tệp tạm và Pomodoro mini.
