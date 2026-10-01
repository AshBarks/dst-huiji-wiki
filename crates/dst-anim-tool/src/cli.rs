use std::collections::HashSet;
use std::path::{Path, PathBuf};

use clap::{Subcommand, ValueEnum};
use rayon::prelude::*;
use serde::Serialize;

use dst_anim_tool::anim::{AnimAnimation, AnimBank, AnimFile};
use dst_anim_tool::archive::ParsedArchive;
use dst_anim_tool::build_file::BuildFile;
use dst_anim_tool::error::{Error, Result};
use dst_anim_tool::render::{
    render_frame_with_elements, BoundingBox, BoundsKind, BuildRef, RenderOptions, SymbolOverrideMap,
};

#[derive(clap::Parser)]
#[command(name = "dst-anim-tool", about = "DST animation file extraction tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum FrameBoundsArg {
    Rect,
    Ink,
}

impl FrameBoundsArg {
    fn to_kind(self) -> BoundsKind {
        match self {
            Self::Rect => BoundsKind::Rect,
            Self::Ink => BoundsKind::Ink,
        }
    }
}

#[allow(clippy::large_enum_variant)]
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
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long)]
        skin: Option<PathBuf>,
        #[arg(long, value_name = "FILE")]
        symbol_map: Option<PathBuf>,
        #[arg(long, value_name = "FILE", num_args = 1..)]
        build: Vec<PathBuf>,
        #[arg(long, value_name = "BANK/ANIM")]
        anim: Option<String>,
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
        #[arg(long, value_name = "NAME")]
        hide_layer: Vec<String>,
        #[arg(long, value_name = "NAME")]
        disable_symbol: Vec<String>,
        #[arg(long, value_enum, default_value_t = FrameBoundsArg::Rect)]
        frame_bounds: FrameBoundsArg,
        #[arg(long, visible_alias = "meta", value_name = "FILE")]
        manifest: Option<PathBuf>,
        #[arg(long)]
        dump_elements: bool,
        #[arg(long)]
        dump_symbol: bool,
        #[arg(long)]
        strict: bool,
        #[arg(long, value_name = "FILE")]
        jobs: Option<PathBuf>,
        anim_path: Option<String>,
        output_dir: Option<PathBuf>,
    },
    List {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
        #[arg(long)]
        allow_missing: bool,
        #[arg(long)]
        json: bool,
    },
    Info {
        #[arg(short, long, num_args = 1.., required = true)]
        input: Vec<PathBuf>,
        #[arg(long)]
        allow_missing: bool,
        #[arg(long)]
        json: bool,
    },
    Decrypt {
        input: PathBuf,
        output: PathBuf,
    },
    Decode {
        input: PathBuf,
        output_dir: PathBuf,
        #[arg(long)]
        allow_missing: bool,
    },
    Search {
        #[arg(short, long)]
        dir: PathBuf,
        symbol: String,
        #[arg(long)]
        case_sensitive: bool,
    },
    #[cfg(feature = "gui")]
    Preview {
        #[arg(short, long)]
        input: Option<Vec<PathBuf>>,
    },
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => {
            #[cfg(feature = "gui")]
            {
                cmd_preview(None)
            }
            #[cfg(not(feature = "gui"))]
            {
                Err(Error::Other(
                    "preview command requires the 'gui' feature".to_string(),
                ))
            }
        }
        Some(command) => match command {
            Commands::Split {
                input,
                skin,
                output_dir,
            } => cmd_split(&input, skin.as_deref(), &output_dir),
            Commands::Render {
                input,
                skin,
                symbol_map,
                build,
                anim,
                out,
                hide_layer,
                disable_symbol,
                frame_bounds,
                manifest,
                dump_elements,
                dump_symbol,
                strict,
                jobs,
                anim_path,
                output_dir,
            } => cmd_render(RenderArgs {
                input,
                skin,
                symbol_map,
                build,
                anim,
                out,
                hide_layer,
                disable_symbol,
                frame_bounds,
                manifest,
                dump_elements,
                dump_symbol,
                strict,
                jobs,
                anim_path,
                output_dir,
            }),
            Commands::List {
                input,
                allow_missing,
                json,
            } => cmd_list(&input, allow_missing, json),
            Commands::Info {
                input,
                allow_missing,
                json,
            } => cmd_info(&input, allow_missing, json),
            Commands::Decrypt { input, output } => cmd_decrypt(&input, &output),
            Commands::Decode {
                input,
                output_dir,
                allow_missing,
            } => cmd_decode(&input, &output_dir, allow_missing),
            Commands::Search {
                dir,
                symbol,
                case_sensitive,
            } => cmd_search(&dir, &symbol, case_sensitive),
            #[cfg(feature = "gui")]
            Commands::Preview { input } => cmd_preview(input),
        },
    }
}

fn load_skin_archive(path: &Path) -> Result<ParsedArchive> {
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
                return Err(Error::MissingCompanion(
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
                    return Err(Error::MissingCompanion(
                        "no companion .dyn found for build-only .zip skin file".to_string(),
                    ));
                }
            }
        }
        _ => {
            return Err(Error::Other("skin file must be .dyn or .zip".to_string()));
        }
    }

    if archive.build.is_none() {
        return Err(Error::MissingData("build.bin in skin".to_string()));
    }
    if archive.tex_sources.iter().all(|s| s.tex_files.is_empty()) {
        return Err(Error::MissingData("atlas textures in skin".to_string()));
    }

    Ok(archive)
}

fn write_symbol_frames(
    symbol: &dst_anim_tool::build_file::BuildSymbol,
    output_dir: &Path,
) -> Result<()> {
    let has_images = symbol.frames.iter().any(|f| f.image.is_some());
    if !has_images {
        return Ok(());
    }
    let safe_name = symbol.name.replace(['/', '\\', ':'], "_");
    let sym_dir = output_dir.join(&safe_name);
    std::fs::create_dir_all(&sym_dir)?;
    for frame in &symbol.frames {
        if let Some(img) = dst_anim_tool::atlas::rebuild_frame_canvas(frame) {
            let out_path = sym_dir.join(format!("frame_{}.png", frame.frame_num));
            img.save(&out_path)
                .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
            println!("{}", out_path.display());
        }
    }
    Ok(())
}

fn cmd_split(inputs: &[PathBuf], skin: Option<&Path>, output_dir: &Path) -> Result<()> {
    let mut base_archive = dst_anim_tool::archive::load_archives(inputs)?;
    if base_archive.build.is_none() {
        return Err(Error::MissingData("build.bin".to_string()));
    }

    {
        let base_tex = base_archive.tex_files();
        let base_atlas = dst_anim_tool::atlas::decode_atlas_images_from_tex(
            &base_archive.build.as_ref().unwrap().atlases,
            base_tex,
        );
        dst_anim_tool::atlas::split_atlas(base_archive.build.as_mut().unwrap(), &base_atlas)?;
    }

    let skin_build: Option<BuildFile> = match skin {
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

struct RenderArgs {
    input: Vec<PathBuf>,
    skin: Option<PathBuf>,
    symbol_map: Option<PathBuf>,
    build: Vec<PathBuf>,
    anim: Option<String>,
    out: Option<PathBuf>,
    hide_layer: Vec<String>,
    disable_symbol: Vec<String>,
    frame_bounds: FrameBoundsArg,
    manifest: Option<PathBuf>,
    dump_elements: bool,
    dump_symbol: bool,
    strict: bool,
    jobs: Option<PathBuf>,
    anim_path: Option<String>,
    output_dir: Option<PathBuf>,
}

struct RenderContext {
    base_archive: ParsedArchive,
    build_list: Vec<BuildFile>,
    overrides: SymbolOverrideMap,
    input_names: Vec<String>,
}

struct JobOptions {
    hide_layers: HashSet<String>,
    disable_symbols: HashSet<String>,
    frame_bounds: BoundsKind,
}

fn load_symbol_map(path: Option<&Path>) -> Result<SymbolOverrideMap> {
    match path {
        Some(path) => {
            let text = std::fs::read_to_string(path)?;
            dst_anim_tool::render::parse_symbol_map(&text).map_err(Error::Other)
        }
        None => Ok(SymbolOverrideMap::new()),
    }
}

fn load_render_context(
    inputs: &[PathBuf],
    skin: Option<&Path>,
    symbol_map: Option<&Path>,
    extra_builds: &[PathBuf],
) -> Result<RenderContext> {
    let overrides = load_symbol_map(symbol_map)?;
    if !overrides.is_empty() {
        println!("symbol overrides: {} entries", overrides.len());
    }

    let mut base_archive = dst_anim_tool::archive::load_archives(inputs)?;
    let mut build_list: Vec<BuildFile> = Vec::new();

    if let Some(mut base_build) = base_archive.build.take() {
        let base_tex = base_archive.tex_files();
        let base_atlas =
            dst_anim_tool::atlas::decode_atlas_images_from_tex(&base_build.atlases, base_tex);
        dst_anim_tool::atlas::split_atlas(&mut base_build, &base_atlas)?;
        build_list.push(base_build);
    }

    if let Some(skin_path) = skin {
        let mut skin_archive = load_skin_archive(skin_path)?;
        let skin_tex = skin_archive.tex_files().clone();
        let skin_build = skin_archive.build.as_mut().unwrap();
        let skin_atlas =
            dst_anim_tool::atlas::decode_atlas_images_from_tex(&skin_build.atlases, &skin_tex);
        dst_anim_tool::atlas::split_atlas(skin_build, &skin_atlas)?;
        build_list.insert(0, skin_archive.build.unwrap());
    }

    for extra in extra_builds {
        let ext = extra
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let mut archive = if ext == "dyn" {
            load_skin_archive(extra)?
        } else {
            dst_anim_tool::archive::load_archives(std::slice::from_ref(extra))?
        };
        if archive.build.is_none() {
            return Err(Error::MissingData(format!(
                "build.bin in {}",
                extra.display()
            )));
        }
        let tex = archive.tex_files().clone();
        let build = archive.build.as_mut().unwrap();
        let atlas = dst_anim_tool::atlas::decode_atlas_images_from_tex(&build.atlases, &tex);
        dst_anim_tool::atlas::split_atlas(build, &atlas)?;
        build_list.push(archive.build.unwrap());
        println!(
            "extra build: {} ({})",
            build_list.last().unwrap().name,
            extra.display()
        );
    }

    if build_list.is_empty() {
        return Err(Error::MissingData("build.bin".to_string()));
    }

    let input_names = inputs.iter().map(|p| p.display().to_string()).collect();
    Ok(RenderContext {
        base_archive,
        build_list,
        overrides,
        input_names,
    })
}

fn find_animation<'a>(
    anim: &'a AnimFile,
    anim_path: &str,
) -> Result<(&'a AnimBank, &'a AnimAnimation)> {
    let parts: Vec<&str> = anim_path.splitn(2, '/').collect();
    let bank_name = parts[0];
    let anim_name = parts.get(1).copied().unwrap_or("");
    for bank in &anim.banks {
        if bank.name.to_lowercase() == bank_name.to_lowercase() {
            for animation in &bank.animations {
                if animation.name.to_lowercase() == anim_name.to_lowercase() {
                    return Ok((bank, animation));
                }
            }
        }
    }
    Err(Error::Other(format!("animation not found: {anim_path}")))
}

#[derive(Serialize, Clone, Copy)]
struct BoundsJson {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl From<&BoundingBox> for BoundsJson {
    fn from(bounds: &BoundingBox) -> Self {
        Self {
            left: bounds.left,
            top: bounds.top,
            right: bounds.right,
            bottom: bounds.bottom,
        }
    }
}

#[derive(Serialize)]
struct FrameEntry {
    src_index: usize,
    file: String,
    bounds: BoundsJson,
    ink_bounds: Option<BoundsJson>,
}

#[derive(Serialize)]
struct MissingEntry {
    symbol: String,
    layer: String,
    lookup_symbol: String,
    kind: &'static str,
    frames: Vec<usize>,
}

#[derive(Serialize)]
struct Manifest {
    sources: Vec<String>,
    bank: String,
    anim: String,
    frame_rate: f32,
    scale: f32,
    offset: [f32; 2],
    frame_bounds: &'static str,
    output_dir: String,
    hidden_layers: Vec<String>,
    disabled_symbols: Vec<String>,
    frames: Vec<FrameEntry>,
    skipped: Vec<usize>,
    missing_symbols: Vec<MissingEntry>,
}

fn missing_kind_str(kind: dst_anim_tool::render::MissingSymbolKind) -> &'static str {
    use dst_anim_tool::render::MissingSymbolKind;
    match kind {
        MissingSymbolKind::NoSymbol => "NoSymbol",
        MissingSymbolKind::NoFrame => "NoFrame",
        MissingSymbolKind::NoImage => "NoImage",
    }
}

fn print_diagnostics(diagnostics: &dst_anim_tool::render::RenderDiagnostics) {
    for info in diagnostics.missing.values() {
        eprintln!(
            "missing symbol: {} ({} frames, layer={}, kind={})",
            info.symbol,
            info.frames.len(),
            info.layer,
            missing_kind_str(info.kind)
        );
    }
    if !diagnostics.skipped_frames.is_empty() {
        eprintln!("skipped frames: {:?}", diagnostics.skipped_frames);
    }
}

fn render_animation(
    ctx: &RenderContext,
    anim_path: &str,
    output_dir: &Path,
    opts: &JobOptions,
    manifest_path: Option<&Path>,
    strict: bool,
    print_paths: bool,
) -> Result<()> {
    let anim_file = ctx
        .base_archive
        .anim
        .as_ref()
        .ok_or_else(|| Error::MissingData("anim.bin".to_string()))?;
    let (bank, animation) = find_animation(anim_file, anim_path)?;

    let empty: HashSet<String> = HashSet::new();
    let bl: Vec<BuildRef<'_>> = ctx
        .build_list
        .iter()
        .map(|b| BuildRef {
            build: b,
            disabled_symbols: &empty,
        })
        .collect();

    let options = RenderOptions::new(1.0, (0.0, 0.0))
        .with_disabled_symbols(opts.disable_symbols.clone())
        .with_disabled_layers(opts.hide_layers.clone())
        .with_overrides(ctx.overrides.clone())
        .with_bounds_kind(opts.frame_bounds);
    let (bounds, prepared, diagnostics) =
        dst_anim_tool::render::prepare_animation_frames_opts(&animation.frames, &bl, &options);

    print_diagnostics(&diagnostics);
    std::fs::create_dir_all(output_dir)?;

    let entries: Vec<Option<FrameEntry>> = prepared
        .par_iter()
        .enumerate()
        .map(|(i, pf)| -> Result<Option<FrameEntry>> {
            let Some(pf) = pf else {
                return Ok(None);
            };
            let render_bounds = bounds.as_ref().unwrap_or(&pf.bounds);
            let Some(rendered) =
                render_frame_with_elements(&pf.elements, render_bounds, 1.0, (0.0, 0.0))
            else {
                return Ok(None);
            };
            let file_name = format!("frame_{i:03}.png");
            let out_path = output_dir.join(&file_name);
            rendered
                .image
                .save(&out_path)
                .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
            if print_paths {
                println!("{}", out_path.display());
            }
            Ok(Some(FrameEntry {
                src_index: i,
                file: file_name,
                bounds: BoundsJson::from(&pf.bounds),
                ink_bounds: rendered.ink_bounds.as_ref().map(BoundsJson::from),
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    let frames: Vec<FrameEntry> = entries.into_iter().flatten().collect();
    let present: HashSet<usize> = frames.iter().map(|f| f.src_index).collect();
    let skipped: Vec<usize> = (0..prepared.len())
        .filter(|i| !present.contains(i))
        .collect();

    if let Some(path) = manifest_path {
        let mut hidden_layers: Vec<String> = opts.hide_layers.iter().cloned().collect();
        hidden_layers.sort();
        let mut disabled_symbols: Vec<String> = opts.disable_symbols.iter().cloned().collect();
        disabled_symbols.sort();
        let missing_symbols: Vec<MissingEntry> = diagnostics
            .missing
            .values()
            .map(|info| MissingEntry {
                symbol: info.symbol.clone(),
                layer: info.layer.clone(),
                lookup_symbol: info.lookup_symbol.clone(),
                kind: missing_kind_str(info.kind),
                frames: info.frames.clone(),
            })
            .collect();
        let manifest = Manifest {
            sources: ctx.input_names.clone(),
            bank: bank.name.clone(),
            anim: animation.name.clone(),
            frame_rate: animation.frame_rate,
            scale: options.scale,
            offset: [options.offset.0, options.offset.1],
            frame_bounds: match opts.frame_bounds {
                BoundsKind::Rect => "rect",
                BoundsKind::Ink => "ink",
            },
            output_dir: output_dir.display().to_string(),
            hidden_layers,
            disabled_symbols,
            frames,
            skipped,
            missing_symbols,
        };
        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| Error::Other(format!("manifest: {e}")))?;
        std::fs::write(path, json)?;
        println!("manifest: {}", path.display());
    }

    if strict && diagnostics.has_missing() {
        return Err(Error::Other(format!(
            "{} missing symbol(s) in {anim_path} (--strict)",
            diagnostics.missing.len()
        )));
    }
    Ok(())
}

fn dump_symbols(ctx: &RenderContext) -> Result<()> {
    #[derive(Serialize)]
    struct SymbolFrameJson {
        frame_num: u32,
        duration: u32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }
    #[derive(Serialize)]
    struct SymbolJson<'a> {
        name: &'a str,
        frames: Vec<SymbolFrameJson>,
    }
    #[derive(Serialize)]
    struct BuildJson<'a> {
        name: &'a str,
        atlases: Vec<&'a str>,
        symbols: Vec<SymbolJson<'a>>,
    }

    let dumps: Vec<BuildJson<'_>> = ctx
        .build_list
        .iter()
        .map(|build| BuildJson {
            name: &build.name,
            atlases: build.atlases.iter().map(|a| a.name.as_str()).collect(),
            symbols: build
                .symbols
                .iter()
                .map(|symbol| SymbolJson {
                    name: &symbol.name,
                    frames: symbol
                        .frames
                        .iter()
                        .map(|f| SymbolFrameJson {
                            frame_num: f.frame_num,
                            duration: f.duration,
                            x: f.x,
                            y: f.y,
                            width: f.width,
                            height: f.height,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    let json = serde_json::to_string_pretty(&dumps)
        .map_err(|e| Error::Other(format!("dump-symbol: {e}")))?;
    println!("{json}");
    Ok(())
}

fn dump_elements(ctx: &RenderContext, anim_path: &str, opts: &JobOptions) -> Result<()> {
    #[derive(Serialize)]
    struct ElementJson<'a> {
        symbol: &'a str,
        layer: &'a str,
        frame_num: u32,
        z_index: f32,
        matrix: [f32; 4],
        translation: [f32; 2],
    }
    #[derive(Serialize)]
    struct FrameJson<'a> {
        src_index: usize,
        bounds: Option<BoundsJson>,
        ink_bounds: Option<BoundsJson>,
        elements: Vec<ElementJson<'a>>,
    }
    #[derive(Serialize)]
    struct DumpJson<'a> {
        bank: &'a str,
        anim: &'a str,
        frame_rate: f32,
        frames: Vec<FrameJson<'a>>,
    }

    let anim_file = ctx
        .base_archive
        .anim
        .as_ref()
        .ok_or_else(|| Error::MissingData("anim.bin".to_string()))?;
    let (bank, animation) = find_animation(anim_file, anim_path)?;

    let empty: HashSet<String> = HashSet::new();
    let bl: Vec<BuildRef<'_>> = ctx
        .build_list
        .iter()
        .map(|b| BuildRef {
            build: b,
            disabled_symbols: &empty,
        })
        .collect();
    let options = RenderOptions::new(1.0, (0.0, 0.0))
        .with_disabled_symbols(opts.disable_symbols.clone())
        .with_disabled_layers(opts.hide_layers.clone())
        .with_overrides(ctx.overrides.clone())
        .with_bounds_kind(opts.frame_bounds);
    let (_, prepared, _) =
        dst_anim_tool::render::prepare_animation_frames_opts(&animation.frames, &bl, &options);

    let frames: Vec<FrameJson<'_>> = animation
        .frames
        .iter()
        .enumerate()
        .map(|(i, frame)| {
            let pf = prepared.get(i).and_then(|p| p.as_ref());
            FrameJson {
                src_index: i,
                bounds: pf.map(|p| BoundsJson::from(&p.bounds)),
                ink_bounds: pf.and_then(|p| p.ink_bounds.as_ref().map(BoundsJson::from)),
                elements: frame
                    .elements
                    .iter()
                    .map(|e| ElementJson {
                        symbol: &e.symbol,
                        layer: &e.layer_name,
                        frame_num: e.frame_num,
                        z_index: e.z_index,
                        matrix: [e.a, e.b, e.c, e.d],
                        translation: [e.tx, e.ty],
                    })
                    .collect(),
            }
        })
        .collect();
    let dump = DumpJson {
        bank: &bank.name,
        anim: &animation.name,
        frame_rate: animation.frame_rate,
        frames,
    };
    let json = serde_json::to_string_pretty(&dump)
        .map_err(|e| Error::Other(format!("dump-elements: {e}")))?;
    println!("{json}");
    Ok(())
}

fn job_options_from_args(args: &RenderArgs) -> JobOptions {
    JobOptions {
        hide_layers: args.hide_layer.iter().cloned().collect(),
        disable_symbols: args.disable_symbol.iter().cloned().collect(),
        frame_bounds: args.frame_bounds.to_kind(),
    }
}

fn cmd_render(args: RenderArgs) -> Result<()> {
    if let Some(jobs_path) = args.jobs.clone() {
        return cmd_render_jobs(&args, &jobs_path);
    }

    let anim_path = args.anim.clone().or_else(|| args.anim_path.clone());
    let output_dir = args.out.clone().or_else(|| args.output_dir.clone());
    let ctx = load_render_context(
        &args.input,
        args.skin.as_deref(),
        args.symbol_map.as_deref(),
        &args.build,
    )?;

    let opts = job_options_from_args(&args);
    let mut did_something = false;

    if args.dump_symbol {
        dump_symbols(&ctx)?;
        did_something = true;
    }
    if args.dump_elements {
        let anim_path = anim_path.as_ref().ok_or_else(|| {
            Error::Other(
                "--dump-elements requires an animation (--anim or <ANIM_PATH>)".to_string(),
            )
        })?;
        dump_elements(&ctx, anim_path, &opts)?;
        did_something = true;
    }

    let Some(anim_path) = anim_path else {
        if did_something {
            return Ok(());
        }
        return Err(Error::Other(
            "missing animation: pass --anim <BANK/ANIM> or <ANIM_PATH>".to_string(),
        ));
    };
    let Some(output_dir) = output_dir else {
        if did_something {
            return Ok(());
        }
        return Err(Error::Other(
            "missing output directory: pass --out <DIR> or <OUTPUT_DIR>".to_string(),
        ));
    };

    let print_paths = !args.dump_elements && !args.dump_symbol;
    render_animation(
        &ctx,
        &anim_path,
        &output_dir,
        &opts,
        args.manifest.as_deref(),
        args.strict,
        print_paths,
    )
}

#[derive(serde::Deserialize)]
struct RenderJobsFile {
    #[serde(default)]
    input: Vec<PathBuf>,
    #[serde(default)]
    build: Vec<PathBuf>,
    #[serde(default)]
    skin: Option<PathBuf>,
    #[serde(default)]
    symbol_map: Option<PathBuf>,
    #[serde(default)]
    out_dir: Option<PathBuf>,
    #[serde(default)]
    frame_bounds: Option<String>,
    jobs: Vec<RenderJobFile>,
}

#[derive(serde::Deserialize)]
struct RenderJobFile {
    anim: String,
    #[serde(default)]
    out: Option<PathBuf>,
    #[serde(default)]
    hide_layer: Vec<String>,
    #[serde(default)]
    disable_symbol: Vec<String>,
    #[serde(default)]
    frame_bounds: Option<String>,
    #[serde(default)]
    manifest: Option<PathBuf>,
}

fn parse_bounds_kind(value: Option<&str>) -> Result<BoundsKind> {
    match value.map(|v| v.to_ascii_lowercase()).as_deref() {
        None | Some("rect") => Ok(BoundsKind::Rect),
        Some("ink") => Ok(BoundsKind::Ink),
        Some(other) => Err(Error::Other(format!(
            "invalid frame_bounds '{other}', expected 'rect' or 'ink'"
        ))),
    }
}

fn cmd_render_jobs(args: &RenderArgs, jobs_path: &Path) -> Result<()> {
    let text = std::fs::read_to_string(jobs_path)?;
    let file: RenderJobsFile = serde_json::from_str(&text)
        .map_err(|e| Error::Other(format!("jobs file {}: {e}", jobs_path.display())))?;

    let inputs = if !file.input.is_empty() {
        file.input
    } else {
        args.input.clone()
    };
    if inputs.is_empty() {
        return Err(Error::Other(
            "no input files (pass -i or set 'input' in the jobs file)".to_string(),
        ));
    }
    let extra_builds = if !file.build.is_empty() {
        file.build
    } else {
        args.build.clone()
    };
    let skin = file.skin.or_else(|| args.skin.clone());
    let symbol_map = file.symbol_map.or_else(|| args.symbol_map.clone());
    let ctx = load_render_context(
        &inputs,
        skin.as_deref(),
        symbol_map.as_deref(),
        &extra_builds,
    )?;

    let default_bounds = parse_bounds_kind(file.frame_bounds.as_deref())?;
    for job in &file.jobs {
        let out = job
            .out
            .clone()
            .or_else(|| file.out_dir.clone())
            .ok_or_else(|| Error::Other(format!("job '{}': missing output dir", job.anim)))?;
        let kind = match job.frame_bounds.as_deref() {
            Some(value) => parse_bounds_kind(Some(value))?,
            None => default_bounds,
        };
        let opts = JobOptions {
            hide_layers: job.hide_layer.iter().cloned().collect(),
            disable_symbols: job.disable_symbol.iter().cloned().collect(),
            frame_bounds: kind,
        };
        render_animation(
            &ctx,
            &job.anim,
            &out,
            &opts,
            job.manifest.as_deref(),
            args.strict,
            true,
        )?;
    }
    Ok(())
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[derive(Serialize)]
struct AnimEntryJson<'a> {
    name: &'a str,
    frame_rate: f32,
    frames: usize,
}

#[derive(Serialize)]
struct BankJson<'a> {
    name: &'a str,
    animations: Vec<AnimEntryJson<'a>>,
}

#[derive(Serialize)]
struct AnimJson<'a> {
    version: i32,
    banks: Vec<BankJson<'a>>,
}

fn anim_json(anim: &AnimFile) -> AnimJson<'_> {
    AnimJson {
        version: anim.version,
        banks: anim
            .banks
            .iter()
            .map(|bank| BankJson {
                name: &bank.name,
                animations: bank
                    .animations
                    .iter()
                    .map(|animation| AnimEntryJson {
                        name: &animation.name,
                        frame_rate: animation.frame_rate,
                        frames: animation.frames.len(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[derive(Serialize)]
struct ListEntryJson<'a> {
    source_file: String,
    anim: Option<AnimJson<'a>>,
}

fn cmd_list(inputs: &[PathBuf], allow_missing: bool, json: bool) -> Result<()> {
    let files = dst_anim_tool::archive::load_archive_files(inputs)?;

    if json {
        let mut entries = Vec::with_capacity(files.len());
        for (path, archive) in &files {
            if archive.anim.is_none() && !allow_missing {
                eprintln!("hint: pass --allow-missing to treat this as success");
                return Err(Error::MissingData("anim.bin".to_string()));
            }
            entries.push(ListEntryJson {
                source_file: path.display().to_string(),
                anim: archive.anim.as_ref().map(anim_json),
            });
        }
        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| Error::Other(format!("list json: {e}")))?;
        println!("{json}");
        return Ok(());
    }

    let multiple = files.len() > 1;
    for (path, archive) in &files {
        match &archive.anim {
            Some(anim) => {
                if multiple {
                    println!("# {}", file_label(path));
                }
                for bank in &anim.banks {
                    for animation in &bank.animations {
                        println!("{}/{}", bank.name, animation.name);
                    }
                }
            }
            None if allow_missing => eprintln!("no anim.bin found"),
            None => {
                eprintln!("hint: pass --allow-missing to treat this as success");
                return Err(Error::MissingData("anim.bin".to_string()));
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct TexJson<'a> {
    name: &'a str,
    width: Option<u32>,
    height: Option<u32>,
    compression: Option<String>,
}

#[derive(Serialize)]
struct TexSourceJson<'a> {
    source_name: &'a str,
    tex_files: Vec<TexJson<'a>>,
}

#[derive(Serialize)]
struct SymbolJson<'a> {
    name: &'a str,
    frames: usize,
}

#[derive(Serialize)]
struct BuildJson<'a> {
    version: i32,
    name: &'a str,
    symbols: Vec<SymbolJson<'a>>,
    atlases: Vec<&'a str>,
}

#[derive(Serialize)]
struct InfoEntryJson<'a> {
    source_file: String,
    anim: Option<AnimJson<'a>>,
    build: Option<BuildJson<'a>>,
    tex_sources: Vec<TexSourceJson<'a>>,
}

fn info_json_entry<'a>(path: &Path, archive: &'a ParsedArchive) -> InfoEntryJson<'a> {
    let build = archive.build.as_ref().map(|build| BuildJson {
        version: build.version,
        name: &build.name,
        symbols: build
            .symbols
            .iter()
            .map(|symbol| SymbolJson {
                name: &symbol.name,
                frames: symbol.frames.len(),
            })
            .collect(),
        atlases: build.atlases.iter().map(|a| a.name.as_str()).collect(),
    });
    let tex_sources = archive
        .tex_sources
        .iter()
        .map(|source| TexSourceJson {
            source_name: &source.source_name,
            tex_files: source
                .tex_files
                .iter()
                .map(|(name, data)| {
                    if let Ok((header, metas, _)) = dst_anim_tool::ktex::parse(data) {
                        let compression = header.compression.to_string();
                        match metas.first() {
                            Some(m0) => TexJson {
                                name,
                                width: Some(m0.width),
                                height: Some(m0.height),
                                compression: Some(compression),
                            },
                            None => TexJson {
                                name,
                                width: None,
                                height: None,
                                compression: Some(compression),
                            },
                        }
                    } else {
                        TexJson {
                            name,
                            width: None,
                            height: None,
                            compression: None,
                        }
                    }
                })
                .collect(),
        })
        .collect();
    InfoEntryJson {
        source_file: path.display().to_string(),
        anim: archive.anim.as_ref().map(anim_json),
        build,
        tex_sources,
    }
}

fn print_info_text(archive: &ParsedArchive) {
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
            if let Ok((header, metas, _)) = dst_anim_tool::ktex::parse(data) {
                match metas.first() {
                    Some(m0) => println!(
                        "    {}: {}x{} {}",
                        name, m0.width, m0.height, header.compression
                    ),
                    None => println!("    {}: 0 mipmaps", name),
                }
            } else {
                println!("    {}: parse error", name);
            }
        }
    }
}

fn cmd_info(inputs: &[PathBuf], allow_missing: bool, json: bool) -> Result<()> {
    let files = dst_anim_tool::archive::load_archive_files(inputs)?;

    if json {
        let mut entries = Vec::with_capacity(files.len());
        for (path, archive) in &files {
            if archive.anim.is_none() && archive.build.is_none() && archive.tex_sources.is_empty() {
                if allow_missing {
                    entries.push(info_json_entry(path, archive));
                    continue;
                }
                eprintln!("hint: pass --allow-missing to treat this as success");
                return Err(Error::MissingData(
                    "anim.bin, build.bin or .tex files".to_string(),
                ));
            }
            entries.push(info_json_entry(path, archive));
        }
        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| Error::Other(format!("info json: {e}")))?;
        println!("{json}");
        return Ok(());
    }

    let multiple = files.len() > 1;
    for (path, archive) in &files {
        if archive.anim.is_none() && archive.build.is_none() && archive.tex_sources.is_empty() {
            if allow_missing {
                eprintln!(
                    "{}: no anim.bin, build.bin or .tex files found",
                    file_label(path)
                );
                continue;
            }
            eprintln!("hint: pass --allow-missing to treat this as success");
            return Err(Error::MissingData(
                "anim.bin, build.bin or .tex files".to_string(),
            ));
        }
        if multiple {
            println!("== {} ==", file_label(path));
        }
        print_info_text(archive);
    }
    Ok(())
}

fn cmd_decrypt(input: &Path, output: &Path) -> Result<()> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "dyn" {
        return Err(Error::Other("decrypt only supports .dyn files".to_string()));
    }
    let data = std::fs::read(input)?;
    let decrypted = dst_anim_tool::xor::xor_decrypt(&data);
    std::fs::write(output, &decrypted)?;
    println!("{}", output.display());
    Ok(())
}

fn cmd_decode(input: &Path, output_dir: &Path, allow_missing: bool) -> Result<()> {
    let archive = dst_anim_tool::archive::load_archives(&[input.to_path_buf()])?;
    if archive.tex_sources.is_empty() {
        if allow_missing {
            eprintln!("no .tex files found");
            return Ok(());
        }
        eprintln!("hint: pass --allow-missing to treat this as success");
        return Err(Error::MissingData(".tex files".to_string()));
    }
    std::fs::create_dir_all(output_dir)?;
    let tex_files = archive.tex_files();
    for (name, data) in tex_files {
        let img = dst_anim_tool::ktex::decode_rgba(data)?;
        let out_name = format!("{}.png", name);
        let out_path = output_dir.join(&out_name);
        img.save(&out_path)
            .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
        println!("{}", out_path.display());
    }
    Ok(())
}

fn is_archive_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "zip" | "dyn" | "bin"
    )
}

fn collect_archives(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_archives(&path, out)?;
        } else if is_archive_path(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn search_file(path: &Path, symbol: &str, case_sensitive: bool) -> Result<bool> {
    let data = std::fs::read(path)?;
    let archive = dst_anim_tool::archive::parse_file_by_path(path, &data)?;
    let Some(build) = archive.build else {
        return Ok(false);
    };
    Ok(build.symbols.iter().any(|s| {
        if case_sensitive {
            s.name == symbol
        } else {
            s.name.eq_ignore_ascii_case(symbol)
        }
    }))
}

fn cmd_search(dir: &Path, symbol: &str, case_sensitive: bool) -> Result<()> {
    if !dir.is_dir() {
        return Err(Error::Other(format!("not a directory: {}", dir.display())));
    }

    let mut files = Vec::new();
    collect_archives(dir, &mut files)?;
    files
        .par_iter()
        .for_each(|path| match search_file(path, symbol, case_sensitive) {
            Ok(true) => println!("{}", path.display()),
            Ok(false) => {}
            Err(e) => eprintln!("{}: {e}", path.display()),
        });
    Ok(())
}

#[cfg(feature = "gui")]
fn cmd_preview(inputs: Option<Vec<PathBuf>>) -> Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DST Anim Tool",
        native_options,
        Box::new(move |_cc| Ok(Box::new(crate::ui::App::new(inputs)))),
    )
    .map_err(|e| Error::Ui(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn decrypt_produces_valid_zip() {
        let input = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.dyn");
        let data = std::fs::read(&input).unwrap();
        let decrypted = dst_anim_tool::xor::xor_decrypt(&data);
        let archive = zip::ZipArchive::new(std::io::Cursor::new(decrypted.as_slice())).unwrap();
        assert!(!archive.is_empty());
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn decode_dyn_to_png() {
        let input = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.dyn");
        let archive = dst_anim_tool::archive::load_archives(&[input]).unwrap();
        assert!(!archive.tex_sources.is_empty());
        let tex_files = archive.tex_files();
        assert!(!tex_files.is_empty());
        for (name, data) in tex_files {
            let img = dst_anim_tool::ktex::decode_rgba(data).unwrap();
            assert!(img.width() > 0);
            assert!(img.height() > 0);
            let out_name = format!("{}.png", name);
            assert!(out_name.ends_with(".tex.png"));
        }
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn load_skin_archive_dyn() {
        let skin_path = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.dyn");
        let archive = load_skin_archive(&skin_path).unwrap();
        assert!(archive.build.is_some());
        assert!(!archive.tex_sources.is_empty());
        let build = archive.build.unwrap();
        assert_eq!(build.name, "abigail_ice");
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn load_skin_archive_build_zip() {
        let skin_path = std::path::PathBuf::from("data/anim/dynamic/abigail_ice.zip");
        let archive = load_skin_archive(&skin_path).unwrap();
        assert!(archive.build.is_some());
        assert!(!archive.tex_sources.is_empty());
        let build = archive.build.unwrap();
        assert_eq!(build.name, "abigail_ice");
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn search_file_finds_symbol() {
        let path = std::path::PathBuf::from("data/anim/abigail_flower.zip");
        assert!(search_file(&path, "petal1", false).unwrap());
        assert!(search_file(&path, "Petal1", false).unwrap());
        assert!(!search_file(&path, "Petal1", true).unwrap());
        assert!(search_file(&path, "petal1", true).unwrap());
        assert!(!search_file(&path, "nonexistent", false).unwrap());
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn collect_archives_recursive() {
        let tmp = std::env::temp_dir().join(format!(
            "dst_anim_tool_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let sub = tmp.join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        let src = std::path::PathBuf::from("data/anim/abigail_flower.zip");
        std::fs::copy(&src, tmp.join("a.zip")).unwrap();
        std::fs::copy(&src, sub.join("b.dyn")).unwrap();
        std::fs::write(tmp.join("readme.txt"), b"hello").unwrap();

        let mut files = Vec::new();
        collect_archives(&tmp, &mut files).unwrap();
        files.sort();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].file_name().unwrap(), "a.zip");
        assert_eq!(files[1].file_name().unwrap(), "b.dyn");

        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
