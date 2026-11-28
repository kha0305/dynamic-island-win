# DANH SÁCH ĐẦU VIỆC: `device-hud-and-anti-fix` (`v0.3.5`)

- [x] **T1:** Cập nhật [Cargo.toml](../../Cargo.toml) (version `0.3.5` + 3 feature `Win32_Devices_*`).
- [x] **T2:** Cập nhật [src/anti.rs](../../src/anti.rs) (nhận diện nguồn 3 tầng độc lập đường dẫn, ưu tiên `toolSummary` + rút gọn lệnh, `recent_steps`, unit test) & [src/watcher.rs](../../src/watcher.rs) (`focus_anti_window`).
- [x] **T3:** Tạo [src/hud/device.rs](../../src/hud/device.rs) (theo dõi Bluetooth `% pin` + USB + ngoại vi bằng `CM_Register_Notification`, gộp `ContainerId`, đọc bù `% pin`) và cập nhật [src/hud/policy_config.rs](../../src/hud/policy_config.rs) + [src/hud/mod.rs](../../src/hud/mod.rs) (`AudioOutputKind` + `IMMNotificationClient` tai nghe).
- [x] **T4:** Cập nhật [src/state/mod.rs](../../src/state/mod.rs), [src/state/animator.rs](../../src/state/animator.rs), [src/state/hitboxes.rs](../../src/state/hitboxes.rs), [src/config.rs](../../src/config.rs), [src/tray.rs](../../src/tray.rs) và test tương ứng.
- [x] **T5:** Cập nhật lớp vẽ [src/renderer/](../../src/renderer/mod.rs) (`anti_view.rs`, `hud_views.rs`, `media_views.rs`, `expanded_view.rs`, `settings_views.rs`, `tests.rs`) cho Anti nâng cấp, icon âm thanh thống nhất, HUD thiết bị `% pin` và tile `HUD Thiết bị`.
- [x] **T6:** Cập nhật điều phối cửa sổ [src/window/](../../src/window/mod.rs) (`modes.rs`, `input.rs`, `workers.rs`, `tests.rs`).
- [x] **T7:** Chạy `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, `cargo build --release --locked`, kiểm chứng thực tế bản chạy ở `Medium IL`, cập nhật `dist/`, [SO_TAY_KIEM_TRA_BUG.md](../../SO_TAY_KIEM_TRA_BUG.md) và [NHAT_KY_HOAT_DONG.md](../../NHAT_KY_HOAT_DONG.md).
