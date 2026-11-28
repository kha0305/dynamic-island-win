use std::cell::Cell;
use std::ffi::OsString;
use std::fs;
use std::mem::{ManuallyDrop, size_of};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr::{copy_nonoverlapping, null_mut};
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::{
    DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS,
    DV_E_FORMATETC, E_INVALIDARG, E_NOTIMPL, HGLOBAL, HWND, LPARAM, OLE_E_ADVISENOTSUPPORTED,
    POINT, POINTL, S_FALSE, S_OK, TRUE, WPARAM,
};
use windows::Win32::System::Com::{
    DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject, IDataObject_Impl,
    IEnumFORMATETC, IEnumFORMATETC_Impl, IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalUnlock,
};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, DROPEFFECT_NONE,
    DoDragDrop, IDropSource, IDropSource_Impl, IDropTarget, IDropTarget_Impl, ReleaseStgMedium,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{DROPFILES, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows::core::{BOOL, HRESULT, Ref, Result as WinResult, implement};

use crate::config::config_file_path;

pub const MAX_SHELF_ITEMS: usize = 32;
const SHELF_STORAGE_FILE: &str = "shelf-items.txt";
pub const WM_APP_SHELF_DROP: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 9;
pub const WM_APP_SHELF_HOVER: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfItem {
    pub path: PathBuf,
    pub name: String,
    pub size_label: String,
    pub ext_badge: String,
    pub selected: bool,
}

#[derive(Debug, Default, Clone)]
pub struct ShelfState {
    pub items: Vec<ShelfItem>,
    pub open: bool,
    pub drag_hover: bool,
    pub scroll: usize,
}

impl ShelfState {
    pub fn add_paths(&mut self, paths: &[PathBuf]) -> usize {
        let mut added = 0_usize;
        for raw in paths {
            let Some(item) = build_shelf_item(raw) else {
                continue;
            };
            let duplicate = self
                .items
                .iter()
                .any(|existing| paths_equal_ci(&existing.path, &item.path));
            if duplicate {
                continue;
            }
            if self.items.len() >= MAX_SHELF_ITEMS {
                self.items.pop();
            }
            self.items.insert(0, item);
            added += 1;
        }
        if added > 0 {
            self.scroll = 0;
            self.open = true;
        }
        added
    }

    pub fn toggle_select(&mut self, idx: usize) {
        if let Some(item) = self.items.get_mut(idx) {
            item.selected = !item.selected;
        }
    }

    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|item| item.selected).count()
    }

    pub fn has_any_selected(&self) -> bool {
        self.selected_count() > 0
    }

    pub fn all_selected(&self) -> bool {
        !self.items.is_empty() && self.items.iter().all(|item| item.selected)
    }

    pub fn toggle_select_all(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let target = !self.all_selected();
        for item in &mut self.items {
            item.selected = target;
        }
    }

    /// Chỉ xóa các mục đã được đánh dấu khỏi danh sách tạm trên khay; không bao giờ xóa tệp gốc trên đĩa.
    pub fn delete_selected_from_shelf(&mut self) -> usize {
        if !self.has_any_selected() {
            return 0;
        }
        let before = self.items.len();
        self.items.retain(|item| !item.selected);
        let removed = before.saturating_sub(self.items.len());
        if removed > 0 {
            self.clamp_scroll(MAX_VISIBLE_ROWS_DEFAULT);
        }
        removed
    }

    pub fn drag_paths_for(&self, idx: usize) -> Vec<PathBuf> {
        let Some(target) = self.items.get(idx) else {
            return Vec::new();
        };
        if target.selected && self.selected_count() > 1 {
            self.items
                .iter()
                .filter(|item| item.selected && item.path.exists())
                .map(|item| item.path.clone())
                .collect()
        } else if target.path.exists() {
            vec![target.path.clone()]
        } else {
            Vec::new()
        }
    }

    pub fn prune_missing_files(&mut self) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.path.exists());
        let changed = self.items.len() != before;
        if changed {
            self.clamp_scroll(MAX_VISIBLE_ROWS_DEFAULT);
        }
        changed
    }

    pub fn clamp_scroll(&mut self, max_visible: usize) {
        let max_scroll = self.items.len().saturating_sub(max_visible.max(1));
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }
}

const MAX_VISIBLE_ROWS_DEFAULT: usize = 5;

fn paths_equal_ci(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

fn build_shelf_item(raw: &Path) -> Option<ShelfItem> {
    if !raw.exists() {
        return None;
    }
    let metadata = fs::metadata(raw).ok();
    let is_dir = metadata.as_ref().is_some_and(fs::Metadata::is_dir);
    let name = raw
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map_or_else(
            || raw.to_string_lossy().into_owned(),
            std::borrow::ToOwned::to_owned,
        );
    let size_label = if is_dir {
        String::from("Thư mục")
    } else if let Some(meta) = metadata {
        format_bytes_compact(meta.len())
    } else {
        String::from("--")
    };
    let ext_badge = if is_dir {
        String::from("DIR")
    } else {
        raw.extension()
            .and_then(|e| e.to_str())
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .map_or_else(
                || String::from("FILE"),
                |ext| ext.chars().take(4).collect::<String>().to_ascii_uppercase(),
            )
    };
    Some(ShelfItem {
        path: raw.to_path_buf(),
        name,
        size_label,
        ext_badge,
        selected: false,
    })
}

pub fn format_bytes_compact(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.0} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.2} GB", b / GB)
    }
}

fn shelf_storage_path() -> Option<PathBuf> {
    config_file_path()
        .and_then(|p| p.parent().map(|dir| dir.join(SHELF_STORAGE_FILE)))
}

pub fn load_shelf_state() -> ShelfState {
    let Some(path) = shelf_storage_path() else {
        return ShelfState::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return ShelfState::default();
    };
    let mut paths = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            paths.push(PathBuf::from(trimmed));
        }
    }
    let mut state = ShelfState::default();
    for p in paths.into_iter().rev() {
        let _ = state.add_paths(&[p]);
    }
    state.open = false;
    state
}

pub fn save_shelf_state(state: &ShelfState) {
    let Some(path) = shelf_storage_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut out = String::new();
    for item in &state.items {
        out.push_str(&item.path.to_string_lossy());
        out.push('\n');
    }
    let tmp = path.with_extension("txt.tmp");
    if fs::write(&tmp, out.as_bytes()).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

fn hdrop_accept_format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

pub unsafe fn extract_hdrop_paths(hdrop: HDROP) -> Vec<PathBuf> {
    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
    let mut paths = Vec::new();
    for idx in 0..count {
        let len = unsafe { DragQueryFileW(hdrop, idx, None) };
        if len == 0 {
            continue;
        }
        let mut buf = vec![0_u16; len as usize + 1];
        let copied = unsafe { DragQueryFileW(hdrop, idx, Some(&mut buf)) };
        if copied > 0 {
            let os = OsString::from_wide(&buf[..copied as usize]);
            paths.push(PathBuf::from(os));
        }
    }
    paths
}

fn build_dropfiles_hglobal(paths: &[PathBuf]) -> WinResult<HGLOBAL> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide_chars: Vec<u16> = Vec::new();
    for path in paths {
        wide_chars.extend(path.as_os_str().encode_wide());
        wide_chars.push(0);
    }
    wide_chars.push(0);

    let header_bytes = size_of::<DROPFILES>();
    let payload_bytes = wide_chars.len() * size_of::<u16>();
    let total_bytes = header_bytes + payload_bytes;

    let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total_bytes) }?;
    let locked = unsafe { GlobalLock(hmem) };
    if locked.is_null() {
        return Err(windows::core::Error::from_thread());
    }
    unsafe {
        let header = locked.cast::<DROPFILES>();
        *header = DROPFILES {
            pFiles: header_bytes as u32,
            pt: POINT::default(),
            fNC: BOOL(0),
            fWide: TRUE,
        };
        let dst = locked.cast::<u8>().add(header_bytes).cast::<u16>();
        copy_nonoverlapping(wide_chars.as_ptr(), dst, wide_chars.len());
        let _ = GlobalUnlock(hmem);
    }
    Ok(hmem)
}

#[implement(IDropTarget)]
pub struct IslandDropTarget {
    hwnd_raw: isize,
    drop_tx: Sender<Vec<PathBuf>>,
    accepted_effect: Cell<DROPEFFECT>,
}

impl IslandDropTarget {
    pub fn new(hwnd: HWND, drop_tx: Sender<Vec<PathBuf>>) -> Self {
        Self {
            hwnd_raw: hwnd.0 as isize,
            drop_tx,
            accepted_effect: Cell::new(DROPEFFECT_NONE),
        }
    }

    fn hwnd(&self) -> HWND {
        HWND(self.hwnd_raw as *mut core::ffi::c_void)
    }

    fn post_hover(&self, active: bool) {
        unsafe {
            let _ = PostMessageW(
                Some(self.hwnd()),
                WM_APP_SHELF_HOVER,
                WPARAM(usize::from(active)),
                LPARAM(0),
            );
        }
    }

    fn pick_drop_effect(allowed_ptr: *mut DROPEFFECT) -> DROPEFFECT {
        let allowed = if allowed_ptr.is_null() {
            DROPEFFECT_COPY
        } else {
            unsafe { *allowed_ptr }
        };
        if (allowed.0 & DROPEFFECT_COPY.0) != 0 || allowed == DROPEFFECT_NONE {
            DROPEFFECT_COPY
        } else if (allowed.0 & DROPEFFECT_LINK.0) != 0 {
            DROPEFFECT_LINK
        } else if (allowed.0 & DROPEFFECT_MOVE.0) != 0 {
            DROPEFFECT_MOVE
        } else {
            DROPEFFECT_COPY
        }
    }
}

impl IDropTarget_Impl for IslandDropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        let fmt = hdrop_accept_format();
        let accepted = pdataobj
            .as_ref()
            .is_some_and(|obj| unsafe { obj.QueryGetData(&fmt) }.is_ok());
        let effect = if accepted {
            IslandDropTarget::pick_drop_effect(pdweffect)
        } else {
            DROPEFFECT_NONE
        };
        self.accepted_effect.set(effect);
        if !pdweffect.is_null() {
            unsafe {
                *pdweffect = effect;
            }
        }
        self.post_hover(accepted);
        Ok(())
    }

    fn DragOver(
        &self,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        let cached = self.accepted_effect.get();
        let effect = if cached != DROPEFFECT_NONE {
            IslandDropTarget::pick_drop_effect(pdweffect)
        } else {
            DROPEFFECT_NONE
        };
        if !pdweffect.is_null() {
            unsafe {
                *pdweffect = effect;
            }
        }
        Ok(())
    }

    fn DragLeave(&self) -> WinResult<()> {
        self.accepted_effect.set(DROPEFFECT_NONE);
        self.post_hover(false);
        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        let drop_effect = IslandDropTarget::pick_drop_effect(pdweffect);
        self.accepted_effect.set(DROPEFFECT_NONE);
        self.post_hover(false);

        let Some(obj) = pdataobj.as_ref() else {
            if !pdweffect.is_null() {
                unsafe { *pdweffect = DROPEFFECT_NONE };
            }
            return Ok(());
        };

        let fmt = hdrop_accept_format();
        let mut medium = match unsafe { obj.GetData(&fmt) } {
            Ok(med) => med,
            Err(err) => {
                if !pdweffect.is_null() {
                    unsafe { *pdweffect = DROPEFFECT_NONE };
                }
                return Err(err);
            }
        };

        let paths = if (medium.tymed & (TYMED_HGLOBAL.0 as u32)) != 0 {
            let hdrop = unsafe { HDROP(medium.u.hGlobal.0) };
            unsafe { extract_hdrop_paths(hdrop) }
        } else {
            Vec::new()
        };
        unsafe { ReleaseStgMedium(&mut medium) };

        if !paths.is_empty() && self.drop_tx.send(paths).is_ok() {
            unsafe {
                let _ = PostMessageW(Some(self.hwnd()), WM_APP_SHELF_DROP, WPARAM(0), LPARAM(0));
            }
            if !pdweffect.is_null() {
                unsafe { *pdweffect = drop_effect };
            }
        } else if !pdweffect.is_null() {
            unsafe { *pdweffect = DROPEFFECT_NONE };
        }
        Ok(())
    }
}

#[implement(IEnumFORMATETC)]
struct ShelfFormatEnum {
    formats: Vec<FORMATETC>,
    index: Cell<usize>,
}

impl IEnumFORMATETC_Impl for ShelfFormatEnum_Impl {
    fn Next(
        &self,
        celt: u32,
        rgelt: *mut FORMATETC,
        pceltfetched: *mut u32,
    ) -> HRESULT {
        if rgelt.is_null() || (celt > 1 && pceltfetched.is_null()) {
            return E_INVALIDARG;
        }
        let mut idx = self.index.get();
        let mut fetched = 0_u32;
        while fetched < celt && idx < self.formats.len() {
            unsafe {
                *rgelt.add(fetched as usize) = self.formats[idx];
            }
            idx += 1;
            fetched += 1;
        }
        self.index.set(idx);
        if !pceltfetched.is_null() {
            unsafe { *pceltfetched = fetched };
        }
        if fetched == celt { S_OK } else { S_FALSE }
    }

    fn Skip(&self, celt: u32) -> WinResult<()> {
        let next = self.index.get().saturating_add(celt as usize);
        self.index.set(next.min(self.formats.len()));
        if next <= self.formats.len() {
            Ok(())
        } else {
            Err(windows::core::Error::from(S_FALSE))
        }
    }

    fn Reset(&self) -> WinResult<()> {
        self.index.set(0);
        Ok(())
    }

    fn Clone(&self) -> WinResult<IEnumFORMATETC> {
        Ok(ShelfFormatEnum {
            formats: self.formats.clone(),
            index: Cell::new(self.index.get()),
        }
        .into())
    }
}

#[implement(IDataObject)]
struct ShelfDataObject {
    paths: Vec<PathBuf>,
}

impl ShelfDataObject {
    fn matches_hdrop(fmt: *const FORMATETC) -> bool {
        if fmt.is_null() {
            return false;
        }
        let f = unsafe { &*fmt };
        f.cfFormat == CF_HDROP.0
            && (f.tymed & (TYMED_HGLOBAL.0 as u32)) != 0
            && (f.dwAspect == DVASPECT_CONTENT.0 || f.dwAspect == 0)
            && (f.lindex == -1 || f.lindex == 0)
    }
}

impl IDataObject_Impl for ShelfDataObject_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> WinResult<STGMEDIUM> {
        if !ShelfDataObject::matches_hdrop(pformatetcin) {
            return Err(windows::core::Error::from(DV_E_FORMATETC));
        }
        let hglobal = build_dropfiles_hglobal(&self.paths)?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: hglobal },
            pUnkForRelease: ManuallyDrop::new(None),
        })
    }

    fn GetDataHere(&self, _pformatetc: *const FORMATETC, _pmedium: *mut STGMEDIUM) -> WinResult<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        if ShelfDataObject::matches_hdrop(pformatetc) {
            S_OK
        } else {
            DV_E_FORMATETC
        }
    }

    fn GetCanonicalFormatEtc(
        &self,
        pformatectin: *const FORMATETC,
        pformatetcout: *mut FORMATETC,
    ) -> HRESULT {
        if !pformatetcout.is_null() && !pformatectin.is_null() {
            unsafe {
                *pformatetcout = *pformatectin;
                (*pformatetcout).ptd = null_mut();
            }
        }
        DATA_S_SAMEFORMATETC
    }

    fn SetData(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *const STGMEDIUM,
        _frelease: BOOL,
    ) -> WinResult<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> WinResult<IEnumFORMATETC> {
        if dwdirection != DATADIR_GET.0 as u32 {
            return Err(windows::core::Error::from(E_NOTIMPL));
        }
        Ok(ShelfFormatEnum {
            formats: vec![hdrop_accept_format()],
            index: Cell::new(0),
        }
        .into())
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: Ref<IAdviseSink>,
    ) -> WinResult<u32> {
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }

    fn DUnadvise(&self, _dwconnection: u32) -> WinResult<()> {
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }

    fn EnumDAdvise(&self) -> WinResult<IEnumSTATDATA> {
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }
}

#[implement(IDropSource)]
struct ShelfDropSource;

impl IDropSource_Impl for ShelfDropSource_Impl {
    fn QueryContinueDrag(
        &self,
        fescapepressed: BOOL,
        grfkeystate: MODIFIERKEYS_FLAGS,
    ) -> HRESULT {
        if fescapepressed.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        if (grfkeystate.0 & MK_LBUTTON.0) == 0 {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

pub fn start_shelf_drag_out(paths: &[PathBuf]) -> bool {
    let valid: Vec<PathBuf> = paths.iter().filter(|p| p.exists()).cloned().collect();
    if valid.is_empty() {
        return false;
    }
    let data_obj: IDataObject = ShelfDataObject { paths: valid }.into();
    let drop_src: IDropSource = ShelfDropSource.into();
    let mut effect = DROPEFFECT_NONE;
    let hr = unsafe { DoDragDrop(&data_obj, &drop_src, DROPEFFECT_COPY, &mut effect) };
    hr == DRAGDROP_S_DROP
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_file(name: &str, content: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wdi_shelf_test_{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = fs::write(&path, content);
        path
    }

    #[test]
    fn shelf_tickbox_controls_delete_visibility_and_never_deletes_disk_files() {
        let p1 = create_temp_file("alpha.txt", b"hello");
        let p2 = create_temp_file("beta.log", b"world data");

        let mut shelf = ShelfState::default();
        assert_eq!(shelf.add_paths(&[p1.clone(), p2.clone()]), 2);
        // Thêm trùng không làm tăng danh sách.
        assert_eq!(shelf.add_paths(std::slice::from_ref(&p1)), 0);
        assert_eq!(shelf.items.len(), 2);

        // Chưa tick ô nào -> Delete All phải ẩn.
        assert!(!shelf.has_any_selected());
        assert_eq!(shelf.selected_count(), 0);

        // Tick 1 mục -> Delete All hiện ra; bấm xóa chỉ bỏ mục khỏi khay, tệp gốc trên đĩa vẫn còn nguyên.
        shelf.toggle_select(0);
        assert!(shelf.has_any_selected());
        assert_eq!(shelf.selected_count(), 1);
        assert_eq!(shelf.delete_selected_from_shelf(), 1);
        assert_eq!(shelf.items.len(), 1);
        assert!(!shelf.has_any_selected());
        assert!(p1.exists(), "Original file must stay untouched on disk");
        assert!(p2.exists(), "Second file must stay untouched on disk");

        // Tick Chọn tất cả -> xóa sạch danh sách tạm, tệp gốc vẫn tồn tại.
        shelf.toggle_select_all();
        assert!(shelf.all_selected());
        assert_eq!(shelf.delete_selected_from_shelf(), 1);
        assert!(shelf.items.is_empty());
        assert!(p1.exists());
        assert!(p2.exists());

        let _ = fs::remove_file(&p1);
        let _ = fs::remove_file(&p2);
        if let Some(dir) = p1.parent() {
            let _ = fs::remove_dir(dir);
        }
    }

    #[test]
    fn shelf_cf_hdrop_roundtrip_preserves_multi_folder_paths() {
        let p1 = create_temp_file("first_unicode_tệp.txt", b"1");
        let p2 = create_temp_file("second.bin", b"123456");

        let data_obj: IDataObject = ShelfDataObject {
            paths: vec![p1.clone(), p2.clone()],
        }
        .into();
        let fmt = hdrop_accept_format();
        assert_eq!(unsafe { data_obj.QueryGetData(&fmt) }, S_OK);

        let mut medium = unsafe { data_obj.GetData(&fmt) }.expect("CF_HDROP medium");
        assert_eq!(medium.tymed, TYMED_HGLOBAL.0 as u32);
        let hdrop = unsafe { HDROP(medium.u.hGlobal.0) };
        let extracted = unsafe { extract_hdrop_paths(hdrop) };
        unsafe { ReleaseStgMedium(&mut medium) };

        assert_eq!(extracted, vec![p1.clone(), p2.clone()]);

        // Kiểm tra EnumFormatEtc chỉ trả về cho DATADIR_GET (1), từ chối DATADIR_SET (2).
        assert!(unsafe { data_obj.EnumFormatEtc(DATADIR_GET.0 as u32) }.is_ok());
        assert!(unsafe { data_obj.EnumFormatEtc(2) }.is_err());

        // Kiểm tra IslandDropTarget::DragEnter + Drop nhận đúng đường dẫn và giữ bitmask effect.
        let (tx, rx) = std::sync::mpsc::channel();
        let drop_target: IDropTarget = IslandDropTarget::new(HWND::default(), tx).into();
        let mut eff = DROPEFFECT_COPY | DROPEFFECT_MOVE;
        unsafe {
            drop_target
                .DragEnter(
                    &data_obj,
                    MODIFIERKEYS_FLAGS(MK_LBUTTON.0),
                    POINTL { x: 100, y: 20 },
                    &mut eff,
                )
                .expect("DragEnter must succeed");
        }
        assert_eq!(eff, DROPEFFECT_COPY);
        unsafe {
            drop_target
                .Drop(
                    &data_obj,
                    MODIFIERKEYS_FLAGS(0),
                    POINTL { x: 100, y: 20 },
                    &mut eff,
                )
                .expect("Drop must succeed");
        }
        assert_eq!(eff, DROPEFFECT_COPY);
        assert_eq!(rx.try_recv().unwrap(), vec![p1.clone(), p2.clone()]);

        let _ = fs::remove_file(&p1);
        let _ = fs::remove_file(&p2);
        if let Some(dir) = p1.parent() {
            let _ = fs::remove_dir(dir);
        }
    }
}
