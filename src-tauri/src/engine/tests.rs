use super::*;

// Independent minimal XISO fixture: one root entry and a small inert payload.
fn fixture(path: &Path, filename: &str, payload_size: usize) {
    let mut data = vec![0u8; 35 * 2048 + payload_size];
    let descriptor = &mut data[32 * 2048..33 * 2048];
    descriptor[..20].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
    descriptor[20..24].copy_from_slice(&33u32.to_le_bytes());
    descriptor[24..28].copy_from_slice(&2048u32.to_le_bytes());
    descriptor[2028..].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
    let table = &mut data[33 * 2048..34 * 2048];
    table[4..8].copy_from_slice(&35u32.to_le_bytes());
    table[8..12].copy_from_slice(&(payload_size as u32).to_le_bytes());
    table[12] = 0x20;
    table[13] = filename.len() as u8;
    table[14..14 + filename.len()].copy_from_slice(filename.as_bytes());
    for (index, value) in data[35 * 2048..].iter_mut().enumerate() {
        *value = (index % 251) as u8;
    }
    fs::write(path, data).unwrap();
}
fn token() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[test]
fn native_roundtrip_and_case_insensitive_xex() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("بازی 'space'.ISO");
    fixture(&iso, "DEFAULT.XEX", 4097);
    let plan = scan(&iso, token()).unwrap();
    assert!(plan.info.has_xex);
    let (partial, final_path) = prepare(
        tmp.path(),
        "Game Disc 1",
        plan.info.total_bytes,
        plan.info.total_files,
    )
    .unwrap();
    let mut progress = Progress::default();
    let result = extract(plan, &partial, &final_path, &AtomicBool::new(false), |p| {
        progress = p
    })
    .unwrap();
    assert_eq!(result.files, 1);
    assert_eq!(result.bytes, 4097);
    assert!(result.has_xex);
    assert_eq!(
        fs::read(final_path.join("DEFAULT.XEX")).unwrap(),
        (0..4097).map(|i| (i % 251) as u8).collect::<Vec<_>>()
    );
    assert_eq!(progress.bytes_done, 4097);
    assert_eq!(progress.files_done, 1);
    assert!(!partial.path.exists());
}
#[test]
fn rejects_traversal_and_unsafe_names() {
    for name in ["..", ".", "/outside", "a/b", "a\\b", "a:b", "\0", ""] {
        assert!(!valid_name(name));
        let tmp = tempfile::tempdir().unwrap();
        let iso = tmp.path().join("bad.iso");
        fixture(&iso, name, 1);
        assert!(scan(&iso, token()).is_err(), "accepted {name:?}");
    }
}
#[test]
fn rejects_truncated_and_invalid_images() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("broken.iso");
    fixture(&iso, "default.xex", 100);
    File::options()
        .write(true)
        .open(&iso)
        .unwrap()
        .set_len(35 * 2048)
        .unwrap();
    assert!(scan(&iso, token()).is_err());
    fs::write(&iso, b"not an xbox image").unwrap();
    assert!(scan(&iso, token()).is_err());
}
#[test]
fn cancellation_leaves_only_managed_partial() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("game.iso");
    fixture(&iso, "default.xex", 500);
    let cancel = token();
    let plan = scan(&iso, cancel.clone()).unwrap();
    let (partial, target) = prepare(tmp.path(), "Game", 500, 1).unwrap();
    cancel.store(true, Ordering::SeqCst);
    assert_eq!(
        extract(plan, &partial, &target, &cancel, |_| {})
            .unwrap_err()
            .category,
        "cancelled"
    );
    assert!(!target.exists());
    assert!(partial.path.exists());
    fs::write(tmp.path().join("unrelated.txt"), "preserve me").unwrap();
    partial.delete().unwrap();
    assert_eq!(
        fs::read_to_string(tmp.path().join("unrelated.txt")).unwrap(),
        "preserve me"
    );
}
#[test]
fn never_overwrites_existing_destination_even_at_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("Game");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("existing"), "original").unwrap();
    assert_eq!(
        prepare(tmp.path(), "Game", 1, 1).unwrap_err().category,
        "collision"
    );
    let iso = tmp.path().join("game.iso");
    fixture(&iso, "default.xex", 10);
    let plan = scan(&iso, token()).unwrap();
    let (partial, target2) = prepare(tmp.path(), "Game 2", 10, 1).unwrap();
    fs::create_dir(&target2).unwrap();
    assert_eq!(
        extract(plan, &partial, &target2, &AtomicBool::new(false), |_| {})
            .unwrap_err()
            .category,
        "collision"
    );
    assert!(partial.path.join("default.xex").exists());
    assert_eq!(
        fs::read_to_string(target.join("existing")).unwrap(),
        "original"
    );
    partial.delete().unwrap();
}
#[test]
fn insufficient_space_preflight_creates_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        prepare(tmp.path(), "Game", u64::MAX, 1)
            .unwrap_err()
            .category,
        "space"
    );
    assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
}
#[test]
fn refuses_cleanup_when_partial_is_replaced_by_symlink() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("original"), "safe").unwrap();
    let (partial, _) = prepare(tmp.path(), "Game", 0, 0).unwrap();
    fs::remove_dir(&partial.path).unwrap();
    std::os::unix::fs::symlink(&outside, &partial.path).unwrap();
    assert_eq!(partial.delete().unwrap_err().category, "safety");
    assert_eq!(
        fs::read_to_string(outside.join("original")).unwrap(),
        "safe"
    );
}
#[test]
fn retains_disc_names_and_finds_unique_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("Example Disc 2")).unwrap();
    assert_eq!(
        suggested_name(Path::new("/a/Example Disc 2.iso")),
        "Example Disc 2"
    );
    assert_eq!(
        unique_name(tmp.path(), "Example Disc 2"),
        "Example Disc 2 (2)"
    );
}

#[test]
fn unicode_suggestions_fit_byte_limits_and_cannot_probe_parent_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let suggested = suggested_name(&PathBuf::from(format!("{}.iso", "🎮".repeat(150))));
    assert!(valid_name(&suggested));
    assert!(suggested.len() <= 200);
    assert_eq!(unique_name(tmp.path(), "../outside"), "Extracted Game");
}
#[test]
fn corrupted_directory_cycles_are_bounded() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("cycle.iso");
    fixture(&iso, "loop", 2048);
    let mut data = fs::read(&iso).unwrap();
    let start = 33 * 2048;
    data[start + 4..start + 8].copy_from_slice(&33u32.to_le_bytes());
    data[start + 12] = 0x10;
    fs::write(&iso, data).unwrap();
    assert!(scan(&iso, token()).is_err());
}

#[test]
fn rejects_case_collisions_before_creating_output() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("collision.iso");
    fixture(&iso, "DEFAULT.XEX", 10);
    let mut data = fs::read(&iso).unwrap();
    let table = 33 * 2048;
    data[table + 2..table + 4].copy_from_slice(&10u16.to_le_bytes());
    let other = table + 40;
    data[other + 4..other + 8].copy_from_slice(&35u32.to_le_bytes());
    data[other + 8..other + 12].copy_from_slice(&10u32.to_le_bytes());
    data[other + 12] = 0x20;
    data[other + 13] = 11;
    data[other + 14..other + 25].copy_from_slice(b"default.xex");
    fs::write(&iso, data).unwrap();
    assert!(scan(&iso, token()).is_err());
    assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
}

#[test]
fn refuses_to_commit_when_the_source_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("changing.iso");
    fixture(&iso, "DEFAULT.XEX", 10);
    let plan = scan(&iso, token()).unwrap();
    let (partial, target) = prepare(tmp.path(), "Game", 10, 1).unwrap();
    let file = OpenOptions::new().write(true).open(&iso).unwrap();
    file.set_len(file.metadata().unwrap().len() + 1).unwrap();
    let error = extract(plan, &partial, &target, &AtomicBool::new(false), |_| {}).unwrap_err();
    assert_eq!(error.category, "source");
    assert!(!target.exists());
    assert!(partial.path.exists());
}

#[test]
fn rejects_a_named_pipe_without_waiting_for_a_writer() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("pipe.iso");
    let cpath = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    assert!(scan(&path, token()).is_err());
}

#[test]
fn directory_record_must_fit_inside_its_table() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("bad-table.iso");
    fixture(&iso, "DEFAULT.XEX", 10);
    let mut data = fs::read(&iso).unwrap();
    data[32 * 2048 + 24..32 * 2048 + 28].copy_from_slice(&14u32.to_le_bytes());
    fs::write(&iso, data).unwrap();
    assert!(scan(&iso, token()).is_err());
}

#[test]
fn cyclic_child_pointers_are_rejected_before_large_allocations() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("cycle.iso");
    fixture(&iso, "DEFAULT.XEX", 10);
    let mut data = fs::read(&iso).unwrap();
    let table = 33 * 2048;
    data[table + 2..table + 4].copy_from_slice(&10u16.to_le_bytes());
    let other = table + 40;
    let original_node = data[table..table + 25].to_vec();
    data[other..other + 25].copy_from_slice(&original_node);
    data[other + 14..other + 25].copy_from_slice(b"another.dat");
    fs::write(&iso, data).unwrap();
    let error = scan(&iso, token()).err().unwrap();
    assert!(error.detail.contains("cycle") || error.detail.contains("repeated"));
}

#[test]
fn symlink_ancestors_cannot_redirect_output_and_cleanup_preserves_the_target() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("game.iso");
    fixture(&iso, "DEFAULT.XEX", 10);
    let mut plan = scan(&iso, token()).unwrap();
    plan.entries[0].path = PathBuf::from("nested/DEFAULT.XEX");
    let (partial, final_path) = prepare(tmp.path(), "Game", 10, 1).unwrap();
    let outside = tmp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("existing"), b"preserved").unwrap();
    std::os::unix::fs::symlink(&outside, partial.path.join("nested")).unwrap();
    let error = extract(plan, &partial, &final_path, &AtomicBool::new(false), |_| {}).unwrap_err();
    assert_eq!(error.category, "safety");
    assert!(!outside.join("DEFAULT.XEX").exists());
    partial.delete().unwrap();
    assert_eq!(fs::read(outside.join("existing")).unwrap(), b"preserved");
}

#[test]
fn cleanup_refuses_an_unrelated_partial_directory_even_with_matching_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("unrelated.partial");
    fs::create_dir(&path).unwrap();
    let meta = fs::metadata(&path).unwrap();
    fs::write(path.join("keep"), b"original").unwrap();
    let partial = Partial {
        path: path.clone(),
        dev: meta.dev(),
        ino: meta.ino(),
    };
    assert!(partial.delete().is_err());
    assert!(path.join("keep").exists());
}

#[test]
fn preexisting_partial_files_are_never_overwritten() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("game.iso");
    fixture(&iso, "DEFAULT.XEX", 10);
    let plan = scan(&iso, token()).unwrap();
    let (partial, target) = prepare(tmp.path(), "Game", 10, 1).unwrap();
    fs::write(partial.path.join("DEFAULT.XEX"), b"preserve me").unwrap();
    assert!(extract(plan, &partial, &target, &AtomicBool::new(false), |_| {}).is_err());
    assert_eq!(
        fs::read(partial.path.join("DEFAULT.XEX")).unwrap(),
        b"preserve me"
    );
    assert!(!target.exists());
}

#[test]
fn deterministic_metadata_mutations_do_not_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let iso = tmp.path().join("mutated.iso");
    fixture(&iso, "DEFAULT.XEX", 10);
    let original = fs::read(&iso).unwrap();
    let mut seed = 0x13579bdfu32;
    for _ in 0..256 {
        let mut data = original.clone();
        for _ in 0..8 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let offset = 32 * 2048 + (seed as usize % (2 * 2048));
            data[offset] ^= (seed >> 24) as u8;
        }
        fs::write(&iso, data).unwrap();
        assert!(std::panic::catch_unwind(|| scan(&iso, token())).is_ok());
    }
}
