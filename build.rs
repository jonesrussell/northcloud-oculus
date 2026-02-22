use shaderc;
use std::fs;
use std::path::Path;

fn main() {
    let compiler = shaderc::Compiler::new().expect("Failed to create shaderc compiler");
    let mut options = shaderc::CompileOptions::new().expect("Failed to create compile options");
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_1 as u32,
    );
    options.set_source_language(shaderc::SourceLanguage::GLSL);

    let out_dir = std::env::var("OUT_DIR").unwrap();

    let shaders = [
        ("shaders/fullscreen.vert", shaderc::ShaderKind::Vertex),
        ("shaders/solid.frag", shaderc::ShaderKind::Fragment),
    ];

    println!("cargo:rerun-if-changed=build.rs");

    for (path, kind) in &shaders {
        println!("cargo:rerun-if-changed={}", path);

        let source = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read shader {}: {}", path, e));

        let artifact = compiler
            .compile_into_spirv(&source, *kind, path, "main", Some(&options))
            .unwrap_or_else(|e| panic!("Failed to compile shader {}: {}", path, e));

        if artifact.get_num_warnings() > 0 {
            for line in artifact.get_warning_messages().lines() {
                println!("cargo:warning=Shader {}: {}", path, line);
            }
        }

        let file_name = Path::new(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let out_path = Path::new(&out_dir).join(format!("{}.spv", file_name));

        fs::write(&out_path, artifact.as_binary_u8())
            .unwrap_or_else(|e| panic!("Failed to write SPIR-V {}: {}", out_path.display(), e));
    }
}
