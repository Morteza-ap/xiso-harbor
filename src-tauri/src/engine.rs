//! Native streaming extraction. XDVDFS parsing and partition detection are upstream;
//! path validation, bounded I/O, cancellation and transactional output are local.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Instant, SystemTime},
};
use unicode_normalization::UnicodeNormalization;
use xdvdfs::{
    blockdev::{BlockDeviceRead, OffsetWrapper},
    layout::{DirectoryEntryDiskNode, DirectoryEntryNode, DirectoryEntryTable},
};

const MAX_ENTRIES: usize = 200_000;
const MAX_METADATA_READS: usize = 1_000_000;
const BUFFER_SIZE: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub category: String,
    pub message: String,
    pub recovery: String,
    pub detail: String,
}
impl AppError {
    pub fn new(category: &str, message: &str, recovery: &str, detail: impl ToString) -> Self {
        Self {
            category: category.into(),
            message: message.into(),
            recovery: recovery.into(),
            detail: detail.to_string(),
        }
    }
    pub fn invalid(detail: impl ToString) -> Self {
        Self::new(
            "image",
            "This image is unsupported, incomplete, or malformed.",
            "Try another local copy of the ISO. This app cannot repair a damaged image.",
            detail,
        )
    }
    pub fn cancelled() -> Self {
        Self::new(
            "cancelled",
            "Extraction cancelled.",
            "Keep or delete the partial files below before starting another extraction.",
            "Cancellation requested; native worker stopped.",
        )
    }
    pub fn io(e: std::io::Error, phase: &str) -> Self {
        let (category, message, recovery) = match e.raw_os_error() {
            Some(28) => ("space", "The destination ran out of free space.", "Free up space or choose a different drive, then try again."),
            Some(13 | 1) => ("permission", "macOS did not allow access to a file or folder.", "Choose the file and folder again. Check their permissions and macOS Privacy & Security settings."),
            Some(19 | 6 | 2) => ("disconnected", "A file or drive is no longer available.", "Reconnect the drive and choose the source and destination again."),
            _ if phase == "source" => ("source", "The source image could not be read completely.", "Copy the ISO to a reliable local drive and try again. The source image or drive may be damaged."),
            _ => ("write", "The destination could not be written or verified.", "Check the drive connection, free space, and folder permissions. Then try another destination."),
        };
        Self::new(category, message, recovery, format!("{phase}: {e}"))
    }
}
pub type Result<T> = std::result::Result<T, AppError>;
fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(AppError::cancelled())
    } else {
        Ok(())
    }
}

struct Device {
    file: File,
    len: u64,
    reads: usize,
    limited: bool,
    cancel: Arc<AtomicBool>,
}
impl BlockDeviceRead<std::io::Error> for Device {
    fn read(&mut self, offset: u64, buffer: &mut [u8]) -> std::io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Cancelled",
            ));
        }
        self.reads += 1;
        if self.limited && self.reads > MAX_METADATA_READS {
            return Err(std::io::Error::other(
                "Metadata traversal limit reached; image may contain a cycle",
            ));
        }
        if offset
            .checked_add(buffer.len() as u64)
            .is_none_or(|end| end > self.len)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Image region extends beyond source",
            ));
        }
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(buffer)
    }
}
#[derive(Clone)]
struct Entry {
    path: PathBuf,
    directory: bool,
    offset: u64,
    size: u64,
}
pub struct Plan {
    image: OffsetWrapper<Device, std::io::Error>,
    entries: Vec<Entry>,
    pub info: ImageInfo,
    modified: Option<SystemTime>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub total_bytes: u64,
    pub total_files: usize,
    pub has_xex: bool,
    pub suggested_name: String,
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c == '/' || c == '\\' || c == ':' || c.is_control())
        && name.len() <= 240
}
fn safe_component(name: &str) -> Result<()> {
    if valid_name(name) {
        Ok(())
    } else {
        Err(AppError::invalid(format!(
            "Unsafe or unsupported image filename: {name:?}"
        )))
    }
}
fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .nfd()
        .flat_map(char::to_lowercase)
        .collect()
}

// Keep upstream decoding, but validate traversal edges and complete record bounds
// before allocating the full image manifest. Corrupt trees cannot loop or overlap.
fn read_table(
    image: &mut OffsetWrapper<Device, std::io::Error>,
    table: &DirectoryEntryTable,
    remaining: usize,
) -> Result<Vec<DirectoryEntryNode>> {
    let size = table.region.size;
    let base = table.region.sector as u64 * 2048;
    let mut stack = vec![0u32];
    let mut visited = HashSet::new();
    let mut ranges = BTreeMap::<u32, u32>::new();
    let mut nodes = Vec::new();
    while let Some(relative) = stack.pop() {
        if !visited.insert(relative) {
            return Err(AppError::invalid(
                "Directory entry cycle or repeated child pointer.",
            ));
        }
        if relative.checked_add(14).is_none_or(|end| end > size) {
            return Err(AppError::invalid(
                "Directory entry header extends beyond its table.",
            ));
        }
        let mut header = [0u8; 14];
        image
            .read(base + relative as u64, &mut header)
            .map_err(|e| {
                if image.get_ref().cancel.load(Ordering::Relaxed) {
                    AppError::cancelled()
                } else {
                    AppError::io(e, "source")
                }
            })?;
        if header == [0; 14] || header == [0xff; 14] {
            continue;
        }
        if nodes.len() >= remaining {
            return Err(AppError::invalid(
                "Image exceeds the 200,000-entry safety limit.",
            ));
        }
        let node = DirectoryEntryDiskNode::deserialize::<std::io::Error>(&header)
            .map_err(AppError::invalid)?;
        let length = node.dirent.filename_length as usize;
        let end = relative + 14 + length as u32;
        if end > size {
            return Err(AppError::invalid(
                "Directory entry filename extends beyond its table.",
            ));
        }
        if ranges
            .range(..=relative)
            .next_back()
            .is_some_and(|(_, &end)| end > relative)
            || ranges
                .range(relative..)
                .next()
                .is_some_and(|(&start, _)| start < end)
        {
            return Err(AppError::invalid("Directory entry records overlap."));
        }
        ranges.insert(relative, end);
        let mut name = [0u8; 256];
        image
            .read(base + relative as u64 + 14, &mut name[..length])
            .map_err(|e| AppError::io(e, "source"))?;
        for child in [node.right_entry_offset, node.left_entry_offset] {
            if child != 0 && child != 0xffff {
                stack.push(child as u32 * 4);
            }
        }
        nodes.push(DirectoryEntryNode {
            node,
            name,
            offset: base + relative as u64,
        });
    }
    Ok(nodes)
}

pub fn suggested_name(path: &Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let clean: String = stem
        .chars()
        .map(|c| {
            if c == ':' || c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut byte_length = 0;
    let clean: String = clean
        .chars()
        .take_while(|c| {
            byte_length += c.len_utf8();
            byte_length <= 200
        })
        .collect();
    if clean.trim().is_empty() || clean == "." || clean == ".." {
        "Extracted Game".into()
    } else {
        clean
    }
}
pub fn scan(path: &Path, cancel: Arc<AtomicBool>) -> Result<Plan> {
    check_cancel(&cancel)?;
    if !path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("iso") || s.eq_ignore_ascii_case("xiso"))
    {
        return Err(AppError::invalid("Choose an .iso or .xiso file."));
    }
    let canonical = path.canonicalize().map_err(|e| AppError::io(e, "source"))?;
    // Opening a FIFO normally blocks before metadata can reject it. NONBLOCK is
    // ignored for regular files; NOFOLLOW also closes a final-component swap.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&canonical)
        .map_err(|e| AppError::io(e, "source"))?;
    let meta = file.metadata().map_err(|e| AppError::io(e, "source"))?;
    if !meta.is_file() {
        return Err(AppError::invalid("Source is not a regular file."));
    }
    let mut image = OffsetWrapper::new(Device {
        file,
        len: meta.len(),
        reads: 0,
        limited: true,
        cancel: cancel.clone(),
    })
    .map_err(|e| {
        if cancel.load(Ordering::Relaxed) {
            AppError::cancelled()
        } else {
            AppError::invalid(e)
        }
    })?;
    let volume = xdvdfs::read::read_volume(&mut image).map_err(AppError::invalid)?;
    let mut queue = vec![(PathBuf::new(), volume.root_table, 0usize)];
    let mut tables = HashSet::new();
    let mut paths = HashSet::new();
    let mut entries = Vec::new();
    let mut total_bytes = 0u64;
    let mut total_files = 0usize;
    let mut has_xex = false;
    while let Some((parent, table, depth)) = queue.pop() {
        check_cancel(&cancel)?;
        if depth > 64 {
            return Err(AppError::invalid("Directory nesting exceeds 64 levels."));
        }
        if table.is_empty() {
            continue;
        }
        let sector = table.region.sector;
        if !tables.insert(sector) {
            return Err(AppError::invalid(
                "Repeated directory table or directory cycle.",
            ));
        }
        let nodes = read_table(&mut image, &table, MAX_ENTRIES - entries.len())?;
        for node in nodes {
            check_cancel(&cancel)?;
            let name = node
                .name_str::<std::io::Error>()
                .map_err(AppError::invalid)?
                .into_owned();
            safe_component(&name)?;
            let path = parent.join(&name);
            if path.as_os_str().len() > 900 || !paths.insert(path_key(&path)) {
                return Err(AppError::invalid(
                    "Duplicate, case-colliding, or overly long output path.",
                ));
            }
            let dirent = node.node.dirent;
            let directory = dirent.is_directory();
            let size = dirent.data.size as u64;
            let offset = dirent.data.sector as u64 * 2048;
            // Probe the end of each region through the upstream partition wrapper.
            if size > 0 {
                let mut probe = [0];
                image
                    .read(offset + size - 1, &mut probe)
                    .map_err(|e| AppError::io(e, "source"))?;
            }
            if directory {
                queue.push((
                    path.clone(),
                    DirectoryEntryTable {
                        region: dirent.data,
                    },
                    depth + 1,
                ));
            } else {
                total_bytes = total_bytes
                    .checked_add(size)
                    .ok_or_else(|| AppError::invalid("Output size overflow"))?;
                total_files += 1;
                has_xex |= name.eq_ignore_ascii_case("default.xex");
            }
            entries.push(Entry {
                path,
                directory,
                offset,
                size,
            });
            if entries.len() > MAX_ENTRIES {
                return Err(AppError::invalid(
                    "Image exceeds the 200,000-entry safety limit.",
                ));
            }
        }
    }
    if total_files == 0 {
        return Err(AppError::invalid("No files were found in the image."));
    }
    image.get_mut().limited = false;
    let info = ImageInfo {
        path: canonical.to_string_lossy().into(),
        name: canonical
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        size: meta.len(),
        total_bytes,
        total_files,
        has_xex,
        suggested_name: suggested_name(&canonical),
    };
    Ok(Plan {
        image,
        entries,
        info,
        modified: meta.modified().ok(),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Partial {
    pub path: PathBuf,
    pub dev: u64,
    pub ino: u64,
}
impl Partial {
    pub fn verify(&self) -> Result<()> {
        let managed_name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| {
                n.strip_prefix("Xiso Harbor ")
                    .or_else(|| n.strip_prefix("Xbox Extraction "))
            })
            .and_then(|n| n.strip_suffix(".partial"))
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok());
        if !self.path.is_absolute()
            || self
                .path
                .components()
                .any(|p| matches!(p, Component::ParentDir))
            || !managed_name
        {
            return Err(AppError::new(
                "safety",
                "This is not a recognized app-managed partial folder.",
                "Inspect the folder in Finder. Automatic deletion has been refused.",
                "Partial path is not an absolute app-managed UUID directory.",
            ));
        }
        let meta =
            fs::symlink_metadata(&self.path).map_err(|e| AppError::io(e, "partial output"))?;
        if !meta.is_dir()
            || meta.dev() != self.dev
            || meta.ino() != self.ino
            || !self
                .path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".partial"))
        {
            return Err(AppError::new(
                "safety",
                "The partial folder has changed since extraction.",
                "Inspect this folder in Finder. Automatic deletion has been refused.",
                "Partial directory identity does not match recorded device and inode.",
            ));
        }
        Ok(())
    }
    pub fn delete(&self) -> Result<()> {
        self.verify()?;
        fs::remove_dir_all(&self.path).map_err(|e| AppError::io(e, "partial cleanup"))
    }
}
pub fn unique_name(parent: &Path, desired: &str) -> String {
    let desired = if valid_name(desired) && !desired.trim().is_empty() {
        desired
    } else {
        "Extracted Game"
    };
    for index in 0..10_000 {
        let name = if index == 0 {
            desired.to_owned()
        } else {
            format!("{desired} ({})", index + 1)
        };
        if fs::symlink_metadata(parent.join(&name))
            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            return name;
        }
    }
    format!("{desired} {}", uuid::Uuid::new_v4())
}
pub fn free_space(path: &Path) -> Result<u64> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(AppError::invalid)?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return Err(AppError::io(std::io::Error::last_os_error(), "destination"));
    }
    let stat = unsafe { stat.assume_init() };
    Ok((stat.f_bavail as u64).saturating_mul(stat.f_frsize))
}
pub fn prepare(
    parent: &Path,
    name: &str,
    bytes: u64,
    entries: usize,
) -> Result<(Partial, PathBuf)> {
    if !valid_name(name) || name.trim().is_empty() {
        return Err(AppError::new(
            "name",
            "Choose a simple, nonempty subfolder name.",
            "Use a name without slashes, colons, or control characters.",
            "Invalid destination subfolder name.",
        ));
    }
    let parent = parent
        .canonicalize()
        .map_err(|e| AppError::io(e, "destination"))?;
    if !parent.is_dir() {
        return Err(AppError::invalid("Destination must be a folder."));
    }
    let final_path = parent.join(name);
    if fs::symlink_metadata(&final_path).is_ok() {
        return Err(AppError::new("collision", "A folder with this name already exists.", "Edit the subfolder name or choose another output folder. Existing files will be preserved.", "Destination already exists."));
    }
    // Allow per-entry filesystem allocation overhead and a modest free-space reserve.
    let needed = bytes
        .saturating_add(entries as u64 * 16384)
        .saturating_add(64 * 1024 * 1024);
    if free_space(&parent)? < needed {
        return Err(AppError::new(
            "space",
            "There is not enough free space at this destination.",
            "Free up space or choose a different drive.",
            format!("Need approximately {needed} bytes including allocation overhead and reserve."),
        ));
    }
    let path = parent.join(format!("Xiso Harbor {}.partial", uuid::Uuid::new_v4()));
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .map_err(|e| AppError::io(e, "destination"))?;
    let meta = fs::symlink_metadata(&path).map_err(|e| AppError::io(e, "destination"))?;
    Ok((
        Partial {
            path,
            dev: meta.dev(),
            ino: meta.ino(),
        },
        final_path,
    ))
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub files_done: usize,
    pub total_files: usize,
    pub current_file: String,
    pub elapsed_seconds: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionResult {
    pub path: String,
    pub bytes: u64,
    pub files: usize,
    pub has_xex: bool,
}

fn create_directories(root: &Path, relative: &Path) -> Result<()> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(AppError::invalid("Unsafe output directory component."));
        };
        path.push(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => (),
            Ok(_) => return Err(AppError::new("safety", "An output directory was replaced by another file or link.", "Leave the partial files unchanged while extraction is running, then try again in a new folder.", "Output ancestor is not a real directory.")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path).map_err(|e| AppError::io(e, "destination"))?,
            Err(e) => return Err(AppError::io(e, "destination")),
        }
    }
    Ok(())
}

pub fn extract(
    mut plan: Plan,
    partial: &Partial,
    final_path: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> Result<ExtractionResult> {
    partial.verify()?;
    let started = Instant::now();
    let mut last_emit = Instant::now();
    let mut p = Progress {
        total_bytes: plan.info.total_bytes,
        total_files: plan.info.total_files,
        ..Default::default()
    };
    let mut buffer = vec![0u8; BUFFER_SIZE];
    for entry in &plan.entries {
        check_cancel(cancel)?;
        partial.verify()?;
        let target = partial.path.join(&entry.path);
        p.current_file = entry.path.to_string_lossy().into();
        p.elapsed_seconds = started.elapsed().as_secs();
        if entry.directory {
            create_directories(&partial.path, &entry.path)?;
            continue;
        }
        if let Some(parent) = entry.path.parent() {
            create_directories(&partial.path, parent)?;
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&target)
            .map_err(|e| AppError::io(e, "destination"))?;
        let mut copied = 0;
        while copied < entry.size {
            check_cancel(cancel)?;
            let length = (entry.size - copied).min(BUFFER_SIZE as u64) as usize;
            plan.image
                .read(entry.offset + copied, &mut buffer[..length])
                .map_err(|e| {
                    if cancel.load(Ordering::Relaxed) {
                        AppError::cancelled()
                    } else {
                        AppError::io(e, "source")
                    }
                })?;
            output
                .write_all(&buffer[..length])
                .map_err(|e| AppError::io(e, "destination"))?;
            copied += length as u64;
            p.bytes_done += length as u64;
            if last_emit.elapsed().as_millis() >= 100 {
                p.elapsed_seconds = started.elapsed().as_secs();
                progress(p.clone());
                last_emit = Instant::now();
            }
        }
        output
            .sync_all()
            .map_err(|e| AppError::io(e, "destination sync"))?;
        if output
            .metadata()
            .map_err(|e| AppError::io(e, "verification"))?
            .len()
            != entry.size
        {
            return Err(AppError::invalid(
                "Output size did not match source metadata.",
            ));
        }
        p.files_done += 1;
        if last_emit.elapsed().as_millis() >= 100 {
            progress(p.clone());
            last_emit = Instant::now();
        }
    }
    check_cancel(cancel)?;
    let meta = plan
        .image
        .get_ref()
        .file
        .metadata()
        .map_err(|e| AppError::io(e, "source"))?;
    if meta.len() != plan.info.size || meta.modified().ok() != plan.modified {
        return Err(AppError::new(
            "source",
            "The source image changed during extraction.",
            "Wait until any download or copy finishes, then try again.",
            "Source size or modification time changed.",
        ));
    }
    // Check every output using symlink_metadata before committing the directory.
    for entry in &plan.entries {
        let meta = fs::symlink_metadata(partial.path.join(&entry.path))
            .map_err(|e| AppError::io(e, "verification"))?;
        if (entry.directory && !meta.is_dir())
            || (!entry.directory && (!meta.is_file() || meta.len() != entry.size))
        {
            return Err(AppError::invalid("Output verification failed."));
        }
    }
    partial.verify()?;
    rename_exclusive(&partial.path, final_path)?;
    p.elapsed_seconds = started.elapsed().as_secs();
    progress(p);
    Ok(ExtractionResult {
        path: final_path.to_string_lossy().into(),
        bytes: plan.info.total_bytes,
        files: plan.info.total_files,
        has_xex: plan.info.has_xex,
    })
}
fn rename_exclusive(from: &Path, to: &Path) -> Result<()> {
    let from = CString::new(from.as_os_str().as_bytes()).map_err(AppError::invalid)?;
    let to = CString::new(to.as_os_str().as_bytes()).map_err(AppError::invalid)?;
    #[cfg(target_os = "macos")]
    let status = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    #[cfg(not(target_os = "macos"))]
    compile_error!(
        "This release targets macOS; exclusive rename must be implemented for other platforms."
    );
    if status != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(AppError::new("collision", "The final folder was created by another process.", "Keep the completed partial folder and choose another name for your next extraction.", error));
        }
        return Err(AppError::io(error, "commit output"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "engine/tests.rs"]
mod tests;
