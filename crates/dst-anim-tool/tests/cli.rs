use std::path::PathBuf;
use std::process::Command;

fn tool() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dst-anim-tool"))
}

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/anim")
        .join(name)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dst_anim_tool_cli_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
#[ignore = "requires local DST game data (data/anim symlink)"]
fn list_missing_anim_exits_nonzero_unless_allowed() {
    let build_only = data("pig_build.zip");
    let output = tool()
        .args(["list", "-i"])
        .arg(&build_only)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "list should fail without anim.bin"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("anim.bin"),
        "stderr should mention anim.bin"
    );

    let output = tool()
        .args(["list", "-i"])
        .arg(&build_only)
        .arg("--allow-missing")
        .output()
        .unwrap();
    assert!(output.status.success(), "--allow-missing should succeed");
}

#[test]
#[ignore = "requires local DST game data (data/anim symlink)"]
fn info_build_only_stays_successful() {
    let output = tool()
        .args(["info", "-i"])
        .arg(data("pig_build.zip"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "build-only info must stay exit 0 (downstream census depends on it)"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("pig_build"));
}

#[test]
#[ignore = "requires local DST game data (data/anim symlink)"]
fn decode_missing_tex_exits_nonzero_unless_allowed() {
    let anim_only = data("player_actions_axe.zip");
    let dir = temp_dir("decode");
    let output = tool()
        .args(["decode"])
        .arg(&anim_only)
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!output.status.success(), "decode should fail without .tex");

    let output = tool()
        .args(["decode"])
        .arg(&anim_only)
        .arg(&dir)
        .arg("--allow-missing")
        .output()
        .unwrap();
    assert!(output.status.success(), "--allow-missing should succeed");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "requires local DST game data (data/anim symlink)"]
fn list_json_has_source_file_and_frame_rate() {
    let output = tool()
        .args(["list", "-i"])
        .arg(data("ds_tallbird_basic.zip"))
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let entry = &value[0];
    assert!(entry["source_file"]
        .as_str()
        .unwrap()
        .ends_with("ds_tallbird_basic.zip"));
    let animations = entry["anim"]["banks"][0]["animations"].as_array().unwrap();
    assert!(!animations.is_empty());
    assert!(animations.iter().all(|a| a["frame_rate"].is_number()));
}

#[test]
#[ignore = "requires local DST game data (data/anim symlink)"]
fn render_manifest_reports_timing_and_ink_bounds() {
    let dir = temp_dir("manifest");
    let manifest = dir.join("meta.json");
    let frames_dir = dir.join("frames");
    let output = tool()
        .args(["render", "-i"])
        .arg(data("ds_tallbird_basic.zip"))
        .args([
            "--anim",
            "tallbird/idle_side",
            "--frame-bounds",
            "ink",
            "--manifest",
        ])
        .arg(&manifest)
        .arg("--out")
        .arg(&frames_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(value["frame_rate"], 30.0);
    assert_eq!(value["frames"].as_array().unwrap().len(), 40);
    assert_eq!(value["skipped"].as_array().unwrap().len(), 0);
    let first = &value["frames"][0];
    assert_eq!(first["src_index"], 0);
    assert_eq!(first["file"], "frame_000.png");
    assert!(first["bounds"]["bottom"].is_number());
    assert!(first["ink_bounds"]["bottom"].is_number());
    assert!(
        frames_dir.join("frame_000.png").exists(),
        "manifest frame file should exist"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "requires local DST game data (data/anim symlink)"]
fn strict_missing_symbol_exits_nonzero() {
    let dir = temp_dir("strict");
    let output = tool()
        .args(["render", "-i"])
        .arg(data("player_actions_axe.zip"))
        .arg("--build")
        .arg(data("wilson.zip"))
        .args(["--anim", "wilson/chop_loop_side", "--strict", "--out"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "--strict should fail on unresolved symbols"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing symbol"),
        "stderr should summarize missing symbols"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
