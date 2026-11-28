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
| Nhật ký hoạt động | `NHAT_KY_HOAT_DONG.md` | HOÀN TẤT | Đã ghi Mục #001–#032 gồm `v0.3.0`, `v0.3.1`, `v0.3.2` và **`v0.3.3`** (`#032`) |
| Sổ tay kiểm tra bug | `SO_TAY_KIEM_TRA_BUG.md` | HOÀN TẤT | Đã đối chiếu toàn bộ checklist `v0.3.3` và ghi nhận Lỗi #001–#017 |
| Đặc tả tính năng | `specs/v03-shelf-anti-refactor/spec.md` (kế thừa `specs/dynamic-island/`..`ui-polish-v021/`) | ĐÃ CHỐT | Đã hoàn tất đặc tả và nghiệm thu toàn bộ `v0.3.0`–`v0.3.3` |
| Kế hoạch & Công việc | `specs/*/plan.md` (+ `tasks.md`) | HOÀN TẤT v0.3.3 | Đã hoàn thành toàn bộ `T1.1–T1.6`, `T2.1–T2.5`, `T3.1–T3.4` và các bản vá tới `v0.3.3` |
| Mã nguồn & CI/CD | `Cargo.toml`, `.cargo/config.toml`, `src/**/*.rs`, `.github/workflows/release.yml` | ĐÃ PHÁT HÀNH v0.3.3 | `clippy --locked` sạch, `45/45` tests đạt (`0.34s`), `.exe` dung lượng `854,016 bytes`; đã đẩy nhánh `main` và tag `v0.3.3` lên `origin` |

---

## 4. Các quyết định đã chốt và bước tiếp theo (`v0.3.0`)
1. **Kiến trúc đã chốt (`v0.3.0`):** Thuần Rust Native (`Win32` + `OleInitialize`/`IDropTarget`/`IDropSource`/`IDataObject` + `ReadDirectoryChangesW` + `DWMWA_USE_HOSTBACKDROPBRUSH` + `Windows::UI::Composition HostBackdropBrush` 2 lớp cho Dynamic & Shelf + `CompositionDrawingSurface` + `Direct2D`/`DirectWrite`/`WIC`/`GSMTC`/`WASAPI`/`IPolicyConfig`/`WinHTTP`) với `[profile.release]` `lto = true`, `panic = "abort"`.
2. **Phạm vi đã hoàn thành (`v0.3.0`):**
   - **Tối ưu & tách module:** Phân rã `src/hud/`, `src/state/`, `src/renderer/`, `src/window/`, tách `src/json_mini.rs`, lọc feature `Cargo.toml`.
   - **Theo dõi Anti (`src/anti.rs`, `src/renderer/anti_view.rs`):** Theo dõi đủ 10 trạng thái cho cả `agy CLI`, `Anti IDE` và `Anti app` thông qua 1 tệp chung `%LOCALAPPDATA%\WinDynamicIsland\anti-progress.json` + cổng `--anti-hook <EVENT>` + `ReadDirectoryChangesW` (`0% CPU` khi rảnh), tự ẩn khi focus cửa sổ Anti và hiện ưu tiên kèm vòng xoay 6 chấm khi chuyển sang app khác.
   - **File Shelf (`src/shelf.rs`, `src/renderer/shelf_view.rs`):** Khay tệp gắn ngay bên phải Dynamic Island trên màn hình FHD (`MAX_SURFACE_WIDTH_DIP = 1040.0`, hợp vùng xuyên chuột `CombineRgn(RGN_OR)`), kéo thả OLE hai chiều (`CF_HDROP`), tickbox từng dòng + `"Chọn tất cả"`, ẩn nút xóa khi `selected == 0` và chỉ hiện `Delete All` khi `selected >= 1` (chỉ dọn danh sách khay, giữ nguyên tệp gốc trên đĩa).
3. **Hiện trạng vận hành & lưu ý phiên sau:** Đã đồng bộ gói `v0.3.0` vào `dist\` (không theo dõi Git vì nằm trong `.gitignore`) và xóa sạch `target\`. Phiên tiếp theo lần đầu chạy `cargo` sẽ build lại từ đầu; nếu muốn chạy thử file trong `target\`, cần thoát tiến trình đang chạy từ `dist\win-dynamic-island.exe` trước.
4. **Hướng mở rộng tiếp theo (sau `v0.3.0`):** Có thể bổ sung Pomodoro mini, theo dõi tài nguyên gọn hoặc tác vụ nhanh theo yêu cầu người dùng.
