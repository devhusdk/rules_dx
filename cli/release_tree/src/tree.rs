use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn run(args: &[String]) -> Result<(), String> {
    let (out, steps) = args.split_first().ok_or("missing output directory")?;
    let out = PathBuf::from(out);
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut rest = steps.iter();
    while let Some(step) = rest.next() {
        match step.as_str() {
            "--dir" => {
                let rel = rest.next().ok_or("--dir needs a path")?;
                let dest = below(&out, rel)?;
                fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
            }
            "--copy" => {
                let source = rest.next().ok_or("--copy needs a source")?;
                let rel = rest.next().ok_or("--copy needs a path")?;
                copy(Path::new(source), &below(&out, rel)?)?;
            }
            other => return Err(format!("unknown step {other}")),
        }
    }
    Ok(())
}

fn below(out: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = Path::new(rel);
    if rel.is_absolute() || rel.components().any(|c| c == Component::ParentDir) {
        return Err(format!("{} leaves the output directory", rel.display()));
    }
    Ok(out.join(rel))
}

fn copy(source: &Path, dest: &Path) -> Result<(), String> {
    let meta = fs::metadata(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if meta.is_dir() {
        fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        let entries = fs::read_dir(source).map_err(|e| format!("{}: {e}", source.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("{}: {e}", source.display()))?;
            copy(&entry.path(), &dest.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    if dest.exists() {
        fs::remove_file(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    }
    fs::copy(source, dest).map_err(|e| format!("{}: {e}", source.display()))?;
    Ok(())
}
