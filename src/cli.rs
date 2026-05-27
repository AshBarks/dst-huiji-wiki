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
        input: PathBuf,
        output_dir: PathBuf,
    },
    Split {
        input: PathBuf,
        output_dir: PathBuf,
    },
    Render {
        input: PathBuf,
        anim_path: String,
        output_dir: PathBuf,
    },
    List {
        input: PathBuf,
    },
    Info {
        input: PathBuf,
    },
}

fn load_archive(path: &PathBuf) -> crate::error::Result<crate::archive::ParsedArchive> {
    let data = std::fs::read(path)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "zip" => crate::archive::parse_zip(&data),
        "dyn" => crate::archive::parse_dyn(&data),
        _ => Err(crate::error::Error::UnknownFormat(format!(
            "unsupported file extension: .{ext}"
        ))),
    }
}

fn decode_atlas_images(archive: &crate::archive::ParsedArchive) -> Vec<image::RgbaImage> {
    let build = match &archive.build {
        Some(b) => b,
        None => return Vec::new(),
    };
    let mut atlas_images: Vec<image::RgbaImage> = Vec::new();
    for atlas in &build.atlases {
        let tex_data = archive.tex_files.get(&atlas.name);
        if let Some(tex_data) = tex_data
            && let Ok(ktex) = crate::ktex::parse_ktex(tex_data)
            && let Ok(img) = ktex.to_image_rgba()
        {
            atlas_images.push(img);
            continue;
        }
        atlas_images.push(image::RgbaImage::new(1, 1));
    }
    atlas_images
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
    }
}

fn cmd_extract(input: &PathBuf, output_dir: &PathBuf) -> crate::error::Result<()> {
    let archive = load_archive(input)?;
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

fn cmd_split(input: &PathBuf, output_dir: &PathBuf) -> crate::error::Result<()> {
    let mut archive = load_archive(input)?;
    if archive.build.is_none() {
        return Err(crate::error::Error::UnknownFormat(
            "no build.bin found".to_string(),
        ));
    }

    let atlas_images = decode_atlas_images(&archive);
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
            if let Some(ref img) = frame.image {
                let out_path = sym_dir.join(format!("frame_{}.png", frame.frame_num));
                img.save(&out_path)
                    .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?;
                println!("{}", out_path.display());
            }
        }
    }
    Ok(())
}

fn cmd_render(input: &PathBuf, anim_path: &str, output_dir: &PathBuf) -> crate::error::Result<()> {
    let mut archive = load_archive(input)?;
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

    let atlas_images = decode_atlas_images(&archive);
    crate::atlas::split_atlas(archive.build.as_mut().unwrap(), &atlas_images)?;

    let animation = &archive.anim.as_ref().unwrap().banks[bank_idx].animations[anim_idx];
    let build = archive.build.as_ref().unwrap();
    let build_list: Vec<&crate::build_file::BuildFile> = vec![build];
    std::fs::create_dir_all(output_dir)?;

    for (i, frame) in animation.frames.iter().enumerate() {
        if let Some(rendered) = crate::render::render_frame(frame, &build_list, 1.0, (0.0, 0.0)) {
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

fn cmd_list(input: &PathBuf) -> crate::error::Result<()> {
    let archive = load_archive(input)?;
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

fn cmd_info(input: &PathBuf) -> crate::error::Result<()> {
    let archive = load_archive(input)?;

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
