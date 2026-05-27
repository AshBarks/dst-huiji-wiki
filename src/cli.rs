use std::path::PathBuf;

use clap::Subcommand;

#[derive(clap::Parser)]
#[command(name = "dst-anim-tool", about = "DST animation file extraction tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Extract {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
        output_dir: PathBuf,
    },
    Split {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
        output_dir: PathBuf,
    },
    Render {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
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

pub fn run(cli: Cli) -> crate::error::Result<()> {
    match cli.command {
        Commands::Extract { input, output_dir } => cmd_extract(&input, &output_dir),
        Commands::Split { input, output_dir } => cmd_split(&input, &output_dir),
        Commands::Render {
            input,
            anim_path,
            output_dir,
        } => cmd_render(&input, &anim_path, &output_dir),
        Commands::List { input } => cmd_list(&input),
        Commands::Info { input } => cmd_info(&input),
        Commands::Decrypt { input, output } => cmd_decrypt(&input, &output),
        Commands::Decode { input, output_dir } => cmd_decode(&input, &output_dir),
        Commands::Preview { input } => cmd_preview(input),
    }
}

fn cmd_extract(inputs: &[PathBuf], output_dir: &PathBuf) -> crate::error::Result<()> {
    let archive = crate::archive::load_archives(inputs)?;
    std::fs::create_dir_all(output_dir)?;
    for (name, data) in &archive.raw_files {
        let out_path = output_dir.join(name);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&out_path, data)?;
        println!("{}", out_path.display());
    }
    Ok(())
}

fn cmd_split(inputs: &[PathBuf], output_dir: &PathBuf) -> crate::error::Result<()> {
    let mut archive = crate::archive::load_archives(inputs)?;
    if archive.build.is_none() {
        return Err(crate::error::Error::UnknownFormat(
            "no build.bin found".to_string(),
        ));
    }

    let atlas_images = crate::atlas::decode_atlas_images(&archive);
    crate::atlas::split_atlas(archive.build.as_mut().unwrap(), &atlas_images)?;

    let build = archive.build.as_ref().unwrap();
    std::fs::create_dir_all(output_dir)?;
    for symbol in &build.symbols {
        let safe_name = symbol.name.replace(['/', '\\', ':'], "_");
        let sym_dir = output_dir.join(&safe_name);
        let has_images = symbol.frames.iter().any(|f| f.image.is_some());
        if !has_images {
            continue;
        }
        std::fs::create_dir_all(&sym_dir)?;
        for frame in &symbol.frames {
            if let Some(img) = frame.image_ref() {
                let out_path = sym_dir.join(format!("frame_{}.png", frame.frame_num));
                img.save(&out_path)
                    .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?;
                println!("{}", out_path.display());
            }
        }
    }
    Ok(())
}

fn cmd_render(
    inputs: &[PathBuf],
    anim_path: &str,
    output_dir: &PathBuf,
) -> crate::error::Result<()> {
    let mut archive = crate::archive::load_archives(inputs)?;
    let anim = archive
        .anim
        .as_ref()
        .ok_or_else(|| crate::error::Error::UnknownFormat("no anim.bin found".to_string()))?;
    if archive.build.is_none() {
        return Err(crate::error::Error::UnknownFormat(
            "no build.bin found".to_string(),
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
            return Err(crate::error::Error::UnknownFormat(format!(
                "animation not found: {anim_path}"
            )));
        }
    };

    let atlas_images = crate::atlas::decode_atlas_images(&archive);
    crate::atlas::split_atlas(archive.build.as_mut().unwrap(), &atlas_images)?;

    let animation = &archive.anim.as_ref().unwrap().banks[bank_idx].animations[anim_idx];
    let build = archive.build.as_ref().unwrap();
    let build_list: Vec<&crate::build_file::BuildFile> = vec![build];
    std::fs::create_dir_all(output_dir)?;

    let bounds =
        crate::render::compute_animation_bounds(&animation.frames, &build_list, 1.0, (0.0, 0.0));

    for (i, frame) in animation.frames.iter().enumerate() {
        if let Some(rendered) =
            crate::render::render_frame(frame, &build_list, 1.0, (0.0, 0.0), bounds.as_ref())
        {
            let out_path = output_dir.join(format!("frame_{i:03}.png"));
            rendered
                .image
                .save(&out_path)
                .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?;
            println!("{}", out_path.display());
        }
    }
    Ok(())
}

fn cmd_list(inputs: &[PathBuf]) -> crate::error::Result<()> {
    let archive = crate::archive::load_archives(inputs)?;
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

fn cmd_info(inputs: &[PathBuf]) -> crate::error::Result<()> {
    let archive = crate::archive::load_archives(inputs)?;

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

    println!("tex files: {}", archive.tex_files.len());
    for (name, data) in &archive.tex_files {
        if let Ok(ktex) = crate::ktex::parse_ktex(data) {
            let m0 = &ktex.mipmaps[0];
            println!(
                "  {}: {}x{} {:?}",
                name, m0.width, m0.height, ktex.header.pixel_format
            );
        } else {
            println!("  {}: parse error", name);
        }
    }

    Ok(())
}

fn cmd_decrypt(input: &PathBuf, output: &PathBuf) -> crate::error::Result<()> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "dyn" {
        return Err(crate::error::Error::UnknownFormat(
            "decrypt only supports .dyn files".to_string(),
        ));
    }
    let data = std::fs::read(input)?;
    let decrypted = crate::xor::xor_decrypt(&data);
    std::fs::write(output, &decrypted)?;
    println!("{}", output.display());
    Ok(())
}

fn cmd_decode(input: &PathBuf, output_dir: &PathBuf) -> crate::error::Result<()> {
    let archive = crate::archive::load_archives(&[input.clone()])?;
    if archive.tex_files.is_empty() {
        eprintln!("no .tex files found");
        return Ok(());
    }
    std::fs::create_dir_all(output_dir)?;
    for (name, data) in &archive.tex_files {
        let ktex = crate::ktex::parse_ktex(data)?;
        let img = ktex.to_image_rgba()?;
        let out_name = format!("{}.png", name);
        let out_path = output_dir.join(&out_name);
        img.save(&out_path)
            .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?;
        println!("{}", out_path.display());
    }
    Ok(())
}

fn cmd_preview(inputs: Option<Vec<PathBuf>>) -> crate::error::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DST Anim Tool",
        native_options,
        Box::new(move |_cc| Ok(Box::new(crate::ui::App::new(inputs)))),
    )
    .map_err(|e| crate::error::Error::Ui(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_produces_valid_zip() {
        let input = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.dyn");
        let data = std::fs::read(&input).unwrap();
        let decrypted = crate::xor::xor_decrypt(&data);
        let archive = zip::ZipArchive::new(std::io::Cursor::new(decrypted.as_slice())).unwrap();
        assert!(archive.len() > 0);
    }

    #[test]
    fn decode_dyn_to_png() {
        let input = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.dyn");
        let archive = crate::archive::load_archives(&[input]).unwrap();
        assert!(!archive.tex_files.is_empty());
        for (name, data) in &archive.tex_files {
            let ktex = crate::ktex::parse_ktex(data).unwrap();
            let img = ktex.to_image_rgba().unwrap();
            assert!(img.width() > 0);
            assert!(img.height() > 0);
            let out_name = format!("{}.png", name);
            assert!(out_name.ends_with(".tex.png"));
        }
    }
}
