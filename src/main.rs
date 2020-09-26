use actix_multipart::Multipart;
use actix_web::client::Client;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder, ResponseError, Result};

use std::fmt;
use std::str;

use futures::{StreamExt, TryStreamExt};

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

use serde_json::Value;

use base64::encode;

#[derive(Debug, Clone)]
enum ChasmError {
    InvalidCommitRequest,
    InvalidCommitJSON,
    InvalidImageUploadRequest,
    NoAccessToken,
}

impl fmt::Display for ChasmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid first item to double")
    }
}

#[derive(Deserialize, Serialize)]
struct CommitContent {
    message: String,
    content: String,
    path: String,
}

impl CommitContent {
    fn new(message: String, content: String, path: String) -> CommitContent {
        CommitContent {
            message: message,
            content: encode(content),
            path: path,
        }
    }

    fn new_from_image(message: String, content: Vec<u8>, path: String) -> CommitContent {
        CommitContent {
            message: message,
            content: encode(content),
            path: path,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct PostContent {
    repo: String,
    access_token: String,
    title: Option<String>,
    content: Vec<ContentPart>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
enum ContentPart {
    Header(String),
    Paragraph(String),
    Image(String),
}

#[derive(Deserialize, Serialize)]
struct CommitResponse {
    content: CommitResponseContent,
}

#[derive(Deserialize, Serialize)]
struct CommitResponseContent {
    download_url: String,
}

async fn commit(
    repo: String,
    access_token: String,
    content: CommitContent,
) -> std::result::Result<CommitResponse, ChasmError> {
    let repository = repo;

    let post_url = format!(
        "https://api.github.com/repos/{}/contents/{}",
        repository, content.path
    );
    let token = access_token;

    let client = Client::default();

    let response = client
        .put(post_url)
        .header("User-Agent", "actix-web/3.0")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json;charset=UTF-8")
        .send_body(serde_json::to_string(&content).unwrap_or("Empty".to_string()))
        .await;

    let mut successful_response;

    match response {
        Ok(response) => {
            successful_response = response;
        }
        Err(_) => {
            return Err(ChasmError::InvalidCommitRequest);
        }
    }

    successful_response
        .json()
        .await
        .map_err(|_| ChasmError::InvalidCommitJSON)
}

async fn post_content(content: web::Json<PostContent>) -> HttpResponse {
    let now: DateTime<Utc> = Utc::now();
    let filename_date = now.format("%Y-%m-%dT%H:%M:%SZ");
    let date = now.format("%Y-%m-%dT%H:%M:%SZ");

    let mut body_string = "".to_string();

    let mut title_string = "".to_string();

    if let Some(title) = &content.title {
        title_string.push_str(&title);
    }

    for content_part in &content.content {
        match content_part {
            ContentPart::Header(text) => {
                let header_string = format!("## {}\n", text);
                body_string.push_str(&header_string);
            }
            ContentPart::Paragraph(text) => {
                let paragraph_string = format!("{}\n", text);
                body_string.push_str(&paragraph_string);
            }
            ContentPart::Image(link) => {
                let image_string = format!("![]({})\n", link);
                body_string.push_str(&image_string);
            }
        }
    }

    let content_string = format!(
        "+++\ntitle = \"{}\"\ndate = {}\n+++\n{}\n\n<!-- more -->",
        title_string, date, body_string
    );

    let c2 = CommitContent::new(
        "Add post".to_string(),
        content_string,
        format!("content/{}.md", filename_date),
    );

    let commit_content = commit(
        content.repo.to_string(),
        content.access_token.to_string(),
        c2,
    )
    .await;

    match commit_content {
        Ok(_) => HttpResponse::Ok().json(content.0),
        Err(error) => HttpResponse::from_error(actix_web::error::ErrorBadRequest(error)),
    }
}

async fn upload_image(mut payload: Multipart) -> Result<web::Json<CommitResponse>> {
    let mut content: Option<CommitContent> = Option::None;
    let mut repo: Option<String> = Option::None;
    let mut access_token: Option<String> = Option::None;

    while let Ok(Some(field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();
        let field_name = content_disposition.unwrap();

        if field_name.get_name() == Some("access_token") {
            let vec= vec_from(field).await;
            access_token = Some(str::from_utf8(&vec).unwrap().to_string());
            continue;
        }

        if field_name.get_name() == Some("repo") {
            let vec= vec_from(field).await;
            repo = Some(str::from_utf8(&vec).unwrap().to_string());
            continue;
        }

        if field_name.get_name() == Some("file") {
            let filename = field_name.get_filename().unwrap();
            let filepath = format!("static/{}", sanitize_filename::sanitize(&filename));

            let vec= vec_from(field).await;

            content = Some(CommitContent::new_from_image(
                "Add image".to_string(),
                vec,
                filepath.to_string(),
            ));

            continue;

            // println!("{}", content.message);
        }
    }

    // TODO: Fix unwrapping

    let unwrapped_repo = repo.unwrap();
    let unwrapped_access_token = access_token.unwrap();
    let unwrapped_content = content.unwrap();

    let response = commit(unwrapped_repo, unwrapped_access_token, unwrapped_content).await;

    Ok(web::Json(response.unwrap()))
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

async fn vec_from(field: actix_multipart::Field) -> Vec<u8> {
    let mut vec = Vec::new();
    let mut field = field;

    while let Some(chunk) = field.next().await {
        let data = chunk.unwrap();
        let vec_b = data.to_vec();
        vec.extend(vec_b);
    }

    return vec
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(hello)
            .service(web::resource("/post_content").route(web::post().to(post_content)))
            .service(web::resource("/upload_image").route(web::post().to(upload_image)))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
