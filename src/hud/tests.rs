use super::*;

#[test]
fn wheel_accumulator_handles_detents_touchpad_and_reversals() {
    let mut acc = WheelVolumeAccumulator::default();
    assert_eq!(acc.push_delta(120), 3);
    assert_eq!(acc.push_delta(-240), -6);

    // Touchpad cuộn mịn: nhiều gói nhỏ cộng dồn đủ 40 sinh 1 bước, rung tay -2 không xóa tích lũy.
    assert_eq!(acc.push_delta(15), 0);
    assert_eq!(acc.push_delta(15), 0);
    assert_eq!(acc.push_delta(-2), 0);
    assert_eq!(acc.push_delta(15), 1);
    assert_eq!(acc.push_delta(20), 0);

    // Đảo chiều rõ ràng: phần dư cũ (+23) bị xóa sạch.
    assert_eq!(acc.push_delta(-30), 0);
    assert_eq!(acc.push_delta(-15), -1);

    acc.push_delta(30);
    acc.reset();
    assert_eq!(acc.push_delta(15), 0);
}

#[test]
fn volume_step_clamps_bounds_and_handles_bursts() {
    assert!((apply_volume_step(0.50, 3) - 0.53).abs() < 1e-5);
    assert!((apply_volume_step(0.99, 5) - 1.0).abs() < 1e-5);
    assert!((apply_volume_step(0.01, -5) - 0.0).abs() < 1e-5);
    assert!((apply_volume_step(0.50, 999) - 1.0).abs() < 1e-5);
    assert_eq!(clamp_volume_ratio(f32::NAN), 0.0);
}

#[test]
fn power_tracker_alerts_only_on_ac_toggle_or_low_thresholds() {
    let mut tracker = PowerStateTracker::default();
    // Lần đọc đầu tiên không pop HUD.
    assert!(
        tracker
            .evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(80),
            })
            .is_none()
    );
    // Tụt từng % bình thường không spam HUD.
    assert!(
        tracker
            .evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(79),
            })
            .is_none()
    );
    // Cắm sạc -> pop HUD sạc.
    assert_eq!(
        tracker.evaluate_update(PowerSnapshot {
            has_battery: true,
            on_ac: true,
            percent: Some(79),
        }),
        Some((
            PowerSnapshot {
                has_battery: true,
                on_ac: true,
                percent: Some(79),
            },
            false
        ))
    );
    // Đang cắm sạc tăng % không spam.
    assert!(
        tracker
            .evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: true,
                percent: Some(85),
            })
            .is_none()
    );
    // Rút sạc -> pop HUD dùng pin.
    assert_eq!(
        tracker.evaluate_update(PowerSnapshot {
            has_battery: true,
            on_ac: false,
            percent: Some(25),
        }),
        Some((
            PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(25),
            },
            false
        ))
    );
    // Tụt xuống <= 15% lần đầu -> cảnh báo pin yếu.
    assert_eq!(
        tracker.evaluate_update(PowerSnapshot {
            has_battery: true,
            on_ac: false,
            percent: Some(15),
        }),
        Some((
            PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(15),
            },
            true
        ))
    );
    // Tụt tiếp 14% -> không nhắc lại cho tới ngưỡng <= 7%.
    assert!(
        tracker
            .evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(14),
            })
            .is_none()
    );
    assert_eq!(
        tracker.evaluate_update(PowerSnapshot {
            has_battery: true,
            on_ac: false,
            percent: Some(7),
        }),
        Some((
            PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(7),
            },
            true
        ))
    );

    // AC hint tu PBT_POWERSETTINGCHANGE bat HUD tuc thi va khong bi spam trung khi RefreshPower ve cung on_ac.
    assert_eq!(
        tracker.evaluate_ac_hint(true),
        Some((
            PowerSnapshot {
                has_battery: true,
                on_ac: true,
                percent: Some(7),
            },
            false
        ))
    );
    assert!(
        tracker
            .evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: true,
                percent: Some(8),
            })
            .is_none()
    );
}

#[test]
fn audio_devices_sort_and_policy_config_com_init() {
    assert_eq!(
        policy_config::classify_audio_output_kind("BTHENUM", Some(5), "Diamond Stereo"),
        AudioOutputKind::Bluetooth
    );
    assert_eq!(
        policy_config::classify_audio_output_kind("HDAUDIO", Some(3), "Headphones"),
        AudioOutputKind::Headphones
    );
    assert_eq!(
        policy_config::classify_audio_output_kind("HDAUDIO", Some(1), "Speakers (Realtek)"),
        AudioOutputKind::Speaker
    );

    let mut items = vec![
        AudioOutputDevice {
            id: String::from("id-2"),
            name: String::from("Speakers (Realtek)"),
            kind: AudioOutputKind::Speaker,
            is_default: false,
        },
        AudioOutputDevice {
            id: String::from("id-1"),
            name: String::from("headphones (USB)"),
            kind: AudioOutputKind::Headphones,
            is_default: true,
        },
    ];
    policy_config::sort_audio_devices(&mut items);
    assert_eq!(items[0].id, "id-1");
    assert_eq!(items[1].id, "id-2");

    let com = ComMtaGuard(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.expect("enumerator");
    let (real_devices, _def, _kind) = enumerate_output_devices(&enumerator);
    println!("Detected {} active playback devices", real_devices.len());
    for d in &real_devices {
        assert!(!d.id.is_empty());
        assert!(!d.name.is_empty());
    }
    let policy = policy_com::create_policy_config();
    assert!(policy.is_ok(), "IPolicyConfig CoCreateInstance failed: {policy:?}");
    if let Some(def) = real_devices.iter().find(|d| d.is_default) {
        let res = policy_com::set_default_audio_device(&def.id);
        assert!(
            res.is_ok(),
            "IPolicyConfig::SetDefaultEndpoint failed on current default: {res:?}"
        );
    }
    drop(policy);
    drop(enumerator);
    drop(com);
}
