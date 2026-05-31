use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use reanim_parser::ReanimatorDefinition;
use reanim_renderer::find_anim_masks;

/// Image filename prefixes that `ImageDb` strips during lookup.
const IMAGE_PREFIXES: &[&str] = &["IMAGE_REANIM_", "IMAGE_"];

#[derive(Parser)]
#[command(name = "reanim_makefile_gen")]
struct Cli {
    /// Directory containing .reanim files
    #[arg(long)]
    reanim: PathBuf,
    /// Output path for the generated Makefile
    #[arg(long)]
    makefile_output: PathBuf,
    /// Output directory for GIF files
    #[arg(long)]
    gif_output: PathBuf,
    /// Additional directories to scan for image files (may be repeated)
    #[arg(long)]
    images_src: Vec<PathBuf>,
}

fn discover_reanim_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let read_dir = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("reanim") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

struct ReanimFileInfo {
    stem: String,
    masks: Vec<String>,
    images: Vec<String>,
}

/// Scan a directory for PNG/JPG files and build a map from lowercased stem -> full filename.
fn scan_image_filenames(dir: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return map,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = match path.extension().and_then(|s| s.to_str()) {
            Some(e) => e,
            None => continue,
        };
        if ext != "png" && ext != "jpg" {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let filename = format!("{}.{}", stem, ext);
            map.insert(stem.to_lowercase(), filename);
        }
    }
    map
}

/// Resolve an XML image ref to the exact filename stem using the same
/// prefix-stripping + lowercasing logic as `ImageDb::get`.
fn resolve_xml_image_ref(xml_ref: &str, image_map: &HashMap<String, String>) -> Option<String> {
    for prefix in IMAGE_PREFIXES {
        if let Some(stripped) = xml_ref.strip_prefix(prefix) {
            let key = stripped.to_lowercase();
            if let Some(exact) = image_map.get(&key) {
                return Some(exact.clone());
            }
        }
    }
    // Fallback: try the whole ref lowercased
    image_map.get(&xml_ref.to_lowercase()).cloned()
}

/// Resolve an XML image ref against the reanim dir map first, then each images_src
/// dir map in order, and return the full Makefile dependency path.
fn resolve_image_dep_path(
    xml_ref: &str,
    reanim_map: &HashMap<String, String>,
    images_src_maps: &[HashMap<String, String>],
) -> Option<String> {
    // Try reanim dir first
    if let Some(exact) = resolve_xml_image_ref(xml_ref, reanim_map) {
        return Some(format!("$(REANIM_DIR)/{exact}"));
    }
    // Try each images-src dir in order
    for (idx, map) in images_src_maps.iter().enumerate() {
        if let Some(exact) = resolve_xml_image_ref(xml_ref, map) {
            return Some(format!("$(IMAGES_SRC_DIR_{idx})/{exact}"));
        }
    }
    None
}

fn analyze_reanim_file(
    path: &Path,
    reanim_map: &HashMap<String, String>,
    images_src_maps: &[HashMap<String, String>],
) -> Result<ReanimFileInfo> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid file name")?
        .to_string();

    let def = ReanimatorDefinition::from_file(path)
        .with_context(|| format!("Failed to parse: {}", path.display()))?;

    let masks = find_anim_masks(&def);
    let mask_names: Vec<String> = masks.into_iter().map(|m| m.name).collect();
    let xml_images = def.referenced_images();
    let resolved_images: Vec<String> = xml_images
        .iter()
        .map(|xml_ref| {
            match resolve_image_dep_path(xml_ref, reanim_map, images_src_maps) {
                Some(dep) => dep,
                None => {
                    eprintln!(
                        "Warning: could not resolve image ref '{}' for {}, using raw ref as fallback",
                        xml_ref, stem
                    );
                    // fallback: use raw ref with .png extension assumption
                    if xml_ref.contains('.') {
                        format!("$(REANIM_DIR)/{xml_ref}")
                    } else {
                        format!("$(REANIM_DIR)/{xml_ref}.png")
                    }
                }
            }
        })
        .collect();

    Ok(ReanimFileInfo {
        stem,
        masks: mask_names,
        images: resolved_images,
    })
}

/// Compute a relative path from `base` (a directory) to `target`.
fn make_relative(base: &Path, target: &Path) -> PathBuf {
    let base_components: Vec<_> = base.components().collect();
    let target_components: Vec<_> = target.components().collect();

    // Find common prefix length
    let prefix_len = base_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // How many ".." segments do we need?
    let up_count = base_components.len() - prefix_len;

    let mut result = PathBuf::new();
    for _ in 0..up_count {
        result.push("..");
    }
    for component in &target_components[prefix_len..] {
        result.push(component);
    }

    result
}

fn generate_makefile(
    makefile_path: &Path,
    reanim_dir: &Path,
    gif_output: &Path,
    images_src_dirs: &[PathBuf],
    files: &[ReanimFileInfo],
) -> Result<()> {
    let makefile_dir = makefile_path.parent().unwrap_or_else(|| Path::new("."));
    let reanim_dir_rel = make_relative(makefile_dir, reanim_dir);
    let gif_output_rel = make_relative(makefile_dir, gif_output);
    let reanim_dir_esc = reanim_dir_rel.to_string_lossy();
    let gif_output_esc = gif_output_rel.to_string_lossy();

    let mut content = String::new();
    content.push_str("# Generated by reanim_makefile_gen \u{2014} do not edit manually\n");
    content.push_str("\n");
    let reanim_dir_str = reanim_dir_esc.trim_end_matches('/');
    let reanim_dir_out = if reanim_dir_str.is_empty() { "." } else { reanim_dir_str };
    content.push_str(&format!("REANIM_DIR := {}\n", reanim_dir_out));
    for (idx, images_src_dir) in images_src_dirs.iter().enumerate() {
        let images_src_rel = make_relative(makefile_dir, images_src_dir);
        let images_src_str = images_src_rel.to_string_lossy();
        let images_src_out = if images_src_str.trim_end_matches('/').is_empty() { "." } else { images_src_str.trim_end_matches('/') };
        content.push_str(&format!("IMAGES_SRC_DIR_{idx} := {}\n", images_src_out));
    }
    content.push_str(&format!("GIF_OUTPUT := {}\n", gif_output_esc.trim_end_matches('/')));
    content.push_str("BIN := pvz-reanim-rs\n");
    content.push_str("\n");

    if files.is_empty() {
        content.push_str(".PHONY: all\n");
        content.push_str("all:\n");
        content.push_str("\t@echo \"No .reanim files to build.\"\n");
    } else {
        // all target
        content.push_str(".PHONY: all\n");
        content.push_str("all:");
        for info in files {
            content.push_str(&format!(" $(GIF_OUTPUT)/{}/.keep", info.stem));
        }
        content.push_str("\n");
        content.push_str("\n");

        // per-file rules
        for info in files {
            let stem = &info.stem;
            content.push_str(&format!(
                "$(GIF_OUTPUT)/{stem}/.keep: $(REANIM_DIR)/{stem}.reanim {}\n",
                info.images.join(" ")
            ));
            content.push_str("\tmkdir -p $(dir $@)\n");
            let images_src_args: String = (0..images_src_dirs.len())
                .map(|idx| format!(" --images-src $(IMAGES_SRC_DIR_{idx})"))
                .collect();
            content.push_str(&format!(
                "\t$(BIN) \"$<\" \"$(dir $@){stem}\"{images_src_args}\n"
            ));
            content.push_str("\ttouch \"$@\"\n");
            content.push_str("\n");
        }
    }

    if let Some(parent) = makefile_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(makefile_path, content)
        .with_context(|| format!("Failed to write Makefile to {}", makefile_path.display()))?;

    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let reanim_files = discover_reanim_files(&args.reanim)?;

    if reanim_files.is_empty() {
        eprintln!(
            "Warning: no .reanim files found in {}",
            args.reanim.display()
        );
    }

    let reanim_image_map = scan_image_filenames(&args.reanim);
    let images_src_maps: Vec<HashMap<String, String>> = args
        .images_src
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
        &args.makefile_output,
        &args.reanim,
        &args.gif_output,
        &args.images_src,
        &file_infos,
    )?;

    eprintln!("Makefile written to: {}", args.makefile_output.display());

    Ok(())
}
