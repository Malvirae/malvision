use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/branding/malvision-logo.png");
    println!("cargo:rerun-if-changed=build.rs");

    let logo_path = Path::new("assets/branding/malvision-logo.png");
    let ico_path = Path::new("assets/branding/malvision.ico");

    if logo_path.exists() && (!ico_path.exists() || is_newer(logo_path, ico_path)) {
        generate_ico(logo_path, ico_path);
    }

    if cfg!(target_os = "windows") {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let rc_path = out_dir.join("resource.rc");
        let obj_path = out_dir.join("resource.o");

        let ico_path_abs = std::fs::canonicalize(ico_path)
            .unwrap_or_else(|_| ico_path.to_path_buf())
            .to_str()
            .unwrap()
            .replace('\\', "/");

        let rc_content = format!(
            r#"#pragma code_page(65001)
1 VERSIONINFO
FILEVERSION 1,0,0,0
PRODUCTVERSION 1,0,0,0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "ProductName", "MALVISION\0"
            VALUE "FileDescription", "Malvision Remote Connection Manager\0"
            VALUE "FileVersion", "1.0.0.0\0"
            VALUE "ProductVersion", "1.0.0\0"
            VALUE "InternalName", "Malvision\0"
            VALUE "OriginalFilename", "Malvision.exe\0"
            VALUE "LegalCopyright", "Copyright (c) 2026 Malvirae\0"
        END
        BLOCK "040c04b0"
        BEGIN
            VALUE "ProductName", "MALVISION\0"
            VALUE "FileDescription", "Malvision Remote Connection Manager\0"
            VALUE "FileVersion", "1.0.0.0\0"
            VALUE "ProductVersion", "1.0.0\0"
            VALUE "InternalName", "Malvision\0"
            VALUE "OriginalFilename", "Malvision.exe\0"
            VALUE "LegalCopyright", "Copyright (c) 2026 Malvirae\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200, 0x040c, 1200
    END
END
1 ICON "{}"
"#,
            ico_path_abs
        );

        std::fs::write(&rc_path, rc_content).expect("Failed to write resource.rc");

        // Use windres to compile rc into a COFF object file
        let status = Command::new("windres")
            .arg("-i")
            .arg(&rc_path)
            .arg("-O")
            .arg("coff")
            .arg("-o")
            .arg(&obj_path)
            .status()
            .expect("Failed to execute windres");

        assert!(status.success(), "windres failed to compile resource.rc");

        // Pass the compiled object directly to the linker!
        println!("cargo:rustc-link-arg={}", obj_path.to_str().unwrap());
    }
}

fn is_newer(a: &Path, b: &Path) -> bool {
    let Ok(ma) = a.metadata().and_then(|m| m.modified()) else {
        return false;
    };
    let Ok(mb) = b.metadata().and_then(|m| m.modified()) else {
        return false;
    };
    ma > mb
}

fn generate_ico(src: &Path, dst: &Path) {
    let img = image::open(src).expect("Failed to open logo PNG");
    let sizes = [16, 24, 32, 48, 64, 128, 256];

    let mut png_blobs: Vec<(u32, Vec<u8>)> = Vec::new();
    for &size in &sizes {
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let mut png_bytes = Vec::new();
        resized
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .expect("Failed to encode PNG for ICO");
        png_blobs.push((size, png_bytes));
    }

    let file = File::create(dst).expect("Failed to create malvision.ico");
    let mut writer = BufWriter::new(file);

    // Header
    writer.write_all(&0u16.to_le_bytes()).unwrap(); // Reserved
    writer.write_all(&1u16.to_le_bytes()).unwrap(); // ICO type
    writer
        .write_all(&(png_blobs.len() as u16).to_le_bytes())
        .unwrap(); // Count

    let mut current_offset = 6 + (png_blobs.len() as u32 * 16);

    for (size, data) in &png_blobs {
        let w_byte = if *size == 256 { 0u8 } else { *size as u8 };
        let h_byte = if *size == 256 { 0u8 } else { *size as u8 };
        writer.write_all(&[w_byte, h_byte, 0, 0]).unwrap(); // W, H, Colors, Reserved
        writer.write_all(&1u16.to_le_bytes()).unwrap(); // Planes
        writer.write_all(&32u16.to_le_bytes()).unwrap(); // BPP
        writer
            .write_all(&(data.len() as u32).to_le_bytes())
            .unwrap(); // Size
        writer.write_all(&current_offset.to_le_bytes()).unwrap(); // Offset
        current_offset += data.len() as u32;
    }

    for (_, data) in &png_blobs {
        writer.write_all(data).unwrap();
    }
    writer.flush().unwrap();
}
