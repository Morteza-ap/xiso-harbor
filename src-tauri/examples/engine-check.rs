//! Developer-only harness. Never included in the .app bundle.
#[path = "../src/engine.rs"]
#[allow(dead_code)]
mod engine;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: engine-check scan IMAGE | extract IMAGE PARENT NAME [cancel]");
        std::process::exit(2);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let result = (|| -> engine::Result<serde_json::Value> {
        let plan = engine::scan(Path::new(&args[2]), cancel.clone())?;
        if args[1] == "scan" {
            return Ok(serde_json::to_value(&plan.info).unwrap());
        }
        let (partial, final_path) = engine::prepare(
            Path::new(&args[3]),
            &args[4],
            plan.info.total_bytes,
            plan.info.total_files,
        )?;
        let mut cancelled = false;
        let result = engine::extract(plan, &partial, &final_path, &cancel, |p| {
            if args.get(5).is_some_and(|s| s == "cancel") && !cancelled && p.bytes_done > 0 {
                cancel.store(true, Ordering::SeqCst);
                cancelled = true;
            }
        });
        match result {
            Ok(result) => Ok(serde_json::to_value(result).unwrap()),
            Err(error) => {
                eprintln!("Partial folder: {}", partial.path.display());
                Err(error)
            }
        }
    })();
    match result {
        Ok(value) => println!("{value}"),
        Err(error) => {
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    }
}
