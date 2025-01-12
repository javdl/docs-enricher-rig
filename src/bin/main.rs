use docs_enricher::{github::Github, setup_pipeline};

#[tokio::main]
async fn main() {
    let github = Github::public_only();

    let files = github
        .download_repo("shuttle-hq".into(), "shuttle-docs".into())
        .await
        .unwrap();

    setup_pipeline(files).await;
}
