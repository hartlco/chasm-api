use std::fmt;

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

use base64::encode;

#[derive(Debug, Clone)]
pub enum ChasmError {
    InvalidCommitRequest,
    InvalidCommitJSON,
    FilenameMissing,
    PostfolderMissing,
    ImageDataMissing,
    RepoMissing,
    AccessTokenMissing,
}

impl fmt::Display for ChasmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChasmError::InvalidCommitRequest => write!(f, "Invalid Commit Request"),
            ChasmError::InvalidCommitJSON => write!(f, "Invalid Commit JSON"),
            ChasmError::FilenameMissing => write!(f, "Filename missing"),
            ChasmError::PostfolderMissing => write!(f, "Postfolder missing"),
            ChasmError::ImageDataMissing => write!(f, "Image Data Missing"),
            ChasmError::RepoMissing => write!(f, "Repo Missng"),
            ChasmError::AccessTokenMissing => write!(f, "Access Token Missing"),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct CommitContent {
    pub message: String,
    pub content: String,
    pub path: String,
}

impl CommitContent {
    pub fn new(message: String, content: String, path: String) -> CommitContent {
        CommitContent {
            message: message,
            content: encode(content),
            path: path,
        }
    }

    pub fn new_from_image(message: String, content: Vec<u8>, path: String) -> CommitContent {
        CommitContent {
            message: message,
            content: encode(content),
            path: path,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PostContent {
    pub date: DateTime<Utc>,
    pub postfolder: String,
    pub title: Option<String>,
    pub content: Vec<ContentPart>,
    pub location: ContentLocation,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    Header {text: String},
    Paragraph {text: String},
    Image {filename: String},
    Link {title: String, url: String},
}

#[derive(Deserialize, Serialize)]
pub struct CommitResponse {
    pub content: CommitResponseContent,
}

#[derive(Deserialize, Serialize)]
pub struct CommitResponseContent {
    pub download_url: String,
}

#[derive(Deserialize, Serialize)]
pub struct ImageUploadResponse {
    pub commit_response: Option<CommitResponse>,
    pub filename: String
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ContentLocation {
    Github { repo: String, access_token: String },
    Local { path: String }
}