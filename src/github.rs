use std::{
    fmt,
    io::{Cursor, Read},
    path::{Display, Path, PathBuf},
};

use flate2::read::GzDecoder;
use http_body_util::BodyExt;
use octocrab::{Octocrab, OctocrabBuilder};
use tokio::fs::read_dir;
use tokio_tar::Archive;

pub struct Github {
    octo: octocrab::Octocrab,
}

impl Github {
    pub fn public_only() -> Self {
        let octo = Octocrab::builder().build().unwrap();

        Self { octo }
    }
    fn from_env() -> Self {
        let gh_api_token = std::env::var("GITHUB_API_TOKEN").unwrap();

        let octo = Octocrab::builder()
            .personal_token(gh_api_token)
            .build()
            .unwrap();

        Self { octo }
    }

    pub async fn download_repo(
        &self,
        org: String,
        repo: String,
    ) -> Result<Vec<File>, Box<dyn std::error::Error>> {
        let repo = self.octo.repos(&org, repo);
        let latest_commit = repo
            .list_commits()
            .send()
            .await?
            .items
            .first()
            .unwrap()
            .sha
            .clone();

        println!("Commit: {latest_commit}");
        let tarball = repo
            .download_tarball(latest_commit)
            .await
            .unwrap()
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();

        // Decompress the .tar.gz
        let mut gz_decoder = GzDecoder::new(Cursor::new(tarball));
        let mut decomp_bytes = Vec::new();
        gz_decoder.read_to_end(&mut decomp_bytes)?;
        println!("Decompressed bytes");
        let mut archive = Archive::new(Cursor::new(decomp_bytes));

        // Extract the archive to a temporary directory
        let temp_dir = tempfile::tempdir()?;
        let output_path = temp_dir.path();
        archive.unpack(output_path).await?;
        println!("Unpacked archive");

        // Recursively collect Markdown files
        let markdown_files = collect_markdown_files(output_path).await?;

        Ok(markdown_files)
    }
}

async fn collect_markdown_files(dir: &Path) -> Result<Vec<File>, Box<dyn std::error::Error>> {
    let mut markdown_files = Vec::new();
    let mut dirs_to_visit = vec![dir.to_path_buf()];

    while let Some(current_dir) = dirs_to_visit.pop() {
        let mut entries = read_dir(&current_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() && !path.to_str().unwrap().contains("_snippets") {
                dirs_to_visit.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("mdx")
                && !path.ends_with("overview.mdx")
            {
                let file_contents = tokio::fs::read_to_string(&path).await.unwrap();
                if file_contents.len() < 250 {
                    continue;
                }
                let file = File::new(path.to_str().unwrap().to_owned(), file_contents);
                markdown_files.push(file);
            }
        }
    }

    Ok(markdown_files)
}

#[derive(Debug, Clone)]
pub struct File {
    pub path: String,
    pub file_contents: String,
}

impl File {
    fn new(path: String, file_contents: String) -> Self {
        Self {
            path,
            file_contents,
        }
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            path,
            file_contents,
        } = self;
        write!(f, "path: {path}\nfile_contents: {file_contents}")
    }
}
