use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use rayon::prelude::*;
use reanim_makefile_gen::*;
use reanim_parser::*;
use reanim_renderer::*;

#[derive(Parser)]
#[command(name = "pvz-reanim-rs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a .reanim file to WebP
    Render {
        /// Path to the .reanim file
        reanim_file: PathBuf,
        /// Output WebP path (defaults to <reanim_stem>.webp)
        output: Option<PathBuf>,
        /// Additional directories to scan for image files (may be repeated)
        #[arg(long)]
        images_src: Vec<PathBuf>,
    },
    /// Generate a Makefile for building all .reanim files in a directory
    #[command(name = "gen_makefile")]
    GenMakefile {
        /// Directory containing .reanim files
        #[arg(long)]
        reanim: PathBuf,
        /// Output path for the generated Makefile
        #[arg(long)]
        makefile_output: PathBuf,
        /// Output directory for WebP files
        #[arg(long)]
        webp_output: PathBuf,
        /// Additional directories to scan for image files (may be repeated)
        #[arg(long)]
        images_src: Vec<PathBuf>,
    },
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match cli.command {
        Command::Render { reanim_file, output, images_src } => {
            cmd_render(reanim_file, output, images_src);
        }
        Command::GenMakefile { reanim, makefile_output, webp_output, images_src } => {
            if let Err(e) = cmd_gen_makefile(reanim, makefile_output, webp_output, images_src) {
                log::error!("gen-makefile error: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn cmd_render(reanim_file: PathBuf, output: Option<PathBuf>, images_src: Vec<PathBuf>) {
    let output_base = if let Some(out) = &output {
        out.parent().unwrap_or(Path::new(".")).join(
            out.file_stem().and_then(|s| s.to_str()).unwrap_or("output")
        )
    } else {
        let stem = reanim_file.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
        Path::new(&stem).to_path_buf()
    };

    let def = match ReanimatorDefinition::from_file(&reanim_file) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Error loading reanim: {e}");
            std::process::exit(1);
        }
    };

    log::info!("FPS: {}, Tracks: {}, Frames: {}", def.fps, def.tracks.len(), def.frame_count());

    // Guess the image directory from the reanim file's location.
    let parent = reanim_file.parent().unwrap_or(Path::new("."));
    let mut image_dirs: Vec<&Path> = vec![parent];
    image_dirs.extend(images_src.iter().map(|p| p.as_path()));
    let images = ImageDb::from_directories(&image_dirs);

    let masks = find_anim_masks(&def);
    if masks.is_empty() {
        // No masks — render all frames to a single animated WebP
        let output = output_base.with_extension("webp");
        log::info!("No anim_* masks found, rendering full animation to {}", output.display());
        if let Err(e) = render_to_webp(&def, &images, (100, 100), &output) {
            log::error!("Render error: {e}");
            std::process::exit(1);
        }
        log::info!("Done: {}", output.display());
    } else {
        // One WebP per mask
        log::info!("Found {} anim_* masks:", masks.len());
        for m in &masks {
            log::info!("  {}: frames {}-{}", m.name, m.frame_start, m.frame_end);
        }
        let results: Vec<_> = masks
            .par_iter()
            .map(|mask| {
                let output = output_base.with_extension(format!("{}.webp", mask.name));
                log::info!("Rendering mask '{}' -> {}", mask.name, output.display());
                render_range_to_webp(&def, &images, (100, 100), &output,
                                     mask.frame_start, mask.frame_end)
            })
            .collect();

        for (mask, result) in masks.iter().zip(results.iter()) {
            if let Err(e) = result {
                log::error!("Render error for '{}': {e}", mask.name);
                std::process::exit(1);
            }
            log::info!("Done: {}", mask.name);
        }
    }
}

fn cmd_gen_makefile(
    reanim: PathBuf,
    makefile_output: PathBuf,
    webp_output: PathBuf,
    images_src: Vec<PathBuf>,
) -> Result<()> {
    let reanim_files = discover_reanim_files(&reanim)?;

    if reanim_files.is_empty() {
        eprintln!(
            "Warning: no .reanim files found in {}",
            reanim.display()
        );
    }

    let reanim_image_map = scan_image_filenames(&reanim);
    let images_src_maps: Vec<HashMap<String, String>> = images_src
        .iter()
        .map(|d| scan_image_filenames(d))
        .collect();

    let mut file_infos = Vec::new();
    for path in &reanim_files {
        match analyze_reanim_file(path, &reanim_image_map, &images_src_maps) {
            Ok(info) => {
                eprintln!("  {}: {} masks, {} images", info.stem, info.masks.len(), info.images.len());
                file_infos.push(info);
            }
            Err(e) => {
                eprintln!("Warning: skipping {}: {e}", path.display());
            }
        }
    }

    eprintln!("Found {} valid .reanim file(s)", file_infos.len());

    generate_makefile(
        &makefile_output,
        &reanim,
        &webp_output,
        &images_src,
        &file_infos,
    )?;

    eprintln!("Makefile written to: {}", makefile_output.display());

    Ok(())
}
