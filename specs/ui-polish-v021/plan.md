# KẾ HOẠCH KIẾN TRÚC: BẢN VÁ UI v0.2.1 (`UI-POLISH-V021`)

- **Đặc tả nguồn:** [specs/ui-polish-v021/spec.md](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/specs/ui-polish-v021/spec.md)
- **Trạng thái:** Đã hoàn tất triển khai & kiểm chứng Phase 7 (`cargo clippy`, `37/37` tests `0.27s`, `--release` `916,992 bytes`).

---

## 1. Thiết kế chi tiết các màn hình cập nhật

### 1.1. Màn hình `Expanded` (`424 x 160 DIP`, `r = 28.0`, `top = 6.0`)
Đồng bộ bề rộng `424.0 DIP` với `Settings` và `AudioDevices` để chuyển qua lại không đổi bề rộng ngang:

```
+------------------------------------------------------------------------------+
|  [64x64]  [Vietsub] Tam Bai Hong Tran Luong, To Mac...                       |
|  [ Art ]  sd Jun                                                             |
|           Google Chrome                                                      |
|  00:02   (o)--------------------------------------------------------  12:06  |
|  [ (Loa) Speakers (Realtek... ]    ( |< )  ( > )  ( >| )    [ (*) Cai dat ]  |
+------------------------------------------------------------------------------+
   ^ 128 DIP (text ~96 DIP)           ^ 3 nut chinh giua       ^ 128 DIP doi xung
```

- Tiêu đề, nghệ sĩ và nguồn phát mở rộng tới `p.left + p.width - 18.0` (`310 DIP`, tăng `+70 DIP` so với `240 DIP` cũ).
- Hàng đáy (`controls_bottom = p.top + p.height - 10.0`):
  - `OpenAudioDevices`: `p.left + 16.0 .. center_x - 68.0` (`128 x 32 DIP`), phần chữ bắt đầu từ `left + 26.0` đến `right - 6.0` (`96 DIP`).
  - Cụm 3 nút phát nhạc giữ chính giữa: `Prev` (`center_x - 60 .. center_x - 26`, `34x34`), `Play/Pause` (`center_x - 19 .. center_x + 19`, `38x38`), `Next` (`center_x + 26 .. center_x + 60`, `34x34`).
  - `OpenSettings`: `center_x + 68.0 .. p.left + p.width - 16.0` (`128 x 32 DIP`), vẽ icon bánh răng vector Direct2D + chữ `"Cài đặt"` căn giữa hài hòa.

### 1.2. Màn hình `Settings` (`424 x 360 DIP`, `r = 26.0`, `top = 6.0`)
Nâng `MAX_SURFACE_HEIGHT_DIP` từ `336.0` lên `396.0` (đủ biên nảy lò xo ở 144Hz):
- **Header (`top + 10 .. 36`):** Tiêu đề `"Bảng điều khiển nhanh"` + nhãn phụ phiên bản `"v0.2.1"` dịu nhẹ bên cạnh, bên phải là nút `"Quay lại"` (`74 x 24 DIP`) dùng màu chữ trung tính `text_primary`.
- **Nhóm 1 — Hiển thị (`top + 40 .. 54`):**
  - Hàng 1 (`56..88`): `Tự ẩn khi rảnh` | `Ẩn toàn màn hình`
  - Hàng 2 (`94..126`): `Gọn mép 4s` | `Sóng nhạc động`
- **Nhóm 2 — HUD & Âm thanh (`top + 134 .. 148`):**
  - Hàng 3 (`150..182`): `HUD Âm lượng` | `Cuộn chỉnh loa`
  - Hàng 4 (`188..220`): `HUD Pin / Sạc` | `Thiết bị phát` (mở nhanh `AudioDevices`)
- **Nhóm 3 — Hệ thống (`top + 228 .. 242`):**
  - Hàng 5 (`244..276`): `Mở cùng Windows` | `Kiểm tra cập nhật` (hoặc `"Cập nhật lên <ver>"` màu `sys_accent` khi có bản mới)
  - Hàng 6 (`282..314`): `Màn hình: <tên>` + nhãn `"Đổi"` (màu `sys_accent`)
- **Footer (`322..352`, đáy `360`):** `Ẩn tạm` | `Thoát ứng dụng` (nút đỏ `danger_bg` / `danger_border`).

### 1.3. Màn hình `AudioDevices` (`424 x 107..293 DIP`, `r = 26.0`, `top = 6.0`)
- Tính chiều cao linh hoạt qua `audio_devices_height_for_count(count)`:
  - `visible = count.clamp(1, MAX_AUDIO_DEVICES_VISIBLE)` (`1..=5`).
  - `list_bottom = 56.0 + visible as f32 * 37.0 + (visible.saturating_sub(1)) as f32 * 6.0`.
  - Cộng thêm `14.0 DIP` lề đáy (hoặc `28.0 DIP` khi `count > 5` để đặt dòng gợi ý `"Cuộn chuột để xem thêm (x-y/N)"`).
  - Với 3 thiết bị thực tế trên máy: chiều cao thu gọn từ `284.0` xuống `193.0 DIP`, loại bỏ hoàn toàn khoảng trống đen ở đáy.
- Dùng `sys_accent` / `sys_accent_soft` cho viền thiết bị đang chọn và chữ `"Đang dùng"`, và màu `text_primary` cho nút `"Quay lại"`.

### 1.4. Chế độ `Slim` (`152 x 16 DIP`) & `Volume HUD`
- **`draw_slim`:** Neo qua `virtual_pill_anchored(input.pill, 152.0, 16.0, false)`. Khi `!input.media.has_session`, bỏ vòng tròn placeholder + rãnh dài `switch_off`, thay bằng dải Notch chờ tối giản gồm chấm chỉ báo nhỏ (`4.5 x 4.5 DIP`, màu `sys_accent_soft`) + vạch cân tâm mờ (`26 x 2.6 DIP`, màu `card_border`). Khi có media, giữ chấm ảnh bìa `10x10`, thanh tiến độ mảnh `2.4 DIP` và 3 vạch sóng nhạc (hoặc 2 vạch tạm dừng `||` thu nhỏ khi paused).
- **`draw_compact_hud` (`show_device_name == true`):** Khi `slider_hovered`, tăng độ dày thanh âm lượng từ `2.8` lên `4.2 DIP` và vẽ knob (`4.6 DIP`) + halo (`7.2 DIP`) tại vị trí mức âm lượng hiện tại.

### 1.5. Tách màu hệ thống & Luồng quay lại thông minh
- Bổ sung `sys_accent` (`0.38, 0.85, 1.0, 1.0`) và `sys_accent_soft` (`0.14, 0.42, 0.62, 0.54`) cố định trong `BrushPalette` (`src/renderer.rs`).
- Lưu `settings_return_mode` và `audio_devices_return_mode` trong `WindowController` (`src/window.rs`) để nút `"Quay lại"` đưa về đúng màn hình hợp lệ trước đó (`Settings` <-> `AudioDevices` <-> `Expanded` / `collapsed_rest_mode()`).

---

## 2. Danh sách tệp sẽ tạo / sửa đổi
1. `Cargo.toml` & `Cargo.lock` — Nâng phiên bản `0.2.0` -> `0.2.1`.
2. [src/state.rs](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/src/state.rs) — Cập nhật hằng số bề mặt `396.0`, kích thước `Expanded`/`Settings`, hàm `audio_devices_height_for_count` + `target_dimensions_for_state`, thêm `UiAction::CheckUpdate`, bố cục lại `fill_expanded_hitboxes` và `fill_settings_hitboxes`, cập nhật unit tests.
3. [src/renderer.rs](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/src/renderer.rs) — Thêm `sys_accent`/`sys_accent_soft` và vector bánh răng cho nút `Cài đặt`, cập nhật `draw_slim`, `draw_compact_hud`, `draw_expanded`, `draw_settings`, `draw_audio_devices`, `mode_content_reveal_alpha`, `draw_clipped_contents`, `warm_up_text_formats` và tests GPU.
4. [src/window.rs](file:///C:/Users/ASUS/Desktop/New%20folder/win-dynamic-island/src/window.rs) — Đồng bộ kích thước động cho `AudioDevices`, thêm `settings_return_mode` / `audio_devices_return_mode`, nối `UiAction::CheckUpdate`, truyền `available_update` sang `RenderFrameInput`, cập nhật tests tích hợp.
5. `specs/ui-polish-v021/*`, `NHAT_KY_HOAT_DONG.md`, `SO_TAY_KIEM_TRA_BUG.md`, `TASK_MEMORY.md`, `README.md` — Cập nhật nhật ký, đối chiếu bug và đóng gói `dist/`.

---

## 3. Kế hoạch kiểm chứng (Phase 7)
1. `cargo clippy --all-targets -- -D warnings` (0 cảnh báo).
2. `cargo test --locked -- --nocapture` (toàn bộ unit + integration tests đạt).
3. `cargo build --release --locked`, đo kiểm runtime (`VCRUNTIME_DEP=False`, `EMOJI_HITS=0`, `NV_MODULES=0`, `CPU_2S_MS=0`, `CLEAN_EXIT=True`) và chụp ảnh màn hình thực tế từng chế độ (`Slim`, `Compact`, `Expanded`, `AudioDevices`, `Settings`) để xác nhận trực quan.
4. Đồng bộ `dist\win-dynamic-island.exe`, `.sha256`, `.zip`, dọn `target/` (sau khi hoàn tất), commit và đẩy tag `v0.2.1` lên GitHub.
