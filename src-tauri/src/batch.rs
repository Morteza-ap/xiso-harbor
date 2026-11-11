//! One synchronous worker owns the entire queue; an error leaves later items untouched.
use crate::engine::{AppError, Result};
use std::sync::atomic::{AtomicBool, Ordering};

pub fn run(
    pending: impl IntoIterator<Item = usize>,
    cancel: &AtomicBool,
    mut extract: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    for index in pending {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::new(
                "cancelled",
                "Extraction queue stopped.",
                "Completed folders are preserved. Continue to retry unfinished images.",
                "Queue cancelled before the next image started.",
            ));
        }
        extract(index)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_in_order_and_skips_already_completed_items() {
        let cancel = AtomicBool::new(false);
        let mut visited = vec![];
        run([1, 3, 4], &cancel, |index| {
            visited.push(index);
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, [1, 3, 4]);
    }

    #[test]
    fn failure_stops_before_next_image() {
        let cancel = AtomicBool::new(false);
        let mut visited = vec![];
        let error = run(0..3, &cancel, |index| {
            visited.push(index);
            if index == 1 {
                return Err(AppError::invalid("Broken image"));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(visited, [0, 1]);
        assert_eq!(error.category, "image");
    }

    #[test]
    fn cancellation_between_images_stops_queue_and_allows_resume() {
        let cancel = AtomicBool::new(false);
        let mut visited = vec![];
        assert_eq!(
            run(0..3, &cancel, |index| {
                visited.push(index);
                cancel.store(true, Ordering::SeqCst);
                Ok(())
            })
            .unwrap_err()
            .category,
            "cancelled"
        );
        assert_eq!(visited, [0]);
        cancel.store(false, Ordering::SeqCst);
        run(1..3, &cancel, |index| {
            visited.push(index);
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, [0, 1, 2]);
    }
    #[test]
    fn extracts_same_named_discs_into_distinct_folders_without_overwrite() {
        use crate::engine;
        use std::{fs, sync::Arc};
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("output");
        fs::create_dir(&output).unwrap();
        fs::create_dir(output.join("Game")).unwrap();
        fs::write(output.join("Game/existing.txt"), b"preserve").unwrap();
        let mut sources = vec![];
        for (index, payload) in [b"disc one", b"disc two"].iter().enumerate() {
            let folder = tmp.path().join(index.to_string());
            fs::create_dir(&folder).unwrap();
            let source = folder.join("Game.iso");
            let mut data = vec![0u8; 35 * 2048 + payload.len()];
            let descriptor = &mut data[32 * 2048..33 * 2048];
            descriptor[..20].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
            descriptor[20..24].copy_from_slice(&33u32.to_le_bytes());
            descriptor[24..28].copy_from_slice(&2048u32.to_le_bytes());
            descriptor[2028..].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
            let table = &mut data[33 * 2048..34 * 2048];
            table[4..8].copy_from_slice(&35u32.to_le_bytes());
            table[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
            table[12] = 0x20;
            table[13] = 11;
            table[14..25].copy_from_slice(b"default.xex");
            data[35 * 2048..].copy_from_slice(*payload);
            fs::write(&source, data).unwrap();
            sources.push(source);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let mut outputs = vec![];
        run(0..sources.len(), &cancel, |index| {
            // The previous disc must already be committed before the next scan.
            for path in &outputs {
                assert!(std::path::Path::new(path).is_dir());
            }
            let plan = engine::scan(&sources[index], cancel.clone())?;
            let name = engine::unique_name(&output, &plan.info.suggested_name);
            let (partial, target) =
                engine::prepare(&output, &name, plan.info.total_bytes, plan.info.total_files)?;
            let result = engine::extract(plan, &partial, &target, &cancel, |_| {})?;
            assert!(!partial.path.exists());
            outputs.push(result.path);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fs::read(output.join("Game/existing.txt")).unwrap(),
            b"preserve"
        );
        assert_eq!(
            fs::read(output.join("Game (2)/default.xex")).unwrap(),
            b"disc one"
        );
        assert_eq!(
            fs::read(output.join("Game (3)/default.xex")).unwrap(),
            b"disc two"
        );
        assert_ne!(outputs[0], outputs[1]);
    }
}
