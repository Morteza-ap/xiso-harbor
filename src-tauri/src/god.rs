//! Single-disc Xbox 360 GOD backend. No drive management or console configuration.
use crate::engine::{self, AppError, ExtractionResult, Partial, Plan, Progress, Result};
use harbor_god_format::{executable::TitleExecutionInfo, god::*};
use sha1::{Digest, Sha1};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Instant, SystemTime},
};

fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(AppError::cancelled())
    } else {
        Ok(())
    }
}
fn error(e: impl ToString) -> AppError {
    AppError::new(
        "god",
        "The GOD package could not be created or verified.",
        "Keep or delete partial output, then try another single-disc Xbox 360 image.",
        e,
    )
}
fn word(bytes: &[u8], at: usize) -> Result<u32> {
    let b = bytes
        .get(at..at.checked_add(4).ok_or_else(|| error("Header overflow"))?)
        .ok_or_else(|| error("Truncated XEX metadata"))?;
    Ok(u32::from_be_bytes(b.try_into().unwrap()))
}
fn metadata(bytes: &[u8]) -> Result<TitleExecutionInfo> {
    if bytes.get(..4) != Some(b"XEX2") {
        return Err(error("Missing XEX2 header"));
    }
    let count = word(bytes, 20)? as usize;
    if count > 4096 || 24 + count * 8 > bytes.len() {
        return Err(error("Invalid XEX field table"));
    }
    for i in 0..count {
        if word(bytes, 24 + i * 8)? == 0x00040006 {
            let pos = word(bytes, 28 + i * 8)? as usize;
            let b = bytes
                .get(
                    pos..pos
                        .checked_add(24)
                        .ok_or_else(|| error("Header overflow"))?,
                )
                .ok_or_else(|| error("Execution metadata outside bounded XEX header"))?;
            let info = TitleExecutionInfo {
                media_id: word(b, 0)?,
                version: word(b, 4)?,
                base_version: word(b, 8)?,
                title_id: word(b, 12)?,
                platform: b[16],
                executable_type: b[17],
                disc_number: b[18],
                disc_count: b[19],
            };
            if info.disc_count != 1 || info.disc_number != 1 {
                return Err(AppError::new(
                    "god",
                    "Multi-disc GOD conversion is not supported yet.",
                    "Use Game folder mode. Install and play discs may need different handling.",
                    "GOD prototype requires disc 1 of 1.",
                ));
            }
            if info.title_id == 0 {
                return Err(error("Missing Title ID"));
            }
            return Ok(info);
        }
    }
    Err(error("No execution metadata in default.xex"))
}
pub struct GodPlan {
    source: File,
    length: u64,
    info: TitleExecutionInfo,
    title: String,
    source_size: u64,
    modified: Option<SystemTime>,
}
impl GodPlan {
    pub fn new(plan: &mut Plan) -> Result<Self> {
        let (source, length, header) = plan.god_input()?;
        let info = metadata(&header)?;
        if length == 0 || length.div_ceil(BLOCK_SIZE) > 0xffffff {
            return Err(error("Unsupported partition size"));
        }
        let title = plan
            .info
            .suggested_name
            .chars()
            .scan(0, |n, c| {
                *n += c.len_utf16();
                (*n <= 100).then_some(c)
            })
            .collect();
        let source_meta = source.metadata().map_err(error)?;
        Ok(Self {
            source_size: source_meta.len(),
            modified: source_meta.modified().ok(),
            source,
            length,
            info,
            title,
        })
    }
    pub fn estimated_bytes(&self) -> u64 {
        self.length
            .saturating_add(self.length / 50)
            .saturating_add(1024 * 1024)
    }
}
// Bound both reads and seeks to the validated game partition; cancellation is
// checked on every read (use Other: Interrupted is retried by read_to_end).
struct Source<'a> {
    file: &'a mut File,
    base: u64,
    length: u64,
    pos: u64,
    cancel: &'a AtomicBool,
}
impl Read for Source<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("Conversion cancelled"));
        }
        let size = out.len().min((self.length - self.pos) as usize);
        let n = self.file.read(&mut out[..size])?;
        if n == 0 && size > 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        self.pos += n as u64;
        Ok(n)
    }
}
impl Seek for Source<'_> {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let pos = match from {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(n) => self.pos as i128 + n as i128,
            SeekFrom::End(n) => self.length as i128 + n as i128,
        };
        if pos < 0 || pos > self.length as i128 {
            return Err(std::io::Error::other("Seek outside game partition"));
        }
        self.file.seek(SeekFrom::Start(self.base + pos as u64))?;
        self.pos = pos as u64;
        Ok(self.pos)
    }
}
fn new_file(path: &Path) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(path)
        .map_err(|e| AppError::io(e, "GOD output"))
}

pub fn convert(
    mut plan: GodPlan,
    partial: &Partial,
    target: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> Result<ExtractionResult> {
    let result = (|| {
        check(cancel)?;
        partial.verify()?;
        let started = Instant::now();
        let root = partial.path.join("Content/0000000000000000");
        let layout = FileLayout::new(&root, &plan.info, ContentType::GamesOnDemand);
        fs::create_dir_all(layout.data_dir_path()).map_err(error)?;
        let parts = plan.length.div_ceil(BLOCK_SIZE * BLOCKS_PER_PART);
        let base = plan.source.stream_position().map_err(error)?;
        let mut p = Progress {
            total_bytes: plan.length * 2,
            total_files: (parts * 2 + 1) as usize,
            ..Default::default()
        };
        let mut sizes = Vec::new();
        let mut hashes = Vec::<[u8; 20]>::new();
        for i in 0..parts {
            check(cancel)?;
            partial.verify()?;
            plan.source.seek(SeekFrom::Start(base)).map_err(error)?;
            let source = Source {
                file: &mut plan.source,
                base,
                length: plan.length,
                pos: 0,
                cancel,
            };
            let mut output = new_file(&layout.part_file_path(i))?;
            write_part(source, i, &mut output).map_err(error)?;
            output.sync_all().map_err(error)?;
            sizes.push(output.metadata().map_err(error)?.len());
            p.bytes_done = ((i + 1) * BLOCK_SIZE * BLOCKS_PER_PART).min(plan.length);
            p.files_done += 1;
            p.current_file = format!("Creating GOD data {}/{}", i + 1, parts);
            p.elapsed_seconds = started.elapsed().as_secs();
            progress(p.clone());
        }
        // Link each master hash table to the next part, then read back all data.
        let mut next = None;
        for i in (0..parts).rev() {
            check(cancel)?;
            partial.verify()?;
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(layout.part_file_path(i))
                .map_err(error)?;
            let mut master = HashList::read(&mut file).map_err(error)?;
            if let Some(hash) = next {
                master.add_hash(&hash);
            }
            file.seek(SeekFrom::Start(0)).map_err(error)?;
            master.write(&mut file).map_err(error)?;
            file.sync_all().map_err(error)?;
            next = Some(master.digest());
            hashes.push(master.digest());
        }
        hashes.reverse();
        for i in 0..parts {
            check(cancel)?;
            partial.verify()?;
            verify_part(
                &layout.part_file_path(i),
                sizes[i as usize],
                &hashes[i as usize],
                hashes.get(i as usize + 1),
                cancel,
            )?;
            p.bytes_done = plan.length + ((i + 1) * BLOCK_SIZE * BLOCKS_PER_PART).min(plan.length);
            p.files_done += 1;
            p.current_file = format!("Verifying GOD data {}/{}", i + 1, parts);
            p.elapsed_seconds = started.elapsed().as_secs();
            progress(p.clone());
        }
        let header = ConHeaderBuilder::new()
            .with_execution_info(&plan.info)
            .with_content_type(ContentType::GamesOnDemand)
            .with_block_counts(plan.length.div_ceil(BLOCK_SIZE) as u32, 0)
            .with_data_parts_info(parts as u32, sizes.iter().sum())
            .with_mht_hash(&hashes[0])
            .with_game_title(&plan.title)
            .finalize();
        let mut file = new_file(&layout.con_header_file_path())?;
        file.write_all(&header).map_err(error)?;
        file.sync_all().map_err(error)?;
        if fs::read(layout.con_header_file_path()).map_err(error)? != header {
            return Err(error("Header readback mismatch"));
        }
        check(cancel)?;
        partial.verify()?;
        let source_meta = plan.source.metadata().map_err(error)?;
        if source_meta.len() != plan.source_size || source_meta.modified().ok() != plan.modified {
            return Err(error("Source image changed during GOD conversion"));
        }
        engine::rename_exclusive(&partial.path, target)?;
        p.files_done += 1;
        p.current_file = "GOD package verified".into();
        progress(p);
        Ok(ExtractionResult {
            path: target.to_string_lossy().into(),
            bytes: sizes.iter().sum::<u64>() + header.len() as u64,
            files: parts as usize + 1,
            has_xex: true,
        })
    })();
    if cancel.load(Ordering::Relaxed) && result.is_err() {
        Err(AppError::cancelled())
    } else {
        result
    }
}
fn verify_part(
    path: &Path,
    size: u64,
    expected: &[u8; 20],
    next: Option<&[u8; 20]>,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut file = File::open(path).map_err(error)?;
    if file.metadata().map_err(error)?.len() != size {
        return Err(error("Part size mismatch"));
    }
    let master = HashList::read(&mut file).map_err(error)?;
    if &master.digest() != expected {
        return Err(error("Master hash mismatch"));
    }
    let mut remaining = size
        .checked_sub(BLOCK_SIZE)
        .ok_or_else(|| error("Short part"))?;
    let mut index = 0;
    let mut buffer = vec![0; SUBPART_SIZE as usize];
    while remaining > 0 {
        check(cancel)?;
        let sub = HashList::read(&mut file).map_err(error)?;
        let digest: [u8; 20] = Sha1::digest(sub.bytes()).into();
        if master.bytes().get(index * 20..index * 20 + 20) != Some(digest.as_slice()) {
            return Err(error("Subtable hash mismatch"));
        }
        remaining = remaining
            .checked_sub(BLOCK_SIZE)
            .ok_or_else(|| error("Short subtable"))?;
        let n = remaining.min(SUBPART_SIZE) as usize;
        file.read_exact(&mut buffer[..n]).map_err(error)?;
        for (j, block) in buffer[..n].chunks(BLOCK_SIZE as usize).enumerate() {
            let digest = Sha1::digest(block);
            if sub.bytes().get(j * 20..j * 20 + 20) != Some(digest.as_slice()) {
                return Err(error("Data hash mismatch"));
            }
        }
        remaining -= n as u64;
        index += 1;
    }
    if let Some(next) = next {
        if master.bytes().get(index * 20..index * 20 + 20) != Some(next.as_slice()) {
            return Err(error("Part chain mismatch"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn fixture(dir: &Path, disc_count: u8, length: usize) -> std::path::PathBuf {
        let path = dir.join("Example.iso");
        let mut data = vec![0; length.max(36 * 2048)];
        let d = &mut data[32 * 2048..33 * 2048];
        d[..20].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
        d[20..24].copy_from_slice(&33u32.to_le_bytes());
        d[24..28].copy_from_slice(&2048u32.to_le_bytes());
        d[2028..].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
        let table = &mut data[33 * 2048..34 * 2048];
        table[4..8].copy_from_slice(&35u32.to_le_bytes());
        table[8..12].copy_from_slice(&128u32.to_le_bytes());
        table[12] = 0x20;
        table[13] = 11;
        table[14..25].copy_from_slice(b"default.xex");
        let xex = &mut data[35 * 2048..35 * 2048 + 128];
        xex[..4].copy_from_slice(b"XEX2");
        xex[20..24].copy_from_slice(&1u32.to_be_bytes());
        xex[24..28].copy_from_slice(&0x40006u32.to_be_bytes());
        xex[28..32].copy_from_slice(&32u32.to_be_bytes());
        xex[32..36].copy_from_slice(&0x12345678u32.to_be_bytes());
        xex[44..48].copy_from_slice(&0x4d530001u32.to_be_bytes());
        xex[50] = 1;
        xex[51] = disc_count;
        fs::write(&path, data).unwrap();
        path
    }
    #[test]
    fn god_roundtrip_checks_metadata_data_and_hashes_across_parts() {
        let tmp = tempfile::tempdir().unwrap();
        let length = (BLOCKS_PER_PART * BLOCK_SIZE + 15000) as usize;
        let source = fixture(tmp.path(), 1, length);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut scan = engine::scan(&source, cancel.clone()).unwrap();
        let plan = GodPlan::new(&mut scan).unwrap();
        let (partial, target) =
            engine::prepare(tmp.path(), "GOD", plan.estimated_bytes(), 4).unwrap();
        let result = convert(plan, &partial, &target, &cancel, |_| {}).unwrap();
        assert_eq!(result.files, 3);
        let dir = target.join("Content/0000000000000000/4D530001/00007000");
        let header = fs::read(dir.join("12345678")).unwrap();
        assert_eq!(word(&header, 0x344).unwrap(), 0x7000);
        assert_eq!(word(&header, 0x360).unwrap(), 0x4d530001);
        let mut original = File::open(source).unwrap();
        // Independent unpack: skip hash blocks, compare every payload byte to ISO.
        for i in 0..2 {
            let mut part = File::open(dir.join(format!("12345678.data/Data{i:04}"))).unwrap();
            part.seek(SeekFrom::Start(BLOCK_SIZE)).unwrap();
            loop {
                let mut table = [0; 4096];
                if part.read(&mut table).unwrap() == 0 {
                    break;
                }
                let mut payload = vec![0; SUBPART_SIZE as usize];
                let n = (part.metadata().unwrap().len() - part.stream_position().unwrap())
                    .min(SUBPART_SIZE) as usize;
                part.read_exact(&mut payload[..n]).unwrap();
                let mut expected = vec![0; n];
                original.read_exact(&mut expected).unwrap();
                assert_eq!(&payload[..n], expected);
            }
        }
        assert_eq!(original.stream_position().unwrap(), length as u64);
        let digest = Sha1::digest(&header[0x344..0xb000]);
        assert_eq!(&header[0x32c..0x340], digest.as_slice());
        assert!(!partial.path.exists());
    }
    #[test]
    fn rejects_multidisc_and_malformed_execution_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let source = fixture(tmp.path(), 2, 0);
        let mut scan = engine::scan(&source, Arc::new(AtomicBool::new(false))).unwrap();
        assert!(GodPlan::new(&mut scan).is_err());
        assert!(metadata(b"XEX2").is_err());
        let mut bad = vec![0; 32];
        bad[..4].copy_from_slice(b"XEX2");
        bad[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(metadata(&bad).is_err());
    }
    #[test]
    fn cancellation_and_collision_preserve_existing_output() {
        let tmp = tempfile::tempdir().unwrap();
        let source = fixture(tmp.path(), 1, 0);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut scan = engine::scan(&source, cancel.clone()).unwrap();
        let plan = GodPlan::new(&mut scan).unwrap();
        let (partial, target) =
            engine::prepare(tmp.path(), "GOD", plan.estimated_bytes(), 3).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"preserve").unwrap();
        assert!(convert(plan, &partial, &target, &cancel, |_| {}).is_err());
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"preserve");
        partial.delete().unwrap();
        let mut scan = engine::scan(&source, cancel.clone()).unwrap();
        let plan = GodPlan::new(&mut scan).unwrap();
        let (partial, target) =
            engine::prepare(tmp.path(), "Cancelled", plan.estimated_bytes(), 3).unwrap();
        let result = convert(plan, &partial, &target, &cancel, |_| {
            cancel.store(true, Ordering::Relaxed)
        });
        assert_eq!(result.unwrap_err().category, "cancelled");
        assert!(!target.exists());
        partial.delete().unwrap();
    }
    #[test]
    fn partition_offsets_are_preserved_without_copying_disc_prefix() {
        for base in [0x18300000u64, 0xfd90000, 0x2080000] {
            let tmp = tempfile::tempdir().unwrap();
            let raw = fixture(tmp.path(), 1, 0);
            let payload = fs::read(&raw).unwrap();
            let source = tmp.path().join("Partition.iso");
            let mut file = File::create(&source).unwrap();
            file.seek(SeekFrom::Start(base)).unwrap();
            file.write_all(&payload).unwrap();
            drop(file);
            let cancel = Arc::new(AtomicBool::new(false));
            let mut scan = engine::scan(&source, cancel.clone()).unwrap();
            let plan = GodPlan::new(&mut scan).unwrap();
            assert_eq!(plan.length, payload.len() as u64);
            let (partial, target) =
                engine::prepare(tmp.path(), "GOD", plan.estimated_bytes(), 2).unwrap();
            convert(plan, &partial, &target, &cancel, |_| {}).unwrap();
            let data = fs::read(
                target.join("Content/0000000000000000/4D530001/00007000/12345678.data/Data0000"),
            )
            .unwrap();
            assert_eq!(&data[8192..], payload);
        }
    }
    #[test]
    fn changed_source_is_not_committed() {
        let tmp = tempfile::tempdir().unwrap();
        let source = fixture(tmp.path(), 1, 0);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut scan = engine::scan(&source, cancel.clone()).unwrap();
        let plan = GodPlan::new(&mut scan).unwrap();
        let (partial, target) =
            engine::prepare(tmp.path(), "GOD", plan.estimated_bytes(), 2).unwrap();
        let mut changed = false;
        let result = convert(plan, &partial, &target, &cancel, |_| {
            if !changed {
                OpenOptions::new()
                    .append(true)
                    .open(&source)
                    .unwrap()
                    .write_all(&[1])
                    .unwrap();
                changed = true;
            }
        });
        assert!(result.is_err());
        assert!(!target.exists());
        partial.delete().unwrap();
    }
    #[test]
    fn installation_content_is_rejected_before_output() {
        let tmp = tempfile::tempdir().unwrap();
        let source = fixture(tmp.path(), 1, 0);
        let mut data = fs::read(&source).unwrap();
        let table = &mut data[33 * 2048..34 * 2048];
        // A second root entry: empty Content directory, alongside default.xex.
        table[2..4].copy_from_slice(&7u16.to_le_bytes());
        table[28 + 12] = 0x10;
        table[28 + 13] = 7;
        table[28 + 14..28 + 21].copy_from_slice(b"Content");
        fs::write(&source, data).unwrap();
        let mut scan = engine::scan(&source, Arc::new(AtomicBool::new(false))).unwrap();
        assert!(GodPlan::new(&mut scan)
            .err()
            .unwrap()
            .message
            .contains("installation"));
    }
    #[test]
    fn verification_detects_payload_corruption() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("part");
        let mut file = new_file(&path).unwrap();
        write_part(std::io::Cursor::new(vec![7; 8192]), 0, &mut file).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let hash = HashList::read(&mut file).unwrap().digest();
        let size = file.metadata().unwrap().len();
        verify_part(&path, size, &hash, None, &AtomicBool::new(false)).unwrap();
        file.seek(SeekFrom::Start(8192)).unwrap();
        file.write_all(&[8]).unwrap();
        assert!(verify_part(&path, size, &hash, None, &AtomicBool::new(false)).is_err());
    }
}
