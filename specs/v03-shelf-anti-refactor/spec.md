# ĐẶC TẢ DỰ ÁN: FILE SHELF, THEO DÕI ANTI & TỐI ƯU MODULE (`v0.3.0`)

- **Mã tính năng (Slug):** `v03-shelf-anti-refactor`
- **Ngày lập:** 2026-11-28
- **Phiên bản mục tiêu:** `0.3.0`
- **Trạng thái:** ĐÃ HOÀN TẤT NGHIỆM THU (`v0.3.0`).

---

## 1. Constitution (Phase 1 — Nguyên tắc bất biến theo `GEMINI.md`)
1. **Không emoji, tiếng Việt chuẩn xác:** Toàn bộ mã nguồn, giao diện, menu, tài liệu và thông báo không chứa emoji và sử dụng tiếng Việt có dấu chuẩn xác.
2. **Thuần Rust Native, tiết kiệm phần cứng Hybrid GPU:** Chỉ dùng Rust `std` và crate `windows` (chọn iGPU `AMD Radeon`, `VCRUNTIME_DEP=False`, `0% CPU` khi đứng yên, không đánh thức card rời NVIDIA RTX 3050).
3. **Không chặn UI thread & không che khuất thao tác:** Theo dõi tệp, hệ thống tiến độ và xử lý kéo thả chạy qua cơ chế sự kiện/luồng nền an toàn; vùng trống ngoài viên thuốc xuyên chuột qua `SetWindowRgn`, tự ẩn khi Fullscreen và giữ `WS_EX_TOOLWINDOW | WS_EX_TOPMOST`.
4. **Không tự ý sửa tệp ngoài không gian dự án:** Việc cài/gỡ hook `~/.gemini/config/hooks.json` chỉ thực hiện khi người dùng chủ động kích hoạt trong ứng dụng hoặc đồng ý rõ ràng.
5. **Spec-First & Ponytail:** Phải trình bản kế hoạch `plan.md` để người dùng duyệt ở Phase 4 trước khi bắt tay vào tách/sửa mã nguồn.

---

## 2. Quyết định đã chốt của người dùng (Phase 3 — Clarify)
1. **Hạng mục 1 (`File Shelf`) — Vị trí & kéo thả:** Đặt **khay thả mini dính ngay cạnh phải Dynamic Island** ở mép trên màn hình FHD (`1920x1080`): kéo tệp từ Explorer/app khác thả vào để giữ tạm, rê chuột/bấm để bung danh sách tệp, và kéo ngược từ danh sách thả ra Zalo/Chrome/Explorer qua OLE Drag-Drop.
2. **Hạng mục 1 (`File Shelf`) — Cơ chế `tickbox` & `Delete All`:** Mỗi dòng tệp có **tickbox riêng** (kèm tickbox `"Chọn tất cả"`); khi chưa tick mục nào thì nút xóa ẩn hoàn toàn, chỉ khi có **ít nhất 1 mục được tick** mới hiện nút `Delete All` — thao tác này **chỉ dọn khỏi khay tạm**, giữ nguyên tuyệt đối tệp gốc trên ổ đĩa.
3. **Hạng mục 2 (`Theo dõi Anti`) — Kiến trúc 1 tệp chung không cần API:** Dùng cơ chế **Global Hooks (`~/.gemini/config/hooks.json`)** (cả `agy CLI`, `Anti IDE` và `Anti app` đều dùng chung thư mục `~/.gemini/config/`) ghi nhận trực tiếp 10 trạng thái vào **1 tệp tiến độ JSON duy nhất cục bộ**, kèm theo dõi bằng Win32 `ReadDirectoryChangesW` (có tùy chọn cài/gỡ hook an toàn trong `Settings` khi người dùng cho phép).
4. **Hạng mục 2 (`Theo dõi Anti`) — Ngữ cảnh ứng dụng & ưu tiên Nhạc:** Chỉ hiện tiến độ Anti khi người dùng **chuyển sang ứng dụng khác** (khi đang focus ngay trong `agy` / `Anti IDE` / `Anti app` thì không làm phiền). Khi vừa nghe nhạc vừa chạy Anti ở app khác, **ưu tiên hiển thị tiến độ Anti** khi tác vụ đang chạy, còn trạng thái kết thúc (`Đã xong` / `Bị lỗi` / `Đã hủy`) giữ hiển thị **4 giây** rồi tự thu gọn.
5. **Hạng mục 3 (`Tối ưu code`) & Lộ trình (`v0.3.0`):** Thực hiện cuốn chiếu **3 chặng tuần tự** trong bản `v0.3.0`:
   - **Chặng 1:** Tối ưu & tách nhỏ gọn gàng toàn bộ các module (`renderer/`, `window/`, `state/`, `hud/` và rà soát `updater`, `media`, `tray`, `watcher`, `config`, `vsync`, `main`, lọc lại thư viện/feature thừa).
   - **Chặng 2:** Triển khai **Theo dõi Anti** (10 trạng thái, vòng xoay mượt, 1 tệp tiến độ chung qua hook + `ReadDirectoryChangesW`).
   - **Chặng 3:** Triển khai **File Shelf** (khay cạnh phải Dynamic trên FHD, OLE Drag-Drop 2 chiều, tickbox + `Delete All` an toàn).

---

## 3. Yêu cầu chức năng chi tiết

### Chặng 1 — Tối ưu và tách gọn toàn bộ module
- **FR-1.1:** Phân rã các module lớn quá tải thành cấu trúc thư mục rõ vai trò (`src/renderer/`, `src/window/`, `src/state/`, `src/hud/`), đồng thời rà soát tinh giản hàm dài/trùng lặp ở mọi module còn lại (`updater`, `media`, `tray`, `watcher`, `config`, `vsync`, `main`).
- **FR-1.2:** Rà soát toàn bộ feature của `windows` trong `Cargo.toml`, gỡ bỏ mọi feature không thật sự dùng tới, giữ nguyên hành vi và **100% unit/integration tests đạt (`37/37`)**.

### Chặng 2 — Theo dõi Anti (10 trạng thái, dùng chung 1 tệp tiến độ)
- **FR-2.1:** Định nghĩa đầy đủ 10 trạng thái agent Antigravity kèm nhãn tiếng Việt chuẩn:
  1. `Đang đọc dự án`
  2. `Đang phân tích`
  3. `Đang code`
  4. `Đang test`
  5. `Đang build`
  6. `Đang chờ duyệt`
  7. `Đang sửa lỗi`
  8. `Bị lỗi`
  9. `Đã hủy`
  10. `Đã xong`
- **FR-2.2:** Cung cấp chế độ thực thi nhẹ `--anti-hook <EVENT>` ngay trong `win-dynamic-island.exe` để làm hook handler chung trong `~/.gemini/config/hooks.json` cho cả `agy CLI`, `Anti IDE` và `Anti app`: đọc JSON stdin do hook gửi, nhận diện nguồn (`agy CLI`, `Anti IDE`, `Anti app`) và ánh xạ sang 10 trạng thái, ghi atomic vào 1 tệp JSON chung (`%LOCALAPPDATA%\win-dynamic-island\anti-progress.json`) không gọi HTTP API.
- **FR-2.3:** App dùng luồng nền với Win32 `ReadDirectoryChangesW` để nhận thay đổi tức thời (0% CPU khi đứng yên); đồng thời phát hiện app foreground là `agy` / `Antigravity` để chỉ bật thẻ tiến độ Anti khi người dùng chuyển sang app khác.
- **FR-2.4:** Có animation xoay mượt cho các trạng thái đang chạy (dùng chung nhịp đồng bộ khung hình và tắt hẳn khi đứng yên hoặc ở trạng thái kết thúc).
- **FR-2.5:** Trong bảng `Settings` bổ sung nút/công tắc `"Theo dõi Anti"` và nút `"Cài Hook Anti"` / `"Gỡ Hook Anti"` (hòa trộn an toàn vào `~/.gemini/config/hooks.json`, không xóa hook sẵn có khác của người dùng).

### Chặng 3 — File Shelf (Khay tệp cạnh Dynamic trên FHD)
- **FR-3.1:** Gắn khay thả `File Shelf` ngay cạnh phải Dynamic Island trên mép trên màn hình FHD (`1920x1080`) trong cùng hệ thống bề mặt kính đồng pha (`SetWindowRgn` hợp vùng `RGN_OR` giữa viên thuốc Dynamic và khay File Shelf để vùng trống xung quanh/giữa vẫn xuyên chuột hoàn toàn).
- **FR-3.2:** Hỗ trợ **kéo vào** (`IDropTarget` + `CF_HDROP` từ Explorer/Desktop/Chrome) và **kéo ra** (`DoDragDrop` + `IDropSource` + Shell `IDataObject` sang Zalo, Discord, Chrome, Explorer). Tự sáng viền khi kéo tệp lại gần vùng khay trên.
- **FR-3.3:** Danh sách tệp trong khay hỗ trợ tickbox từng dòng và tickbox `"Chọn tất cả"`. Khi chưa tick mục nào, nút `Delete All` bị ẩn; khi có ít nhất 1 mục được tick, nút `Delete All` xuất hiện ở thanh đáy để dọn danh sách khỏi khay tạm (tuyệt đối không xóa tệp gốc trên đĩa).

---

## 4. Tiêu chí nghiệm thu (`v0.3.0`)
- [ ] **Chặng 1:** Toàn bộ mã nguồn được tách module gọn gàng, sạch `clippy --all-targets -- -D warnings`, `37/37` unit/integration tests cũ vẫn đạt, không hồi quy hiệu năng.
- [ ] **Chặng 2:** Thử nghiệm phát đủ 10 trạng thái vào tệp tiến độ chung từ cả 3 nhãn nguồn (`agy CLI`, `Anti IDE`, `Anti app`) -> khi chuyển sang cửa sổ khác, Island hiển thị đúng tên trạng thái kèm vòng xoay mượt; khi quay lại cửa sổ Anti thì nhường lại; trạng thái kết thúc tự gọn sau 4s; `IDLE_CPU_2S_MS = 0` khi rảnh.
- [ ] **Chặng 3:** Kéo tệp từ Explorer thả vào khay cạnh Dynamic trên FHD thành công; chưa tick thì không thấy `Delete All`, tick >= 1 tệp thì hiện `Delete All`; bấm `Delete All` chỉ xóa mục khỏi khay mà tệp gốc trên ổ đĩa vẫn nguyên vẹn; kéo tệp từ khay thả sang Explorer/ứng dụng khác thành công.
- [ ] **Chất lượng tổng thể:** `VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `NV_MODULES=0`, kính trên nền `#FFFFFF` đạt tương phản >= `4.5:1`, kiểm chứng giao diện thực tế bằng ảnh chụp màn hình FHD và tổng kiểm tra `SO_TAY_KIEM_TRA_BUG.md`.
