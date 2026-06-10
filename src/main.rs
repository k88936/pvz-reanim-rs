use std::path::{Path, PathBuf};

use clap::Parser;
use rayon::prelude::*;
use reanim_parser::*;
use reanim_renderer::*;

#[derive(Parser)]
#[command(name = "pvz-reanim-rs")]
struct Cli {
    /// Path to the .reanim file
    reanim_file: PathBuf,
    /// Output WebP path (defaults to <reanim_stem>.webp)
    output: Option<PathBuf>,
    /// Additional directories to scan for image files (may be repeated)
    #[arg(long)]
    images_src: Vec<PathBuf>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let output_base = if let Some(out) = &cli.output {
        out.parent().unwrap_or(Path::new(".")).join(
            out.file_stem().and_then(|s| s.to_str()).unwrap_or("output")
        )
    } else {
        let stem = cli.reanim_file.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
        Path::new(&stem).to_path_buf()
    };

    let def = match ReanimatorDefinition::from_file(&cli.reanim_file) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Error loading reanim: {e}");
            std::process::exit(1);
        }
    };

    log::info!("FPS: {}, Tracks: {}, Frames: {}", def.fps, def.tracks.len(), def.frame_count());

    // Guess the image directory from the reanim file's location.
    let parent = cli.reanim_file.parent().unwrap_or(Path::new("."));
    let mut image_dirs: Vec<&Path> = vec![parent];
    image_dirs.extend(cli.images_src.iter().map(|p| p.as_path()));
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
