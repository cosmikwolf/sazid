use helix_loader::grammar::{build_grammars, fetch_grammars};

fn main() {
  // set environment variables that exist in .env
  dotenv::dotenv().ok();
  if std::env::var("HELIX_DISABLE_AUTO_GRAMMAR_BUILD").is_err() {
    fetch_grammars().expect("Failed to fetch tree-sitter grammars");
    build_grammars(Some(std::env::var("TARGET").unwrap()))
      .expect("Failed to compile tree-sitter grammars");
  }

  let git_output = std::process::Command::new("git").args(["rev-parse", "--git-dir"]).output().ok();
  let git_dir = git_output.as_ref().and_then(|output| {
    std::str::from_utf8(&output.stdout)
      .ok()
      .and_then(|s| s.strip_suffix('\n').or_else(|| s.strip_suffix("\r\n")))
  });

  prost_build::compile_protos(&["src/app/treesitter/treesitter.proto"], &["src/app/treesitter/"])
    .unwrap();

  // Tell cargo to rebuild if the head or any relevant refs change.
  if let Some(git_dir) = git_dir {
    let git_path = std::path::Path::new(git_dir);
    let refs_path = git_path.join("refs");
    if git_path.join("HEAD").exists() {
      println!("cargo:rerun-if-changed={}/HEAD", git_dir);
    }
    if git_path.join("packed-refs").exists() {
      println!("cargo:rerun-if-changed={}/packed-refs", git_dir);
    }
    if refs_path.join("heads").exists() {
      println!("cargo:rerun-if-changed={}/refs/heads", git_dir);
    }
    if refs_path.join("tags").exists() {
      println!("cargo:rerun-if-changed={}/refs/tags", git_dir);
    }
  }

  let git_output = std::process::Command::new("git")
    .args(["describe", "--always", "--tags", "--long", "--dirty"])
    .output()
    .ok();
  let git_info =
    git_output.as_ref().and_then(|output| std::str::from_utf8(&output.stdout).ok().map(str::trim));
  let cargo_pkg_version = env!("CARGO_PKG_VERSION");

  // Default git_describe to cargo_pkg_version
  let mut git_describe = String::from(cargo_pkg_version);

  if let Some(git_info) = git_info {
    // If the `git_info` contains `CARGO_PKG_VERSION`, we simply use `git_info` as it is.
    // Otherwise, prepend `CARGO_PKG_VERSION` to `git_info`.
    if git_info.contains(cargo_pkg_version) {
      // Remove the 'g' before the commit sha
      let git_info = &git_info.replace('g', "");
      git_describe = git_info.to_string();
    } else {
      git_describe = format!("v{}-{}", cargo_pkg_version, git_info);
    }
  }
  // create new key command:
  //    openssl enc -pbkdf2 -salt -in openai_api_key -out open_api_key.enc -pass pass:"openai"
  // decrypt key command:
  //    /opt/homebrew/bin/openssl enc -d -pbkdf2 -in ~/.local/share/keys/open_api_key.enc -pass pass:'openai'
  println!("cargo:rustc-env=SAZID_GIT_INFO={}", git_describe);

  // let openai_api_key = std::process::Command::new("/opt/homebrew/bin/openssl")
  //   .args(["enc", "-d", "-pbkdf2", "-in", "/Users/tenkai/.local/share/keys/open_api_key.enc", "-pass", "pass:'openai'"])
  //   .output()
  //   .ok()
  //   .as_ref()
  //   .and_then(|output| std::str::from_utf8(&output.stdout).ok().map(str::trim))
  //   .unwrap()
  //   .to_string();
  // println!("cargo:rustc-env=OPENAI_API_KEY={}", openai_api_key);
}
