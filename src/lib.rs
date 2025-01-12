use github::File;
use rig::extractor::ExtractionError;
use rig::parallel_op;
use rig::pipeline::agent_ops::{extract, prompt};
use rig::pipeline::parallel::Parallel;
use rig::pipeline::{Op, passthrough};
use rig::{parallel, pipeline, providers::openai};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod github;

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
struct Data {
    data_type: String,
    filepath: String,
    issues: Vec<Issue>,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
struct Issue {
    priority: String,
    content: String,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
struct ClassifierResponse {
    data_type: String,
    filepath: String,
}

pub async fn setup_pipeline(files: Vec<File>) {
    let files: Vec<String> = files.into_iter().map(|x| x.to_string()).collect();
    let openai = openai::Client::from_env();

    let classifier_agent = openai
        .extractor::<ClassifierResponse>("gpt-4o")
        .preamble(
            "
        Categorise this page according to the Diataxis framework for the given input.

        Options: [tutorial, how-to, explanation, reference]

        Respond with only the option you have selected and the original filepath. Skip all prose.
        ",
        )
        .build();
    let advisor_agent = openai.extractor::<Data>("gpt-4o").preamble("
        You are now a Senior DevRel.

        Your job is to analyze the documentation found below and check whether or not the documentation is fit for purpose for someone who is looking to access information quickly and easily. The markdown code will be parsed into HTML for use on a web page. The Rust macros in code snippets do not need to be explained. If the snippet talks about something other than the product the documentation is for, assume that the user already knows how to use it and do not provide any improvement suggestions related to it.

        Focus on:
        - Things that are missing from the docs

        Considerations:
        - Do not write out any strengths and write concisely
        - <CodeGroup> tags are used to group code snippets together, so you don't need to comment on it.

        Return:
        - The document type
        - The filepath
        - Your response as a JSON list of issues
        - Additionally for each issue, return an priority level [low, medium, high, urgent]
        ").build();

    let pipeline = pipeline::new()
        .chain(parallel!(passthrough(), extract(classifier_agent)))
        .map(|(query, classifier)| {
            let Ok(classifier) = classifier else {
                panic!("Classify prompt was an error :(")
            };

            format!(
                "
                Document type: {}
                Filepath: {}
                Data: {query}
                ",
                classifier.data_type, classifier.filepath
            )
        })
        .chain(extract(advisor_agent));

    let result = pipeline
        .batch_call(files.len(), files)
        .await
        .into_iter()
        .filter_map(|f| f.ok())
        .collect::<Vec<Data>>();

    let result_bytes = serde_json::to_vec_pretty(&result).unwrap();

    tokio::fs::write("result.json", result_bytes).await.unwrap();
}
