use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the Cargo dependency as the single source for runtime compatibility.
    let text = std::fs::read_to_string("Cargo.toml")?;
    let manifest: toml::Value = toml::from_str(&text)?;
    let required = manifest
        .get("dependencies")
        .and_then(|deps| deps.get("synapt-core"))
        .and_then(|core| core.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or("synapt-core must declare its required version")?;
    writeln!(std::io::stdout(), "cargo:rerun-if-changed=Cargo.toml")?;
    writeln!(
        std::io::stdout(),
        "cargo:rustc-env=SYNAPT_CORE_REQUIREMENT={required}"
    )?;
    tauri_build::build();
    Ok(())
}
