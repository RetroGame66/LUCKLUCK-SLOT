use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::collections::{HashMap, HashSet};
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const DEV: &str = "/dev/mtp_usb";
const TYPE_COMMAND: u16 = 1;
const TYPE_DATA: u16 = 2;
const TYPE_RESPONSE: u16 = 3;
const OK: u16 = 0x2001;
const OP_NOT_SUPPORTED: u16 = 0x2005;
const SESSION_NOT_OPEN: u16 = 0x2003;
const STORAGE_ID: u32 = 0x0001_0001;
const STORAGE_DESC: &str = "SD Card";

const GET_DEVICE_INFO: u16 = 0x1001;
const OPEN_SESSION: u16 = 0x1002;
const CLOSE_SESSION: u16 = 0x1003;
const GET_STORAGE_IDS: u16 = 0x1004;
const GET_STORAGE_INFO: u16 = 0x1005;
const GET_NUM_OBJECTS: u16 = 0x1006;
const GET_OBJECT_HANDLES: u16 = 0x1007;
const GET_OBJECT_INFO: u16 = 0x1008;
const GET_OBJECT: u16 = 0x1009;
const DELETE_OBJECT: u16 = 0x100b;
const GET_DEVICE_PROP_DESC: u16 = 0x1014;
const GET_DEVICE_PROP_VALUE: u16 = 0x1015;
const SET_DEVICE_PROP_VALUE: u16 = 0x1016;
const GET_PARTIAL_OBJECT: u16 = 0x101b;
const GET_OBJECT_PROPS_SUPPORTED: u16 = 0x9801;
const GET_OBJECT_PROP_DESC: u16 = 0x9802;
const GET_OBJECT_PROP_VALUE: u16 = 0x9803;

const FORMAT_UNDEFINED: u16 = 0x3000;
const FORMAT_ASSOCIATION: u16 = 0x3001;
const PROP_STORAGE_ID: u16 = 0xdc01;
const PROP_OBJECT_FORMAT: u16 = 0xdc02;
const PROP_PROTECTION_STATUS: u16 = 0xdc03;
const PROP_OBJECT_SIZE: u16 = 0xdc04;
const PROP_ASSOCIATION_TYPE: u16 = 0xdc05;
const PROP_ASSOCIATION_DESC: u16 = 0xdc06;
const PROP_FILE_NAME: u16 = 0xdc07;
const PROP_PARENT: u16 = 0xdc0b;
const PROP_PERSISTENT_UID: u16 = 0xdc41;
const PROP_NAME: u16 = 0xdc44;
const PROP_DATE_CREATED: u16 = 0xdc08;
const PROP_DATE_MODIFIED: u16 = 0xdc09;
// GetObjectHandles uses 0 for all objects and 0xFFFFFFFF for storage-root children.
const ALL_OBJECTS: u32 = 0;
const ALL_STORAGES: u32 = u32::MAX;
const ROOT_PARENT: u32 = u32::MAX;

#[repr(C)]
struct MtpFileRange {
    fd: i32,
    offset: i64,
    length: i64,
    command: u16,
    transaction_id: u32,
}

#[allow(dead_code)]
#[repr(C)]
struct MtpEvent {
    length: usize,
    data: *const u8,
}

unsafe extern "C" {
    fn ioctl(fd: RawFd, request: usize, ...) -> i32;
}

#[allow(dead_code)]
const MTP_RECEIVE_FILE: usize = 0x4020_4d01;
#[allow(dead_code)]
const MTP_SEND_EVENT: usize = 0x4010_4d03;
const MTP_SEND_FILE_WITH_HEADER: usize = 0x4020_4d04;

#[derive(Clone)]
struct Object {
    handle: u32,
    parent: u32,
    path: PathBuf,
    directory: bool,
}

struct Server {
    root: PathBuf,
    device: Option<File>,
    session: bool,
    session_id: u32,
    read_size: usize,
    objects: Vec<Object>,
    handles_by_path: HashMap<PathBuf, u32>,
    used_handles: HashSet<u32>,
    next_handle: u32,
    pending_handles_parent: Option<u32>,
    all_objects_indexed: bool,
    logged_hierarchy_parents: HashSet<u32>,
    hierarchy_log_budget: usize,
}

fn push_u16(v: &mut Vec<u8>, n: u16) { v.extend_from_slice(&n.to_le_bytes()); }
fn push_u32(v: &mut Vec<u8>, n: u32) { v.extend_from_slice(&n.to_le_bytes()); }
fn push_u64(v: &mut Vec<u8>, n: u64) { v.extend_from_slice(&n.to_le_bytes()); }
fn push_string(v: &mut Vec<u8>, s: &str) {
    let units: Vec<u16> = s.encode_utf16().collect();
    // MTP uses a zero byte as the complete representation of an empty
    // string. Do not append the UTF-16 terminator in that case: doing so
    // shifts every field that follows an empty string in the dataset.
    if units.is_empty() {
        v.push(0);
        return;
    }
    v.push((units.len().saturating_add(1)).min(255) as u8);
    for c in units.into_iter().take(254) { push_u16(v, c); }
    push_u16(v, 0);
}
fn push_array_u16(v: &mut Vec<u8>, values: &[u16]) {
    push_u32(v, values.len() as u32);
    for value in values { push_u16(v, *value); }
}

fn persistent_uid(path: &Path) -> (u64, u64) {
    // Windows uses PersistentUID to retain the WPD object identity between
    // sessions. Object handles are intentionally session-local, so derive a
    // stable 128-bit value from the absolute storage path instead.
    let bytes = path.to_string_lossy();
    let mut first = 0xcbf29ce484222325u64;
    let mut second = 0x84222325cbf29ce4u64;
    for byte in bytes.as_bytes() {
        first ^= u64::from(*byte);
        first = first.wrapping_mul(0x100000001b3);
        second ^= u64::from(*byte).wrapping_add(0x9e);
        second = second.wrapping_mul(0x100000001b3);
    }
    (first, second)
}
fn get_u16(b: &[u8], at: usize) -> u16 {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]])).unwrap_or(0)
}
fn get_u32(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])).unwrap_or(0)
}
fn packet(kind: u16, code: u16, tx: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + payload.len());
    push_u32(&mut out, (12 + payload.len()) as u32);
    push_u16(&mut out, kind);
    push_u16(&mut out, code);
    push_u32(&mut out, tx);
    out.extend_from_slice(payload);
    out
}
fn response(code: u16, tx: u32, params: &[u32]) -> Vec<u8> {
    let mut p = Vec::with_capacity(params.len() * 4);
    for n in params { push_u32(&mut p, *n); }
    packet(TYPE_RESPONSE, code, tx, &p)
}
fn data(code: u16, tx: u32, payload: &[u8]) -> Vec<u8> { packet(TYPE_DATA, code, tx, payload) }

fn send_data_response(server: &mut Server, code: u16, tx: u32, payload: &[u8]) -> io::Result<()> {
    server.write_packet(&data(code, tx, payload))?;
    server.write_packet(&response(OK, tx, &[]))
}

fn log_line(root: &Path, msg: &str) {
    let dir = root.join("System/log");
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(dir.join("mtp.log")) {
        let _ = writeln!(f, "mtp-responder: {msg}");
    }
}

fn collect_objects(
    root: &Path,
    handles_by_path: &mut HashMap<PathBuf, u32>,
    used_handles: &mut HashSet<u32>,
    next_handle: &mut u32,
) -> Vec<Object> {
    let mut objects = Vec::new();
    fn walk(
        path: &Path,
        parent: u32,
        handles_by_path: &mut HashMap<PathBuf, u32>,
        used_handles: &mut HashSet<u32>,
        next_handle: &mut u32,
        out: &mut Vec<Object>,
    ) {
        let mut entries = match fs::read_dir(path) {
            Ok(v) => v.filter_map(Result::ok).collect::<Vec<_>>(),
            Err(_) => return,
        };
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if is_runtime_artifact(&entry.file_name()) {
                continue;
            }
            let p = entry.path();
            let file_type = match entry.file_type() {
                Ok(value) if !value.is_symlink() => value,
                _ => continue,
            };
            let directory = file_type.is_dir();
            let handle = *handles_by_path.entry(p.clone()).or_insert_with(|| {
                loop {
                    let candidate = *next_handle;
                    *next_handle = next_handle.wrapping_add(1).max(1);
                    if candidate != 0 && candidate != ROOT_PARENT && used_handles.insert(candidate) {
                        break candidate;
                    }
                }
            });
            out.push(Object { handle, parent, path: p.clone(), directory });
            if directory {
                walk(&p, handle, handles_by_path, used_handles, next_handle, out);
            }
        }
    }
    walk(root, 0, handles_by_path, used_handles, next_handle, &mut objects);
    objects
}

fn collect_children(
    path: &Path,
    parent: u32,
    handles_by_path: &mut HashMap<PathBuf, u32>,
    used_handles: &mut HashSet<u32>,
    next_handle: &mut u32,
) -> io::Result<Vec<Object>> {
    let mut entries = fs::read_dir(path)?.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    let mut children = Vec::with_capacity(entries.len());
    for entry in entries {
        let p = entry.path();
        if is_runtime_artifact(&entry.file_name()) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }
        let directory = file_type.is_dir();
        let handle = *handles_by_path.entry(p.clone()).or_insert_with(|| {
            loop {
                let candidate = *next_handle;
                *next_handle = next_handle.wrapping_add(1).max(1);
                if candidate != 0 && candidate != ROOT_PARENT && used_handles.insert(candidate) {
                    break candidate;
                }
            }
        });
        children.push(Object { handle, parent, path: p, directory });
    }
    Ok(children)
}

fn is_runtime_artifact(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name == ".mtp-auto.lock"
        || name == ".mtp-responder.pid"
        || name == ".mtp-last-state"
        || name.starts_with(".mtp-config-links.")
        || name.starts_with(".mtp-layout-ready-")
}

fn matches_parent(obj: &Object, parent: u32) -> bool {
    // Root children have ParentObject=0 in ObjectInfo, separate from the root query sentinel.
    if parent == ALL_OBJECTS {
        true
    } else if parent == ROOT_PARENT {
        obj.parent == 0
    } else {
        obj.parent == parent
    }
}

fn object_info(obj: &Object) -> Vec<u8> {
    let name = obj.path.file_name().and_then(|s| s.to_str()).unwrap_or("Unnamed");
    let format = if obj.directory { FORMAT_ASSOCIATION } else { FORMAT_UNDEFINED };
    let size = if obj.directory { 0 } else { fs::metadata(&obj.path).map(|m| m.len()).unwrap_or(0) };
    let mut out = Vec::new();
    push_u32(&mut out, STORAGE_ID);
    push_u16(&mut out, format);
    push_u16(&mut out, 0);
    push_u32(&mut out, size.min(u32::MAX as u64) as u32);
    push_u16(&mut out, 0); push_u32(&mut out, 0); push_u32(&mut out, 0); push_u32(&mut out, 0);
    push_u32(&mut out, 0); push_u32(&mut out, 0); push_u32(&mut out, 0);
    push_u32(&mut out, obj.parent);
    push_u16(&mut out, if obj.directory { 1 } else { 0 });
    push_u32(&mut out, 0); push_u32(&mut out, 0);
    push_string(&mut out, name);
    push_string(&mut out, ""); push_string(&mut out, ""); push_string(&mut out, "");
    out
}

fn device_info() -> Vec<u8> {
    let ops = [GET_DEVICE_INFO, OPEN_SESSION, CLOSE_SESSION, GET_STORAGE_IDS, GET_STORAGE_INFO,
        GET_NUM_OBJECTS, GET_OBJECT_HANDLES, GET_OBJECT_INFO, GET_OBJECT, DELETE_OBJECT,
        GET_DEVICE_PROP_DESC, GET_DEVICE_PROP_VALUE, SET_DEVICE_PROP_VALUE, GET_PARTIAL_OBJECT,
        GET_OBJECT_PROPS_SUPPORTED, GET_OBJECT_PROP_DESC, GET_OBJECT_PROP_VALUE];
    let events: [u16; 0] = [];
    let props = [0x5001u16, 0x5003u16];
    let mut out = Vec::new();
    push_u16(&mut out, 100); push_u32(&mut out, 6); push_u16(&mut out, 100);
    push_string(&mut out, "microsoft.com: 1.0; android.com: 1.0;");
    push_u16(&mut out, 0);
    push_array_u16(&mut out, &ops); push_array_u16(&mut out, &events); push_array_u16(&mut out, &props);
    push_array_u16(&mut out, &[]); push_array_u16(&mut out, &[FORMAT_UNDEFINED, FORMAT_ASSOCIATION]);
    push_string(&mut out, "Slot"); push_string(&mut out, "Slot"); push_string(&mut out, "1.0"); push_string(&mut out, "00000001");
    out
}

fn prop_supported() -> Vec<u8> {
    let props = [PROP_STORAGE_ID, PROP_OBJECT_FORMAT, PROP_PROTECTION_STATUS, PROP_OBJECT_SIZE,
        PROP_ASSOCIATION_TYPE, PROP_ASSOCIATION_DESC, PROP_FILE_NAME, PROP_DATE_CREATED,
        PROP_DATE_MODIFIED, PROP_PARENT, PROP_PERSISTENT_UID, PROP_NAME];
    let mut out = Vec::new(); push_array_u16(&mut out, &props); out
}

fn object_prop_desc(prop: u16) -> Option<Vec<u8>> {
    let data_type = match prop {
        PROP_STORAGE_ID | PROP_PARENT => 0x0006,
        PROP_OBJECT_FORMAT | PROP_PROTECTION_STATUS | PROP_ASSOCIATION_TYPE => 0x0004,
        PROP_ASSOCIATION_DESC => 0x0006,
        PROP_OBJECT_SIZE => 0x0008,
        PROP_FILE_NAME | PROP_NAME | PROP_DATE_CREATED | PROP_DATE_MODIFIED => 0xffff,
        PROP_PERSISTENT_UID => 0x000a,
        _ => return None,
    };
    let mut out = Vec::new();
    push_u16(&mut out, prop);
    push_u16(&mut out, data_type);
    out.push(0);
    match data_type {
        0x0004 => push_u16(&mut out, 0),
        0x0006 => push_u32(&mut out, 0),
        0x0008 => push_u64(&mut out, 0),
        0x000a => { push_u64(&mut out, 0); push_u64(&mut out, 0); }
        0xffff => push_string(&mut out, ""),
        _ => unreachable!(),
    }
    push_u32(&mut out, 0);
    out.push(0);
    Some(out)
}

fn device_prop_desc(prop: u16) -> Option<Vec<u8>> {
    let data_type = match prop {
        0x5001 => 0x0002,
        0x5003 => 0xffff,
        _ => return None,
    };
    let mut out = Vec::new();
    push_u16(&mut out, prop);
    push_u16(&mut out, data_type);
    out.push(0);
    if data_type == 0x0002 {
        out.push(100);
        out.push(100);
    } else {
        push_string(&mut out, "");
        push_string(&mut out, "");
    }
    out.push(0);
    Some(out)
}

fn prop_value(obj: &Object, prop: u16) -> Vec<u8> {
    let mut out = Vec::new();
    match prop {
        PROP_STORAGE_ID => push_u32(&mut out, STORAGE_ID),
        PROP_OBJECT_FORMAT => push_u16(&mut out, if obj.directory { FORMAT_ASSOCIATION } else { FORMAT_UNDEFINED }),
        PROP_PROTECTION_STATUS => push_u16(&mut out, 0),
        PROP_OBJECT_SIZE => push_u64(&mut out, if obj.directory { 0 } else { fs::metadata(&obj.path).map(|m| m.len()).unwrap_or(0) }),
        PROP_ASSOCIATION_TYPE => push_u16(&mut out, if obj.directory { 1 } else { 0 }),
        PROP_ASSOCIATION_DESC => push_u32(&mut out, 0),
        PROP_PARENT => push_u32(&mut out, obj.parent),
        PROP_FILE_NAME | PROP_NAME => push_string(&mut out, obj.path.file_name().and_then(|s| s.to_str()).unwrap_or("Unnamed")),
        PROP_DATE_CREATED | PROP_DATE_MODIFIED => push_string(&mut out, ""),
        PROP_PERSISTENT_UID => {
            let (high, low) = persistent_uid(&obj.path);
            push_u64(&mut out, low);
            push_u64(&mut out, high);
        }
        _ => {}
    }
    out
}

fn send_file_ioctl(device: &File, obj: &Object, op: u16, tx: u32, offset: u64, length: u64) -> io::Result<()> {
    let file = File::open(&obj.path)?;
    let mut range = MtpFileRange { fd: file.as_raw_fd(), offset: offset as i64, length: length as i64, command: op, transaction_id: tx };
    let ret = unsafe { ioctl(device.as_raw_fd(), MTP_SEND_FILE_WITH_HEADER, &mut range as *mut MtpFileRange) };
    if ret < 0 { return Err(io::Error::last_os_error()); }
    Ok(())
}

impl Server {
    fn refresh_children(&mut self, parent: u32) {
        let parent_path = if parent == ROOT_PARENT {
            self.root.clone()
        } else {
            match self.objects.iter().find(|object| object.handle == parent && object.directory) {
                Some(object) => object.path.clone(),
                None => return,
            }
        };
        let children = match collect_children(
            &parent_path,
            if parent == ROOT_PARENT { 0 } else { parent },
            &mut self.handles_by_path,
            &mut self.used_handles,
            &mut self.next_handle,
        ) {
            Ok(children) => children,
            Err(error) => {
                log_line(&self.root, &format!("directory scan failed for {}: {error}", parent_path.display()));
                return;
            }
        };
        let present: HashSet<PathBuf> = children.iter().map(|object| object.path.clone()).collect();
        let removed_directories: Vec<PathBuf> = self.objects.iter()
            .filter(|object| object.parent == parent && object.directory && !present.contains(&object.path))
            .map(|object| object.path.clone())
            .collect();
        self.objects.retain(|object| {
            (object.parent != parent || present.contains(&object.path))
                && !removed_directories.iter().any(|path| object.path.starts_with(path))
        });
        let existing_indices: HashMap<PathBuf, usize> = self.objects.iter().enumerate()
            .map(|(index, object)| (object.path.clone(), index))
            .collect();
        for child in children {
            if let Some(index) = existing_indices.get(&child.path).copied() {
                let existing = &mut self.objects[index];
                existing.parent = child.parent;
                existing.directory = child.directory;
            } else {
                self.objects.push(child);
            }
        }
    }

    fn refresh_objects(&mut self) {
        if self.all_objects_indexed {
            return;
        }
        self.objects = collect_objects(
            &self.root,
            &mut self.handles_by_path,
            &mut self.used_handles,
            &mut self.next_handle,
        );
        self.all_objects_indexed = true;
    }

    fn open_session(&mut self, session_id: u32) {
        self.session = true;
        self.session_id = session_id;
        self.objects.clear();
        self.handles_by_path.clear();
        self.used_handles.clear();
        self.next_handle = 1;
        self.pending_handles_parent = None;
        self.all_objects_indexed = false;
    }

    fn close_session(&mut self) {
        self.session = false;
        self.objects.clear();
        self.handles_by_path.clear();
        self.used_handles.clear();
        self.next_handle = 1;
        self.pending_handles_parent = None;
        self.all_objects_indexed = false;
    }

    fn open_device(&mut self) -> io::Result<()> {
        self.device = Some(OpenOptions::new().read(true).write(true).open(DEV)?);
        log_line(&self.root, "opened /dev/mtp_usb");
        Ok(())
    }

    fn write_packet(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self.device.as_mut() {
            Some(d) => d.write_all(bytes),
            None => Err(io::Error::new(io::ErrorKind::NotConnected, "mtp device closed")),
        }
    }

    fn read_packet(&mut self) -> io::Result<Vec<u8>> {
        // The vendor f_mtp driver advertises a 16 KiB public limit, but some
        // board builds allocate a smaller RX request buffer and return EINVAL
        // when the userspace read is too large. Adapt down until the driver
        // accepts the request and keep the working size for this connection.
        loop {
            let mut buf = vec![0u8; self.read_size];
            let result = match self.device.as_mut() {
                Some(d) => d.read(&mut buf),
                None => return Err(io::Error::new(io::ErrorKind::NotConnected, "mtp device closed")),
            };
            match result {
                Ok(n) => {
                    if n == 0 { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "mtp eof")); }
                    buf.truncate(n);
                    if n >= 4 {
                        let declared = get_u32(&buf, 0) as usize;
                        if declared >= 12 && declared <= n { buf.truncate(declared); }
                    }
                    return Ok(buf);
                }
                Err(e) if e.raw_os_error() == Some(22) && self.read_size > 512 => {
                    self.read_size = (self.read_size / 2).max(512);
                    log_line(&self.root, &format!("read buffer rejected; retrying with {} bytes", self.read_size));
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn handle(&mut self, req: &[u8]) -> io::Result<()> {
        if req.len() < 12 { return Ok(()); }
        let kind = get_u16(req, 4); let op = get_u16(req, 6); let tx = get_u32(req, 8);
        if kind != TYPE_COMMAND { return Ok(()); }
        if op != GET_OBJECT_HANDLES {
            self.pending_handles_parent = None;
        }
        let session_required = op != GET_DEVICE_INFO && op != OPEN_SESSION;
        if session_required && !self.session {
            return self.write_packet(&response(SESSION_NOT_OPEN, tx, &[]));
        }
        match op {
            GET_DEVICE_INFO => send_data_response(self, op, tx, &device_info()),
            OPEN_SESSION => { self.open_session(get_u32(req, 12)); self.write_packet(&response(OK, tx, &[])) },
            CLOSE_SESSION => { self.close_session(); self.write_packet(&response(OK, tx, &[])) },
            GET_STORAGE_IDS => { let mut p = Vec::new(); push_u32(&mut p, 1); push_u32(&mut p, STORAGE_ID); send_data_response(self, op, tx, &p) },
            GET_STORAGE_INFO => { let mut p = Vec::new(); push_u16(&mut p, 3); push_u16(&mut p, 2); push_u16(&mut p, 0); push_u64(&mut p, 32 * 1024 * 1024 * 1024); push_u64(&mut p, 16 * 1024 * 1024 * 1024); push_u32(&mut p, 0xffff_ffff); push_string(&mut p, STORAGE_DESC); push_string(&mut p, "SLOT"); send_data_response(self, op, tx, &p) },
            GET_NUM_OBJECTS => {
                let storage = get_u32(req, 12);
                let format = get_u32(req, 16) as u16;
                let parent = get_u32(req, 20);
                if parent == ALL_OBJECTS {
                    self.refresh_objects();
                } else {
                    self.refresh_children(parent);
                }
                self.pending_handles_parent = Some(parent);
                let count = if storage != 0 && storage != ALL_STORAGES && storage != STORAGE_ID { 0 } else {
                    self.objects.iter().filter(|o| matches_parent(o, parent) && (format == 0 || (if o.directory { FORMAT_ASSOCIATION } else { FORMAT_UNDEFINED }) == format)).count() as u32
                };
                self.write_packet(&response(OK, tx, &[count]))
            },
            GET_OBJECT_HANDLES => {
                let storage = get_u32(req, 12);
                let format = get_u32(req, 16) as u16;
                let parent = get_u32(req, 20);
                if self.pending_handles_parent != Some(parent) {
                    if parent == ALL_OBJECTS {
                        self.refresh_objects();
                    } else {
                        self.refresh_children(parent);
                    }
                }
                self.pending_handles_parent = None;
                let selected: Vec<u32> = if storage != 0 && storage != ALL_STORAGES && storage != STORAGE_ID { Vec::new() } else {
                    let mut selected: Vec<&Object> = self.objects.iter()
                        .filter(|o| matches_parent(o, parent) && (format == 0 || (if o.directory { FORMAT_ASSOCIATION } else { FORMAT_UNDEFINED }) == format))
                        .collect();
                    selected.sort_by(|left, right| left.path.file_name().cmp(&right.path.file_name()));
                    selected.into_iter().map(|o| o.handle).collect()
                };
                if self.hierarchy_log_budget > 0 && self.logged_hierarchy_parents.insert(parent) {
                    let parent_path = if parent == ROOT_PARENT {
                        Some(self.root.as_path())
                    } else {
                        self.objects.iter().find(|object| object.handle == parent && object.directory)
                            .map(|object| object.path.as_path())
                    };
                    let label = match parent_path {
                        Some(_) if parent == ROOT_PARENT => "root".to_owned(),
                        Some(path) => path.strip_prefix(&self.root).unwrap_or(path).display().to_string(),
                        None if parent == ALL_OBJECTS => "all".to_owned(),
                        None => "invalid-parent".to_owned(),
                    };
                    let entries: Vec<&Object> = self.objects.iter()
                        .filter(|object| matches_parent(object, parent)
                            && (format == 0 || (if object.directory { FORMAT_ASSOCIATION } else { FORMAT_UNDEFINED }) == format))
                        .collect();
                    let dirs = entries.iter().filter(|object| object.directory).count();
                    let files = entries.len().saturating_sub(dirs);
                    let sample = entries.iter().take(3).map(|object| {
                        let name = object.path.file_name().and_then(|name| name.to_str()).unwrap_or("?");
                        if object.directory { format!("{name}/") } else { name.to_owned() }
                    }).collect::<Vec<_>>().join(",");
                    log_line(&self.root, &format!(
                        "handles parent={label:?} id={parent:#010x} count={} dirs={dirs} files={files} sample=[{sample}]",
                        entries.len()
                    ));
                    self.hierarchy_log_budget -= 1;
                }
                let mut p = Vec::new(); push_u32(&mut p, selected.len() as u32); for h in selected { push_u32(&mut p, h); }
                send_data_response(self, op, tx, &p)
            },
            GET_OBJECT_INFO => { let handle = get_u32(req, 12); match self.objects.iter().find(|o| o.handle == handle) { Some(o) => send_data_response(self, op, tx, &object_info(o)), None => self.write_packet(&response(0x2009, tx, &[])) } },
            GET_OBJECT | GET_PARTIAL_OBJECT => { let handle = get_u32(req, 12); if let Some(o) = self.objects.iter().find(|o| o.handle == handle && !o.directory) { let size = fs::metadata(&o.path).map(|m| m.len()).unwrap_or(0); let offset = if op == GET_PARTIAL_OBJECT { get_u32(req, 16) as u64 } else { 0 }; let requested = if op == GET_PARTIAL_OBJECT { get_u32(req, 20) as u64 } else { size.saturating_sub(offset) }; let length = requested.min(size.saturating_sub(offset)); send_file_ioctl(self.device.as_ref().unwrap(), o, op, tx, offset, length)?; let params = if op == GET_PARTIAL_OBJECT { vec![length as u32] } else { Vec::new() }; self.write_packet(&response(OK, tx, &params)) } else { self.write_packet(&response(0x2009, tx, &[])) } },
            DELETE_OBJECT => self.write_packet(&response(OK, tx, &[])),
            GET_OBJECT_PROPS_SUPPORTED => send_data_response(self, op, tx, &prop_supported()),
            GET_OBJECT_PROP_DESC => {
                let prop = get_u32(req, 12) as u16;
                match object_prop_desc(prop) {
                    Some(payload) => send_data_response(self, op, tx, &payload),
                    None => self.write_packet(&response(OP_NOT_SUPPORTED, tx, &[])),
                }
            },
            GET_OBJECT_PROP_VALUE => { let handle = get_u32(req, 12); let prop = get_u32(req, 16) as u16; if let Some(o) = self.objects.iter().find(|o| o.handle == handle) { send_data_response(self, op, tx, &prop_value(o, prop)) } else { self.write_packet(&response(0x2009, tx, &[])) } },
            GET_DEVICE_PROP_DESC => {
                let prop = get_u32(req, 12) as u16;
                match device_prop_desc(prop) {
                    Some(payload) => send_data_response(self, op, tx, &payload),
                    None => self.write_packet(&response(OP_NOT_SUPPORTED, tx, &[])),
                }
            },
            GET_DEVICE_PROP_VALUE => {
                let payload = match get_u32(req, 12) as u16 {
                    0x5001 => vec![100],
                    0x5003 => { let mut value = Vec::new(); push_string(&mut value, ""); value },
                    _ => return self.write_packet(&response(OP_NOT_SUPPORTED, tx, &[])),
                };
                send_data_response(self, op, tx, &payload)
            },
            SET_DEVICE_PROP_VALUE => self.write_packet(&response(OK, tx, &[])),
            _ => self.write_packet(&response(OP_NOT_SUPPORTED, tx, &[])),
        }
    }
}

fn main() {
    let root = PathBuf::from("/mnt/sdcard");
    if !root.is_dir() {
        log_line(&root, "storage root /mnt/sdcard is unavailable");
        return;
    }
    let mut server = Server {
        root,
        device: None,
        session: false,
        session_id: 0,
        read_size: 16 * 1024,
        objects: Vec::new(),
        handles_by_path: HashMap::new(),
        used_handles: HashSet::new(),
        next_handle: 1,
        pending_handles_parent: None,
        all_objects_indexed: false,
        logged_hierarchy_parents: HashSet::new(),
        hierarchy_log_budget: 16,
    };
    log_line(&server.root, &format!("responder started, root={}", server.root.display()));
    let mut waiting_for_configuration = false;
    let mut open_failures = 0usize;
    loop {
        if server.device.is_none() {
            if let Err(e) = server.open_device() {
                if open_failures == 0 || open_failures % 20 == 0 {
                    log_line(&server.root, &format!("open failed: {e}; retrying"));
                }
                open_failures += 1;
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            open_failures = 0;
        }
        match server.read_packet() {
            Ok(req) => {
                waiting_for_configuration = false;
                if let Err(e) = server.handle(&req) { log_line(&server.root, &format!("request failed: {e}")); server.device = None; server.session = false; thread::sleep(Duration::from_millis(250)); }
            },
            Err(e) => {
                let code = e.raw_os_error().unwrap_or(0);
                if code == 5 {
                    if !waiting_for_configuration {
                        log_line(&server.root, "USB not configured (EIO), waiting on open MTP device");
                        waiting_for_configuration = true;
                    }
                    // f_mtp returns EIO before SET_CONFIGURATION. Keep the
                    // file open so the first host request is queued as soon
                    // as the gadget becomes active.
                    thread::sleep(Duration::from_millis(50));
                } else {
                    log_line(&server.root, &format!("read failed: {e}"));
                    server.device = None;
                    server.session = false;
                    waiting_for_configuration = false;
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root() -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("mtp-responder-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn test_server(root: PathBuf) -> Server {
        Server {
            root,
            device: None,
            session: false,
            session_id: 0,
            read_size: 16 * 1024,
            objects: Vec::new(),
            handles_by_path: HashMap::new(),
            used_handles: HashSet::new(),
            next_handle: 1,
            pending_handles_parent: None,
            all_objects_indexed: false,
            logged_hierarchy_parents: HashSet::new(),
            hierarchy_log_budget: 16,
        }
    }

    #[test]
    fn directory_parent_and_handles_survive_root_changes() {
        let root = test_root();
        let folder = root.join("Folder");
        fs::create_dir(&folder).unwrap();
        let nested_file = folder.join("inside.txt");
        fs::write(&nested_file, b"inside").unwrap();

        let mut server = test_server(root.clone());
        server.open_session(1);
        server.refresh_children(ROOT_PARENT);
        let folder_handle = server.objects.iter().find(|object| object.path == folder).unwrap().handle;
        server.refresh_children(folder_handle);
        let nested_handle = server.objects.iter().find(|object| object.path == nested_file).unwrap().handle;

        fs::write(root.join("A-root-file.txt"), b"root").unwrap();
        server.refresh_children(ROOT_PARENT);
        server.refresh_children(folder_handle);

        let directory = server.objects.iter().find(|object| object.path == folder).unwrap();
        let file = server.objects.iter().find(|object| object.path == nested_file).unwrap();
        assert_eq!(directory.handle, folder_handle);
        assert_eq!(file.handle, nested_handle);
        assert_eq!(file.parent, folder_handle);
        assert_eq!(get_u32(&object_info(file), 38), folder_handle);

        server.close_session();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nested_files_are_only_returned_for_their_actual_parent() {
        let root = test_root();
        let folder = root.join("Folder");
        let nested = folder.join("Nested");
        fs::create_dir_all(&nested).unwrap();
        let root_file = root.join("root.txt");
        let nested_file = nested.join("inside.txt");
        fs::write(&root_file, b"root").unwrap();
        fs::write(&nested_file, b"inside").unwrap();

        let mut server = test_server(root.clone());
        server.open_session(1);
        server.refresh_children(ROOT_PARENT);
        let folder_handle = server.objects.iter().find(|object| object.path == folder).unwrap().handle;
        let root_children: Vec<&Object> = server.objects.iter()
            .filter(|object| matches_parent(object, ROOT_PARENT)).collect();
        assert!(root_children.iter().any(|object| object.path == root_file));
        assert!(root_children.iter().any(|object| object.path == folder));
        assert!(!root_children.iter().any(|object| object.path == nested_file));

        server.refresh_children(folder_handle);
        let nested_handle = server.objects.iter().find(|object| object.path == nested).unwrap().handle;
        server.refresh_children(nested_handle);
        let nested_object = server.objects.iter().find(|object| object.path == nested_file).unwrap();
        assert_eq!(nested_object.parent, nested_handle);
        assert_eq!(get_u32(&object_info(nested_object), 38), nested_handle);
        assert!(!server.objects.iter().filter(|object| matches_parent(object, ROOT_PARENT))
            .any(|object| object.path == nested_file));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_lock_files_are_not_exposed_as_storage_objects() {
        let root = test_root();
        fs::create_dir_all(root.join("System/log/.mtp-auto.lock")).unwrap();
        fs::write(root.join("System/log/mtp.log"), b"debug").unwrap();
        let mut server = test_server(root.clone());
        server.open_session(1);
        server.refresh_children(ROOT_PARENT);
        let system_handle = server.objects.iter().find(|object| object.path == root.join("System")).unwrap().handle;
        server.refresh_children(system_handle);
        let log_dir_handle = server.objects.iter().find(|object| object.path == root.join("System/log")).unwrap().handle;
        server.refresh_children(log_dir_handle);

        assert!(!server.objects.iter().any(|object| object.path.ends_with(".mtp-auto.lock")));
        assert!(server.objects.iter().any(|object| object.path.ends_with("mtp.log")));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn all_object_inventory_is_cached_until_session_ends() {
        let root = test_root();
        let folder = root.join("Folder");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("first.txt"), b"first").unwrap();

        let mut server = test_server(root.clone());
        server.open_session(1);
        server.refresh_objects();
        assert!(server.all_objects_indexed);

        let folder_handle = server.objects.iter().find(|object| object.path == folder).unwrap().handle;
        let later_file = folder.join("later.txt");
        fs::write(&later_file, b"later").unwrap();
        server.refresh_objects();
        assert!(!server.objects.iter().any(|object| object.path == later_file));

        server.refresh_children(folder_handle);
        assert!(server.objects.iter().any(|object| object.path == later_file));
        server.refresh_objects();
        assert!(server.objects.iter().any(|object| object.path == later_file));

        server.close_session();
        assert!(!server.all_objects_indexed);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_mtp_strings_do_not_add_bytes() {
        let mut encoded = Vec::new();
        push_string(&mut encoded, "");
        assert_eq!(encoded, vec![0]);

        let root = test_root();
        let folder = root.join("Folder");
        fs::create_dir(&folder).unwrap();
        let file = folder.join("inside.txt");
        fs::write(&file, b"inside").unwrap();
        let object = Object { handle: 7, parent: 3, path: file, directory: false };
        let info = object_info(&object);
        assert_eq!(get_u32(&info, 38), 3);
        let filename_start = 52;
        assert_eq!(info[filename_start], 1 + "inside.txt".encode_utf16().count() as u8);
        let name_end = filename_start + 1 + ("inside.txt".encode_utf16().count() + 1) * 2;
        assert_eq!(info[name_end], 0);
        assert_eq!(info.len(), name_end + 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn root_parent_and_all_objects_sentinels_are_distinct() {
        let root = test_root();
        let folder = root.join("Folder");
        fs::create_dir(&folder).unwrap();
        fs::write(root.join("root.txt"), b"root").unwrap();
        fs::write(folder.join("inside.txt"), b"inside").unwrap();

        let mut server = test_server(root.clone());
        server.open_session(1);
        server.refresh_children(ROOT_PARENT);

        let root_objects: Vec<&Object> = server.objects.iter()
            .filter(|object| matches_parent(object, ROOT_PARENT)).collect();
        assert_eq!(root_objects.len(), 2);
        assert!(root_objects.iter().all(|object| object.parent == 0));
        assert!(root_objects.iter().any(|object| object.path == folder));
        assert!(root_objects.iter().any(|object| object.path == root.join("root.txt")));
        assert!(!root_objects.iter().any(|object| object.path.ends_with("inside.txt")));

        server.refresh_objects();
        let all_objects: Vec<&Object> = server.objects.iter()
            .filter(|object| matches_parent(object, ALL_OBJECTS)).collect();
        assert_eq!(all_objects.len(), 3);
        assert!(all_objects.iter().any(|object| object.path.ends_with("inside.txt")));
        let folder_object = all_objects.iter().find(|object| object.path == folder).unwrap();
        assert_eq!(get_u32(&object_info(folder_object), 38), 0);

        server.close_session();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn association_properties_identify_directories() {
        let root = test_root();
        let folder = root.join("Folder");
        fs::create_dir(&folder).unwrap();
        let directory = Object { handle: 11, parent: 0, path: folder, directory: true };
        let file = Object { handle: 12, parent: 11, path: root.join("inside.txt"), directory: false };

        let supported = prop_supported();
        assert!(supported[4..].chunks_exact(2).any(|bytes| get_u16(bytes, 0) == PROP_ASSOCIATION_TYPE));
        assert_eq!(get_u16(&prop_value(&directory, PROP_ASSOCIATION_TYPE), 0), 1);
        assert_eq!(get_u16(&prop_value(&file, PROP_ASSOCIATION_TYPE), 0), 0);
        assert_eq!(get_u32(&prop_value(&file, PROP_ASSOCIATION_DESC), 0), 0);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn standard_object_file_name_property_is_advertised_and_returned() {
        let root = test_root();
        let path = root.join("inside.txt");
        fs::write(&path, b"inside").unwrap();
        let object = Object { handle: 7, parent: 3, path, directory: false };

        let supported = prop_supported();
        let count = get_u32(&supported, 0) as usize;
        let advertised: Vec<u16> = supported[4..].chunks_exact(2)
            .map(|bytes| get_u16(bytes, 0)).collect();
        assert_eq!(advertised.len(), count);
        assert!(advertised.contains(&PROP_FILE_NAME));
        assert_eq!(get_u16(&object_prop_desc(PROP_FILE_NAME).unwrap(), 2), 0xffff);

        let mut expected = Vec::new();
        push_string(&mut expected, "inside.txt");
        assert_eq!(prop_value(&object, PROP_FILE_NAME), expected);
        assert_eq!(prop_value(&object, PROP_NAME), expected);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_uid_is_path_stable_and_handle_independent() {
        let root = test_root();
        let path = root.join("Folder").join("inside.txt");
        let first = Object { handle: 1, parent: 2, path: path.clone(), directory: false };
        let second = Object { handle: 99, parent: 77, path, directory: false };
        assert_eq!(prop_value(&first, PROP_PERSISTENT_UID), prop_value(&second, PROP_PERSISTENT_UID));
        assert_ne!(persistent_uid(&root.join("Folder")), persistent_uid(&root.join("Other")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_uid_is_serialized_as_little_endian_uint128() {
        let root = test_root();
        let object = Object {
            handle: 1,
            parent: 0,
            path: root.join("item.bin"),
            directory: false,
        };
        let (high, low) = persistent_uid(&object.path);
        let mut expected = Vec::new();
        push_u64(&mut expected, low);
        push_u64(&mut expected, high);
        assert_eq!(prop_value(&object, PROP_PERSISTENT_UID), expected);
        fs::remove_dir_all(root).unwrap();
    }
}
