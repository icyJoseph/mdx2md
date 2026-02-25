use clap::Parser;
use mdx2md_core::config::Config;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "mdx2md", about = "Convert MDX files to Markdown")]
struct Cli {
    /// Input MDX file(s) or directory. Omit to read from stdin.
    #[arg()]
    input: Vec<PathBuf>,

    /// Output file (single input only) or directory (multiple inputs).
    /// Omit to write to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Path to a TOML config file.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// File extension for output files in directory mode (default: "md").
    #[arg(long, default_value = "md")]
    ext: String,
}

fn main() {
    let cli = Cli::parse();

    let config = match &cli.config {
        Some(path) => {
            let toml_str = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading config {}: {e}", path.display());
                std::process::exit(1);
            });
            Config::from_toml(&toml_str).unwrap_or_else(|e| {
                eprintln!("Error parsing config: {e}");
                std::process::exit(1);
            })
        }
        None => Config::default(),
    };

    if cli.input.is_empty() {
        // Stdin mode
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).unwrap_or_else(|e| {
            eprintln!("Error reading stdin: {e}");
            std::process::exit(1);
        });
        let result = convert_or_exit(&input, &config, "<stdin>");
        write_output(&result, cli.output.as_deref());
    } else {
        let files = collect_mdx_files(&cli.input);
        if files.is_empty() {
            eprintln!("No .mdx files found");
            std::process::exit(1);
        }

        if files.len() == 1 {
            let input = read_file(&files[0]);
            let result = convert_or_exit(&input, &config, &files[0].display().to_string());
            write_output(&result, cli.output.as_deref());
        } else {
            let out_dir = cli.output.unwrap_or_else(|| {
                eprintln!("Multiple input files require --output directory");
                std::process::exit(1);
            });
            std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| {
                eprintln!("Error creating output directory: {e}");
                std::process::exit(1);
            });
            for file in &files {
                let input = read_file(file);
                let result = convert_or_exit(&input, &config, &file.display().to_string());
                let out_name = file
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
                    + "."
                    + &cli.ext;
                let out_path = out_dir.join(out_name);
                std::fs::write(&out_path, &result).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {e}", out_path.display());
                    std::process::exit(1);
                });
                eprintln!("{} -> {}", file.display(), out_path.display());
            }
        }
    }
}

fn convert_or_exit(input: &str, config: &Config, source: &str) -> String {
    mdx2md_core::convert(input, config).unwrap_or_else(|e| {
        eprintln!("Error converting {source}: {e}");
        std::process::exit(1);
    })
}

fn read_file(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {e}", path.display());
        std::process::exit(1);
    })
}

fn write_output(content: &str, output: Option<&Path>) {
    match output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(path, content).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {e}", path.display());
                std::process::exit(1);
            });
        }
        None => {
            io::stdout().write_all(content.as_bytes()).unwrap_or_else(|e| {
                eprintln!("Error writing stdout: {e}");
                std::process::exit(1);
            });
        }
    }
}

fn collect_mdx_files(inputs: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_dir() {
            if let Ok(entries) = std::fs::read_dir(input) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("mdx") {
                        files.push(path);
                    }
                }
            }
        } else {
            files.push(input.clone());
        }
    }
    files.sort();
    files
}
