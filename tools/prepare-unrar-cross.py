"""Correct host/target detection in unrar_sys 0.5.8 for the GNU cross-build."""
import json
import pathlib
import shutil
import subprocess

metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--format-version', '1']))
package = next(p for p in metadata['packages'] if p['name'] == 'unrar_sys')
source = pathlib.Path(package['manifest_path']).parent
destination = pathlib.Path('target/unrar-sys-portable').resolve()
shutil.copytree(source, destination, dirs_exist_ok=True)
path = destination / 'build.rs'
text = path.read_text()
start = text.index('    if cfg!(windows)')
end = text.index('    let files:', start)
text = text[:start] + '''    let windows = std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows";
    if windows {
        println!("cargo:rustc-link-lib=powrprof");
        println!("cargo:rustc-link-lib=shell32");
        if std::env::var("CARGO_CFG_TARGET_ENV").unwrap() == "gnu" {
            let compiler = std::env::var("CXX_x86_64_pc_windows_gnu").unwrap_or("x86_64-w64-mingw32-g++-posix".into());
            let output = std::process::Command::new(compiler).arg("-print-file-name=libstdc++.a").output().unwrap();
            let archive = String::from_utf8(output.stdout).unwrap();
            let directory = std::path::Path::new(archive.trim()).parent().unwrap();
            println!("cargo:rustc-link-search=native={}", directory.display());
            println!("cargo:rustc-link-search=native=/usr/x86_64-w64-mingw32/lib");
            println!("cargo:rustc-link-lib=static=winpthread");
        }
    } else { println!("cargo:rustc-link-lib=pthread"); }
''' + text[end:]
text = text.replace('let files: Vec<String>', 'let mut files: Vec<String>')
text = text.replace('        #[cfg(windows)]\n        "isnt",\n', '')
text = text.replace('    cc::Build::new()', '    if windows { files.push("vendor/unrar/isnt.cpp".into()); }\n    cc::Build::new()')
path.write_text(text)
print(destination)

# Windows SDK headers are case-insensitive on Windows, unlike a Linux cross-build.
import re
headers = pathlib.Path('/usr/x86_64-w64-mingw32/include')
for source_file in (destination / 'vendor/unrar').glob('*'):
    if source_file.suffix not in ('.hpp', '.cpp', '.h'):
        continue
    content = source_file.read_text(errors='surrogateescape')
    content = re.sub(r'#include <([^>]+)>', lambda match: '#include <' + match[1].lower() + '>' if (headers / match[1].lower()).is_file() else match[0], content)
    source_file.write_text(content, errors='surrogateescape')
