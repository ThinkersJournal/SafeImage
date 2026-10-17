//! Writes the synthetic frames for the reference harness. Ignored by default; run with
//! `SAFEIMAGE_FRAMES_DIR=<dir> cargo test --test write_frames -- --ignored`.

#[path = "support/frame_list.rs"]
mod frame_list;
mod support;

#[test]
#[ignore = "fixture generator, not a check"]
fn write_frames() {
    let dir = std::env::var("SAFEIMAGE_FRAMES_DIR").expect("set SAFEIMAGE_FRAMES_DIR");
    std::fs::create_dir_all(&dir).unwrap();
    let mut args = String::new();
    for (name, w, h, pattern) in frame_list::FRAMES {
        let bytes = support::make(*w, *h, *pattern);
        std::fs::write(format!("{dir}/{name}.rgb"), bytes).unwrap();
        args.push_str(&format!("{name} {w} {h} "));
    }
    std::fs::write(format!("{dir}/args.txt"), args.trim_end()).unwrap();
}
