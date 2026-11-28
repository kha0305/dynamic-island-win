use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn push_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn align4(buf: &mut Vec<u8>) {
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

fn push_utf16_null(buf: &mut Vec<u8>, s: &str) -> usize {
    let mut count = 0usize;
    for unit in s.encode_utf16() {
        push_u16(buf, unit);
        count += 1;
    }
    push_u16(buf, 0);
    count + 1
}

fn write_res_record(
    out: &mut Vec<u8>,
    res_type: u16,
    res_id: u16,
    flags: u16,
    lang: u16,
    data: &[u8],
) {
    push_u32(out, data.len() as u32);
    push_u32(out, 32);
    push_u16(out, 0xFFFF);
    push_u16(out, res_type);
    push_u16(out, 0xFFFF);
    push_u16(out, res_id);
    push_u32(out, 0);
    push_u16(out, flags);
    push_u16(out, lang);
    push_u32(out, 0);
    push_u32(out, 0);
    out.extend_from_slice(data);
    align4(out);
}

fn build_version_string_block(key: &str, value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    push_u16(&mut buf, 0); // wLength placeholder
    push_u16(&mut buf, 0); // wValueLength placeholder (in WORDs)
    push_u16(&mut buf, 1); // wType = text
    push_utf16_null(&mut buf, key);
    align4(&mut buf);
    let value_words = push_utf16_null(&mut buf, value) as u16;
    let total_len = buf.len() as u16;
    buf[0..2].copy_from_slice(&total_len.to_le_bytes());
    buf[2..4].copy_from_slice(&value_words.to_le_bytes());
    buf
}

fn build_version_resource(
    ver: (u16, u16, u16, u16),
    file_desc: &str,
    original_filename: &str,
    internal_name: &str,
) -> Vec<u8> {
    let ver_str = format!("{}.{}.{}.{}", ver.0, ver.1, ver.2, ver.3);
    let pairs = [
        ("CompanyName", "kha0305"),
        ("FileDescription", file_desc),
        ("FileVersion", ver_str.as_str()),
        ("InternalName", internal_name),
        ("OriginalFilename", original_filename),
        ("ProductName", "Win Dynamic Island"),
        ("ProductVersion", ver_str.as_str()),
    ];

    let mut string_table = Vec::new();
    push_u16(&mut string_table, 0);
    push_u16(&mut string_table, 0);
    push_u16(&mut string_table, 1);
    push_utf16_null(&mut string_table, "040904b0");
    align4(&mut string_table);
    for (i, (k, v)) in pairs.iter().enumerate() {
        let child = build_version_string_block(k, v);
        string_table.extend_from_slice(&child);
        if i + 1 < pairs.len() {
            align4(&mut string_table);
        }
    }
    let st_len = string_table.len() as u16;
    string_table[0..2].copy_from_slice(&st_len.to_le_bytes());

    let mut sfi = Vec::new();
    push_u16(&mut sfi, 0);
    push_u16(&mut sfi, 0);
    push_u16(&mut sfi, 1);
    push_utf16_null(&mut sfi, "StringFileInfo");
    align4(&mut sfi);
    sfi.extend_from_slice(&string_table);
    let sfi_len = sfi.len() as u16;
    sfi[0..2].copy_from_slice(&sfi_len.to_le_bytes());

    let mut var_block = Vec::new();
    push_u16(&mut var_block, 0);
    push_u16(&mut var_block, 4);
    push_u16(&mut var_block, 0);
    push_utf16_null(&mut var_block, "Translation");
    align4(&mut var_block);
    push_u16(&mut var_block, 0x0409);
    push_u16(&mut var_block, 0x04B0);
    let var_len = var_block.len() as u16;
    var_block[0..2].copy_from_slice(&var_len.to_le_bytes());

    let mut vfi = Vec::new();
    push_u16(&mut vfi, 0);
    push_u16(&mut vfi, 0);
    push_u16(&mut vfi, 1);
    push_utf16_null(&mut vfi, "VarFileInfo");
    align4(&mut vfi);
    vfi.extend_from_slice(&var_block);
    let vfi_len = vfi.len() as u16;
    vfi[0..2].copy_from_slice(&vfi_len.to_le_bytes());

    let mut root = Vec::new();
    push_u16(&mut root, 0);
    push_u16(&mut root, 52);
    push_u16(&mut root, 0);
    push_utf16_null(&mut root, "VS_VERSION_INFO");
    align4(&mut root);

    // VS_FIXEDFILEINFO (52 bytes)
    push_u32(&mut root, 0xFEEF_04BD); // dwSignature
    push_u32(&mut root, 0x0001_0000); // dwStrucVersion
    push_u32(&mut root, ((ver.0 as u32) << 16) | (ver.1 as u32));
    push_u32(&mut root, ((ver.2 as u32) << 16) | (ver.3 as u32));
    push_u32(&mut root, ((ver.0 as u32) << 16) | (ver.1 as u32));
    push_u32(&mut root, ((ver.2 as u32) << 16) | (ver.3 as u32));
    push_u32(&mut root, 0x3F); // dwFileFlagsMask
    push_u32(&mut root, 0); // dwFileFlags
    push_u32(&mut root, 0x0004_0004); // VOS_NT_WINDOWS32
    push_u32(&mut root, 1); // VFT_APP
    push_u32(&mut root, 0); // VFT2_UNKNOWN
    push_u32(&mut root, 0);
    push_u32(&mut root, 0);

    align4(&mut root);
    root.extend_from_slice(&sfi);
    align4(&mut root);
    root.extend_from_slice(&vfi);
    let root_len = root.len() as u16;
    root[0..2].copy_from_slice(&root_len.to_le_bytes());
    root
}

fn build_manifest_xml(app_name: &str) -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="{app_name}" type="win32"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
    </application>
  </compatibility>
</assembly>
"#
    )
    .into_bytes()
}

fn generate_res_file(
    ico_bytes: &[u8],
    out_path: &Path,
    ver: (u16, u16, u16, u16),
    app_name: &str,
    file_desc: &str,
    original_filename: &str,
    internal_name: &str,
) {
    assert!(ico_bytes.len() >= 6, "app.ico qua ngan");
    let res_type = u16::from_le_bytes([ico_bytes[2], ico_bytes[3]]);
    let count = u16::from_le_bytes([ico_bytes[4], ico_bytes[5]]);
    assert_eq!(res_type, 1, "Tep khong phai dinh dang .ico hop le");
    assert!(
        ico_bytes.len() >= 6 + (count as usize) * 16,
        "app.ico thieu bang thu muc"
    );

    let mut res = Vec::new();
    // Dummy header record (32 bytes)
    write_res_record(&mut res, 0, 0, 0, 0, &[]);

    let mut group_icon = Vec::new();
    push_u16(&mut group_icon, 0);
    push_u16(&mut group_icon, 1);
    push_u16(&mut group_icon, count);

    for i in 0..(count as usize) {
        let entry_off = 6 + i * 16;
        let entry = &ico_bytes[entry_off..entry_off + 16];
        let bytes_in_res = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let image_off = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        let img = &ico_bytes[image_off..image_off + bytes_in_res];
        let icon_id = (i + 1) as u16;
        write_res_record(&mut res, 3, icon_id, 0x1010, 0x0409, img);
        group_icon.extend_from_slice(&entry[0..12]);
        push_u16(&mut group_icon, icon_id);
    }

    // RT_GROUP_ICON = 14, ID = 1
    write_res_record(&mut res, 14, 1, 0x1030, 0x0409, &group_icon);

    // RT_VERSION = 16, ID = 1
    let version_bytes = build_version_resource(ver, file_desc, original_filename, internal_name);
    write_res_record(&mut res, 16, 1, 0x0030, 0x0409, &version_bytes);

    // RT_MANIFEST = 24, ID = 1
    let manifest_bytes = build_manifest_xml(app_name);
    write_res_record(&mut res, 24, 1, 0x0030, 0x0409, &manifest_bytes);

    fs::write(out_path, res).expect("Khong the ghi tep .res tai OUT_DIR");
}

fn parse_pkg_version() -> (u16, u16, u16, u16) {
    let major = env::var("CARGO_PKG_VERSION_MAJOR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let minor = env::var("CARGO_PKG_VERSION_MINOR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let patch = env::var("CARGO_PKG_VERSION_PATCH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (major, minor, patch, 0)
}

fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=WDI_SETUP_PAYLOAD");
    println!("cargo:rustc-check-cfg=cfg(has_setup_payload)");

    if let Some(payload) = env::var_os("WDI_SETUP_PAYLOAD") {
        let p = PathBuf::from(&payload);
        if p.is_file() {
            let abs = fs::canonicalize(&p).unwrap_or(p);
            println!("cargo:rerun-if-changed={}", abs.display());
            println!("cargo:rustc-cfg=has_setup_payload");
            println!("cargo:rustc-env=WDI_SETUP_PAYLOAD_ABS={}", abs.display());
        }
    }

    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let ico_path = manifest_dir.join("assets").join("app.ico");
        let ico_bytes = fs::read(&ico_path).expect("Thieu tep assets/app.ico");
        let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        let ver = parse_pkg_version();

        let app_res = out_dir.join("win-dynamic-island.res");
        generate_res_file(
            &ico_bytes,
            &app_res,
            ver,
            "kha0305.WinDynamicIsland",
            "Win Dynamic Island - Thanh viên thuốc thông minh cho Windows 11",
            "win-dynamic-island.exe",
            "win-dynamic-island",
        );
        println!(
            "cargo:rustc-link-arg-bin=win-dynamic-island={}",
            app_res.display()
        );

        let setup_res = out_dir.join("win-dynamic-island-setup.res");
        generate_res_file(
            &ico_bytes,
            &setup_res,
            ver,
            "kha0305.WinDynamicIslandSetup",
            "Trình cài đặt Win Dynamic Island cho Windows 11",
            "WinDynamicIsland-Setup.exe",
            "win-dynamic-island-setup",
        );
        println!(
            "cargo:rustc-link-arg-bin=win-dynamic-island-setup={}",
            setup_res.display()
        );
    }
}
