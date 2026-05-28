use std::collections::HashSet;
use std::path::{Path, PathBuf};

use clap::Subcommand;

#[derive(clap::Parser)]
#[command(name = "dst-anim-tool", about = "DST animation file extraction tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Split {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
        #[arg(short, long)]
        skin: Option<PathBuf>,
        output_dir: PathBuf,
    },
    Render {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
        #[arg(short, long)]
        skin: Option<PathBuf>,
        anim_path: String,
        output_dir: PathBuf,
    },
    List {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
    },
    Info {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
    },
    Decrypt {
        input: PathBuf,
        output: PathBuf,
    },
    Decode {
        input: PathBuf,
        output_dir: PathBuf,
    },
    Preview {
        #[arg(short, long)]
        input: Option<Vec<PathBuf>>,
    },
}

pub fn run(cli: Cli) -> dst_anim_tool::error::Result<()> {
    match cli.command {
        None => cmd_preview(None),
        Some(command) => match command {
            Commands::Split {
                input,
                skin,
                output_dir,
            } => cmd_split(&input, skin.as_deref(), &output_dir),
            Commands::Render {
                input,
                skin,
                anim_path,
                output_dir,
            } => cmd_render(&input, skin.as_deref(), &anim_path, &output_dir),
            Commands::List { input } => cmd_list(&input),
            Commands::Info { input } => cmd_info(&input),
            Commands::Decrypt { input, output } => cmd_decrypt(&input, &output),
            Commands::Decode { input, output_dir } => cmd_decode(&input, &output_dir),
            Commands::Preview { input } => cmd_preview(input),
        },
    }
}

fn load_skin_archive(
    path: &Path,
) -> dst_anim_tool::error::Result<dst_anim_tool::archive::ParsedArchive> {
    let data = std::fs::read(path)?;
    let mut archive = dst_anim_tool::archive::parse_file_by_path(path, &data)?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "dyn" => {
            let companion = path.with_extension("zip");
            if companion.exists() {
                let companion_data = std::fs::read(&companion)?;
                let companion_archive =
                    dst_anim_tool::archive::parse_file_by_path(&companion, &companion_data)?;
                archive.merge(companion_archive);
            } else {
                return Err(dst_anim_tool::error::Error::MissingCompanion(
                    "no companion .zip found for .dyn skin file".to_string(),
                ));
            }
        }
        "zip" => {
            if archive.tex_sources.iter().all(|s| s.tex_files.is_empty()) {
                let companion = path.with_extension("dyn");
                if companion.exists() {
                    let companion_data = std::fs::read(&companion)?;
                    let companion_archive =
                        dst_anim_tool::archive::parse_file_by_path(&companion, &companion_data)?;
                    archive.merge(companion_archive);
                } else {
                    return Err(dst_anim_tool::error::Error::MissingCompanion(
                        "no companion .dyn found for build-only .zip skin file".to_string(),
                    ));
                }
            }
        }
        _ => {
            return Err(dst_anim_tool::error::Error::Other(
                "skin file must be .dyn or .zip".to_string(),
            ));
        }
    }

    if archive.build.is_none() {
        return Err(dst_anim_tool::error::Error::MissingData(
            "build.bin in skin".to_string(),
        ));
    }
    if archive.tex_sources.iter().all(|s| s.tex_files.is_empty()) {
        return Err(dst_anim_tool::error::Error::MissingData(
            "atlas textures in skin".to_string(),
        ));
    }

    Ok(archive)
}

fn write_symbol_frames(
    symbol: &dst_anim_tool::build_file::BuildSymbol,
    output_dir: &Path,
) -> dst_anim_tool::error::Result<()> {
    let has_images = symbol.frames.iter().any(|f| f.image.is_some());
    if !has_images {
        return Ok(());
    }
    let safe_name = symbol.name.replace(['/', '\\', ':'], "_");
    let sym_dir = output_dir.join(&safe_name);
    std::fs::create_dir_all(&sym_dir)?;
    for frame in &symbol.frames {
        if let Some(img) = frame.image_ref() {
            let out_path = sym_dir.join(format!("frame_{}.png", frame.frame_num));
            img.save(&out_path).map_err(|e| {
                dst_anim_tool::error::Error::Io(std::io::Error::other(e.to_string()))
            })?;
            println!("{}", out_path.display());
        }
    }
    Ok(())
}

fn cmd_split(
    inputs: &[PathBuf],
    skin: Option<&Path>,
    output_dir: &Path,
) -> dst_anim_tool::error::Result<()> {
    let mut base_archive = dst_anim_tool::archive::load_archives(inputs)?;
    if base_archive.build.is_none() {
        return Err(dst_anim_tool::error::Error::MissingData(
            "build.bin".to_string(),
        ));
    }

    {
        let base_tex = base_archive.tex_files();
        let base_atlas = dst_anim_tool::atlas::decode_atlas_images_from_tex(
            &base_archive.build.as_ref().unwrap().atlases,
            base_tex,
        );
        dst_anim_tool::atlas::split_atlas(base_archive.build.as_mut().unwrap(), &base_atlas)?;
    }

    let skin_build: Option<dst_anim_tool::build_file::BuildFile> = match skin {
        Some(skin_path) => {
            let mut skin_archive = load_skin_archive(skin_path)?;
            let skin_tex = skin_archive.tex_files().clone();
            let skin_atlas = dst_anim_tool::atlas::decode_atlas_images_from_tex(
                &skin_archive.build.as_ref().unwrap().atlases,
                &skin_tex,
            );
            dst_anim_tool::atlas::split_atlas(skin_archive.build.as_mut().unwrap(), &skin_atlas)?;
            Some(skin_archive.build.unwrap())
        }
        None => None,
    };

    let base_build = base_archive.build.as_ref().unwrap();
    std::fs::create_dir_all(output_dir)?;
    for symbol in &base_build.symbols {
        let effective: &dst_anim_tool::build_file::BuildSymbol = match &skin_build {
            Some(sb) => sb
                .symbol_index
                .get(&symbol.name.to_lowercase())
                .and_then(|&idx| sb.symbols.get(idx))
                .unwrap_or(symbol),
            None => symbol,
        };
        write_symbol_frames(effective, output_dir)?;
    }
    Ok(())
}

fn cmd_render(
    inputs: &[PathBuf],
    skin: Option<&Path>,
    anim_path: &str,
    output_dir: &Path,
) -> dst_anim_tool::error::Result<()> {
    let mut base_archive = dst_anim_tool::archive::load_archives(inputs)?;
    let anim = base_archive
        .anim
        .as_ref()
        .ok_or_else(|| dst_anim_tool::error::Error::MissingData("anim.bin".to_string()))?;
    if base_archive.build.is_none() {
        return Err(dst_anim_tool::error::Error::MissingData(
            "build.bin".to_string(),
        ));
    }

    let parts: Vec<&str> = anim_path.splitn(2, '/').collect();
    let bank_name = parts[0];
    let anim_name = parts.get(1).copied().unwrap_or("");

    let bank_idx = anim
        .banks
        .iter()
        .position(|b| b.name.to_lowercase() == bank_name.to_lowercase());
    let anim_idx = bank_idx.and_then(|bi| {
        anim.banks[bi]
            .animations
            .iter()
            .position(|a| a.name.to_lowercase() == anim_name.to_lowercase())
    });
    let (bank_idx, anim_idx) = match (bank_idx, anim_idx) {
        (Some(bi), Some(ai)) => (bi, ai),
        _ => {
            return Err(dst_anim_tool::error::Error::Other(format!(
                "animation not found: {anim_path}"
            )));
        }
    };

    {
        let base_tex = base_archive.tex_files();
        let base_build = base_archive.build.as_ref().unwrap();
        let base_atlas =
            dst_anim_tool::atlas::decode_atlas_images_from_tex(&base_build.atlases, base_tex);
        dst_anim_tool::atlas::split_atlas(base_archive.build.as_mut().unwrap(), &base_atlas)?;
    }

    let mut build_list: Vec<dst_anim_tool::build_file::BuildFile> =
        vec![base_archive.build.unwrap()];

    if let Some(skin_path) = skin {
        let mut skin_archive = load_skin_archive(skin_path)?;
        let skin_tex = skin_archive.tex_files().clone();
        let skin_build = skin_archive.build.as_mut().unwrap();
        let skin_atlas =
            dst_anim_tool::atlas::decode_atlas_images_from_tex(&skin_build.atlases, &skin_tex);
        dst_anim_tool::atlas::split_atlas(skin_build, &skin_atlas)?;
        build_list.insert(0, skin_archive.build.unwrap());
    }

    let animation = &base_archive.anim.as_ref().unwrap().banks[bank_idx].animations[anim_idx];
    let empty_disabled: HashSet<String> = HashSet::new();
    let bl: Vec<dst_anim_tool::render::BuildRef<'_>> = build_list
        .iter()
        .map(|b| dst_anim_tool::render::BuildRef {
            build: b,
            disabled_symbols: &empty_disabled,
        })
        .collect();
    std::fs::create_dir_all(output_dir)?;

    let (bounds, prepared) = dst_anim_tool::render::prepare_animation_frames(
        &animation.frames,
        &bl,
        1.0,
        (0.0, 0.0),
        &HashSet::new(),
    );

    for (i, pf) in prepared.iter().enumerate() {
        if let Some(pf) = pf {
            let render_bounds = bounds.as_ref().unwrap_or(&pf.bounds);
            if let Some(rendered) = dst_anim_tool::render::render_frame_with_elements(
                &pf.elements,
                render_bounds,
                1.0,
                (0.0, 0.0),
            ) {
                let out_path = output_dir.join(format!("frame_{i:03}.png"));
                rendered.image.save(&out_path).map_err(|e| {
                    dst_anim_tool::error::Error::Io(std::io::Error::other(e.to_string()))
                })?;
                println!("{}", out_path.display());
            }
        }
    }
    Ok(())
}

fn cmd_list(inputs: &[PathBuf]) -> dst_anim_tool::error::Result<()> {
    let archive = dst_anim_tool::archive::load_archives(inputs)?;
    if let Some(anim) = &archive.anim {
        for bank in &anim.banks {
            for animation in &bank.animations {
                println!("{}/{}", bank.name, animation.name);
            }
        }
    } else {
        eprintln!("no anim.bin found");
    }
    Ok(())
}

fn cmd_info(inputs: &[PathBuf]) -> dst_anim_tool::error::Result<()> {
    let archive = dst_anim_tool::archive::load_archives(inputs)?;

    if let Some(anim) = &archive.anim {
        println!("anim.bin:");
        println!("  version: {}", anim.version);
        println!("  banks: {}", anim.banks.len());
        for bank in &anim.banks {
            println!("    bank '{}':", bank.name);
            for animation in &bank.animations {
                println!(
                    "      animation '{}': {} frames",
                    animation.name,
                    animation.frames.len()
                );
            }
        }
    } else {
        println!("anim.bin: not found");
    }

    if let Some(build) = &archive.build {
        println!("build.bin:");
        println!("  version: {}", build.version);
        println!("  name: {}", build.name);
        println!("  symbols: {}", build.symbols.len());
        for symbol in &build.symbols {
            println!(
                "    symbol '{}': {} frames",
                symbol.name,
                symbol.frames.len()
            );
        }
        println!("  atlases: {}", build.atlases.len());
        for atlas in &build.atlases {
            println!("    atlas: {}", atlas.name);
        }
    } else {
        println!("build.bin: not found");
    }

    println!("tex sources: {}", archive.tex_sources.len());
    for source in &archive.tex_sources {
        println!("  source '{}':", source.source_name);
        for (name, data) in &source.tex_files {
            if let Ok(ktex) = dst_anim_tool::ktex::parse_ktex(data) {
                let m0 = &ktex.mipmaps[0];
                println!(
                    "    {}: {}x{} {:?}",
                    name, m0.width, m0.height, ktex.header.pixel_format
                );
            } else {
                println!("    {}: parse error", name);
            }
        }
    }

    Ok(())
}

fn cmd_decrypt(input: &Path, output: &Path) -> dst_anim_tool::error::Result<()> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "dyn" {
        return Err(dst_anim_tool::error::Error::Other(
            "decrypt only supports .dyn files".to_string(),
        ));
    }
    let data = std::fs::read(input)?;
    let decrypted = dst_anim_tool::xor::xor_decrypt(&data);
    std::fs::write(output, &decrypted)?;
    println!("{}", output.display());
    Ok(())
}

fn cmd_decode(input: &Path, output_dir: &Path) -> dst_anim_tool::error::Result<()> {
    let archive = dst_anim_tool::archive::load_archives(&[input.to_path_buf()])?;
    if archive.tex_sources.is_empty() {
        eprintln!("no .tex files found");
        return Ok(());
    }
    std::fs::create_dir_all(output_dir)?;
    let tex_files = archive.tex_files();
    for (name, data) in tex_files {
        let ktex = dst_anim_tool::ktex::parse_ktex(data)?;
        let img = ktex.to_image_rgba()?;
        let out_name = format!("{}.png", name);
        let out_path = output_dir.join(&out_name);
        img.save(&out_path)
            .map_err(|e| dst_anim_tool::error::Error::Io(std::io::Error::other(e.to_string())))?;
        println!("{}", out_path.display());
    }
    Ok(())
}

fn cmd_preview(inputs: Option<Vec<PathBuf>>) -> dst_anim_tool::error::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DST Anim Tool",
        native_options,
        Box::new(move |_cc| Ok(Box::new(crate::ui::App::new(inputs)))),
    )
    .map_err(|e| dst_anim_tool::error::Error::Ui(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_produces_valid_zip() {
        let input = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.dyn");
        let data = std::fs::read(&input).unwrap();
        let decrypted = dst_anim_tool::xor::xor_decrypt(&data);
        let archive = zip::ZipArchive::new(std::io::Cursor::new(decrypted.as_slice())).unwrap();
        assert!(archive.len() > 0);
    }

    #[test]
    fn decode_dyn_to_png() {
        let input = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.dyn");
        let archive = dst_anim_tool::archive::load_archives(&[input]).unwrap();
        assert!(!archive.tex_sources.is_empty());
        let tex_files = archive.tex_files();
        assert!(!tex_files.is_empty());
        for (name, data) in tex_files {
            let ktex = dst_anim_tool::ktex::parse_ktex(data).unwrap();
            let img = ktex.to_image_rgba().unwrap();
            assert!(img.width() > 0);
            assert!(img.height() > 0);
            let out_name = format!("{}.png", name);
            assert!(out_name.ends_with(".tex.png"));
        }
    }

    #[test]
    fn load_skin_archive_dyn() {
        let skin_path = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.dyn");
        let archive = load_skin_archive(&skin_path).unwrap();
        assert!(archive.build.is_some());
        assert!(!archive.tex_sources.is_empty());
        let build = archive.build.unwrap();
        assert_eq!(build.name, "abigail_ice");
    }

    #[test]
    fn load_skin_archive_build_zip() {
        let skin_path = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.zip");
        let archive = load_skin_archive(&skin_path).unwrap();
        assert!(archive.build.is_some());
        assert!(!archive.tex_sources.is_empty());
        let build = archive.build.unwrap();
        assert_eq!(build.name, "abigail_ice");
    }
}
